pub mod matcher;
pub mod near;
pub mod print;

use crate::db;
use crate::deck;
use crate::error::AppResult;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

pub const BADGE_MIN: i64 = 80;

pub struct FileInfo {
    pub path: String,
    pub size: i64,
    pub mtime: i64,
}

fn info(path: &Path) -> Option<FileInfo> {
    let meta = fs::metadata(path).ok()?;
    let mtime = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_millis() as i64;
    Some(FileInfo { path: path.to_string_lossy().into_owned(), size: meta.len() as i64, mtime })
}

pub fn repath(conn: &Connection, from: &Path, to: &Path) -> rusqlite::Result<()> {
    let (from, to) = (from.to_string_lossy(), to.to_string_lossy());
    conn.execute("UPDATE OR REPLACE fingerprint SET path = ?2 WHERE path = ?1", params![from, to])?;
    for table in ["dupe", "dupe_dismissed"] {
        conn.execute(&format!("UPDATE OR REPLACE {table} SET a = ?2 WHERE a = ?1"), params![from, to])?;
        conn.execute(&format!("UPDATE OR REPLACE {table} SET b = ?2 WHERE b = ?1"), params![from, to])?;
    }
    conn.execute("UPDATE OR REPLACE dupe SET a = b, b = a, offset_ms = -offset_ms WHERE a > b", [])?;
    conn.execute("UPDATE OR REPLACE dupe_dismissed SET a = b, b = a WHERE a > b", [])?;
    Ok(())
}

pub fn forget_missing(conn: &Connection) -> rusqlite::Result<usize> {
    let gone: Vec<String> = conn
        .prepare("SELECT path FROM fingerprint")?
        .query_map([], |r| r.get::<_, String>(0))?
        .filter_map(|r| r.ok())
        .filter(|p| !Path::new(p).exists())
        .collect();
    for p in &gone {
        conn.execute("DELETE FROM fingerprint WHERE path = ?1", params![p])?;
        conn.execute("DELETE FROM dupe WHERE a = ?1 OR b = ?1", params![p])?;
    }
    Ok(gone.len())
}

pub fn files(conn: &Connection) -> rusqlite::Result<Vec<FileInfo>> {
    let deck = db::get_setting(conn, "deck_path")?.unwrap_or_default();
    let table = db::get_setting(conn, "table_path")?.unwrap_or_default();
    let mut out = Vec::new();
    let names: Vec<String> = conn
        .prepare("SELECT file_name FROM card WHERE deck_path = ?1 AND status IN ('in_deck','deferred') ORDER BY status = 'deferred', position")?
        .query_map(params![deck], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    out.extend(names.iter().filter_map(|n| info(&Path::new(&deck).join(n))));
    let piles: Vec<String> = conn
        .prepare("SELECT name FROM pile WHERE table_path = ?1 AND is_trash = 0 AND exists_on_disk = 1")?
        .query_map(params![table], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    for name in piles {
        let Ok(dir) = fs::read_dir(Path::new(&table).join(&name)) else { continue };
        out.extend(dir.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| deck::is_media(p)).filter_map(|p| info(&p)));
    }
    Ok(out)
}

fn sha256(path: &Path) -> Option<Vec<u8>> {
    let mut file = fs::File::open(path).ok()?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = file.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Some(hasher.finalize().to_vec())
}

fn cached_sha(conn: &Connection, f: &FileInfo) -> rusqlite::Result<Option<Vec<u8>>> {
    let known: Option<Vec<u8>> = conn
        .query_row(
            "SELECT sha256 FROM fingerprint WHERE path = ?1 AND size = ?2 AND mtime = ?3 AND sha256 IS NOT NULL",
            params![f.path, f.size, f.mtime],
            |r| r.get(0),
        )
        .optional()?;
    if known.is_some() {
        return Ok(known);
    }
    let Some(sha) = sha256(Path::new(&f.path)) else { return Ok(None) };
    conn.execute(
        "INSERT INTO fingerprint(path, size, mtime, sha256) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(path) DO UPDATE SET sha256 = excluded.sha256,
           state = CASE WHEN size = excluded.size AND mtime = excluded.mtime THEN state ELSE 'new' END,
           frames = CASE WHEN size = excluded.size AND mtime = excluded.mtime THEN frames END,
           audio = CASE WHEN size = excluded.size AND mtime = excluded.mtime THEN audio END,
           semantic = CASE WHEN size = excluded.size AND mtime = excluded.mtime THEN semantic END,
           mean = CASE WHEN size = excluded.size AND mtime = excluded.mtime THEN mean END,
           size = excluded.size, mtime = excluded.mtime",
        params![f.path, f.size, f.mtime, sha],
    )?;
    Ok(Some(sha))
}

pub fn exact_pairs(conn: &Connection) -> rusqlite::Result<BTreeSet<(String, String)>> {
    let mut by_size: HashMap<i64, Vec<FileInfo>> = HashMap::new();
    for f in files(conn)? {
        by_size.entry(f.size).or_default().push(f);
    }
    let mut pairs = BTreeSet::new();
    for group in by_size.into_values().filter(|g| g.len() > 1) {
        let mut by_sha: HashMap<Vec<u8>, Vec<String>> = HashMap::new();
        for f in &group {
            if let Some(sha) = cached_sha(conn, f)? {
                by_sha.entry(sha).or_default().push(f.path.clone());
            }
        }
        for same in by_sha.values() {
            for (i, a) in same.iter().enumerate() {
                for b in &same[i + 1..] {
                    pairs.insert(if a < b { (a.clone(), b.clone()) } else { (b.clone(), a.clone()) });
                }
            }
        }
    }
    let dismissed: BTreeSet<(String, String)> = conn
        .prepare("SELECT a, b FROM dupe_dismissed")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    Ok(pairs.difference(&dismissed).cloned().collect())
}

pub fn refresh_exact(conn: &Connection) -> rusqlite::Result<bool> {
    let pairs = exact_pairs(conn)?;
    let known: BTreeSet<(String, String)> = conn
        .prepare("SELECT a, b FROM dupe WHERE kind = 'exact'")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    if known == pairs {
        return Ok(false);
    }
    let tx = conn.unchecked_transaction()?;
    tx.execute("DELETE FROM dupe WHERE kind = 'exact'", [])?;
    for (a, b) in &pairs {
        tx.execute(
            "INSERT OR REPLACE INTO dupe(a, b, kind, confidence, visual, audio, semantic) VALUES (?1, ?2, 'exact', 100, 1, 1, 1)",
            params![a, b],
        )?;
    }
    tx.commit()?;
    Ok(true)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DupeView {
    path: String,
    #[serde(rename = "where")]
    place: &'static str,
    pile_name: Option<String>,
    card_id: Option<i64>,
    kind: String,
    confidence: i64,
    offset_ms: Option<i64>,
    duration_ms: Option<i64>,
    width: Option<i64>,
    height: Option<i64>,
    bitrate: Option<i64>,
    size: i64,
    taken_at: Option<i64>,
    visual: Option<f64>,
    audio: Option<f64>,
    semantic: Option<f64>,
    better: bool,
}

struct Copy {
    card_id: Option<i64>,
    in_pile: bool,
    pixels: i64,
    bitrate: i64,
    taken_at: i64,
    position: f64,
}

fn copy_of(conn: &Connection, path: &str, deck: &str) -> rusqlite::Result<(Copy, Option<i64>, Option<i64>, Option<i64>)> {
    let p = Path::new(path);
    let in_deck = p.parent().map(|d| d.to_string_lossy().to_lowercase() == deck.to_lowercase()).unwrap_or(false);
    let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let row = if in_deck {
        conn.query_row(
            "SELECT id, width, height, duration_ms, taken_at, position FROM card
             WHERE deck_path = ?1 AND file_name = ?2 AND status IN ('in_deck','deferred')",
            params![deck, name],
            |r| Ok((r.get::<_, i64>(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get::<_, Option<i64>>(4)?, r.get::<_, f64>(5)?)),
        )
        .optional()?
    } else {
        conn.query_row(
            "SELECT id, width, height, duration_ms, taken_at, position FROM card WHERE current_path = ?1 ORDER BY id DESC LIMIT 1",
            params![path],
            |r| Ok((r.get::<_, i64>(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get::<_, Option<i64>>(4)?, r.get::<_, f64>(5)?)),
        )
        .optional()?
    };
    let size = fs::metadata(p).map(|m| m.len() as i64).unwrap_or(0);
    let (id, w, h, d, taken, pos): (Option<i64>, Option<i64>, Option<i64>, Option<i64>, Option<i64>, f64) = match row {
        Some((id, w, h, d, t, pos)) => (Some(id), w, h, d, t, pos),
        None => (None, None, None, None, None, f64::MAX),
    };
    let (w, h, d) = match (w, h, d) {
        (Some(w), Some(h), Some(d)) => (Some(w), Some(h), Some(d)),
        _ => conn
            .query_row("SELECT width, height, duration_ms FROM fingerprint WHERE path = ?1 AND state = 'ok'", params![path], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .optional()?
            .unwrap_or((w, h, d)),
    };
    let bitrate = d.filter(|d| *d > 0).map(|d| size * 8 * 1000 / d);
    let copy = Copy {
        card_id: if in_deck { id } else { None },
        in_pile: !in_deck,
        pixels: w.unwrap_or(0) * h.unwrap_or(0),
        bitrate: bitrate.unwrap_or(0),
        taken_at: taken.unwrap_or(i64::MAX),
        position: pos,
    };
    Ok((copy, w, h, d))
}

fn better(a: &Copy, b: &Copy) -> bool {
    (a.pixels, a.bitrate, a.in_pile, std::cmp::Reverse(a.taken_at))
        .cmp(&(b.pixels, b.bitrate, b.in_pile, std::cmp::Reverse(b.taken_at)))
        .then(b.position.partial_cmp(&a.position).unwrap_or(std::cmp::Ordering::Equal))
        .is_gt()
}

fn series_of(conn: &Connection, deck: &str, path: &str) -> Option<i64> {
    let p = Path::new(path);
    if p.parent().map(|d| d.to_string_lossy().to_lowercase()) != Some(deck.to_lowercase()) {
        return None;
    }
    let name = p.file_name()?.to_string_lossy().into_owned();
    conn.query_row(
        "SELECT series_id FROM card WHERE deck_path = ?1 AND file_name = ?2 AND status IN ('in_deck','deferred')",
        params![deck, name],
        |r| r.get::<_, Option<i64>>(0),
    )
    .ok()
    .flatten()
}

fn same_series(conn: &Connection, deck: &str, a: &str, b: &str) -> bool {
    matches!((series_of(conn, deck, a), series_of(conn, deck, b)), (Some(x), Some(y)) if x == y)
}

pub fn dupes_for(conn: &Connection, card_id: i64) -> AppResult<Vec<DupeView>> {
    let deck = db::get_setting(conn, "deck_path")?.unwrap_or_default();
    let table = db::get_setting(conn, "table_path")?.unwrap_or_default();
    let Some(card) = deck::cards(conn, &[card_id])?.into_iter().next() else { return Ok(Vec::new()) };
    type Row = (String, String, String, i64, Option<i64>, [Option<f64>; 3]);
    let rows: Vec<Row> = conn
        .prepare(
            "SELECT a, b, kind, confidence, offset_ms, visual, audio, semantic FROM dupe WHERE (a = ?1 OR b = ?1) AND confidence >= ?2
             ORDER BY confidence DESC",
        )?
        .query_map(params![card.path, BADGE_MIN], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, [r.get(5)?, r.get(6)?, r.get(7)?]))
        })?
        .collect::<Result<_, _>>()?;
    let (me, _, _, _) = copy_of(conn, &card.path, &deck)?;
    let mut out = Vec::new();
    for (a, b, kind, confidence, offset, [visual, audio, semantic]) in rows {
        let mine = a == card.path;
        let offset = offset.map(|o| if mine { o } else { -o });
        let other = if mine { b } else { a };
        let p = PathBuf::from(&other);
        if !p.is_file() || same_series(conn, &deck, &card.path, &other) {
            continue;
        }
        let (copy, width, height, duration_ms) = copy_of(conn, &other, &deck)?;
        let pile_name = copy
            .in_pile
            .then(|| p.parent().filter(|d| d.parent().map(|t| t == Path::new(&table)).unwrap_or(false)))
            .flatten()
            .and_then(|d| d.file_name().map(|n| n.to_string_lossy().into_owned()));
        if copy.in_pile && pile_name.is_none() {
            continue;
        }
        out.push(DupeView {
            path: other,
            place: if copy.in_pile { "pile" } else { "deck" },
            pile_name,
            card_id: copy.card_id,
            kind,
            confidence,
            offset_ms: offset,
            duration_ms,
            width,
            height,
            bitrate: Some(copy.bitrate).filter(|b| *b > 0),
            size: fs::metadata(&p).map(|m| m.len() as i64).unwrap_or(0),
            taken_at: (copy.taken_at != i64::MAX).then_some(copy.taken_at),
            visual,
            audio,
            semantic,
            better: better(&copy, &me),
        });
    }
    out.sort_by_key(|d| (!d.better, d.place == "deck", -d.confidence));
    Ok(out)
}

pub fn dismiss(conn: &Connection, a: &str, b: &str) -> AppResult<()> {
    let (a, b) = if a < b { (a, b) } else { (b, a) };
    conn.execute("INSERT OR IGNORE INTO dupe_dismissed(a, b) VALUES (?1, ?2)", params![a, b])?;
    conn.execute("DELETE FROM dupe WHERE a = ?1 AND b = ?2", params![a, b])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_pairs_hash_only_same_size_and_skip_dismissed() {
        let dir = tempfile::tempdir().unwrap();
        let deck = dir.path().join("deck");
        let table = dir.path().join("table");
        fs::create_dir_all(&deck).unwrap();
        fs::create_dir_all(table.join("Музыка")).unwrap();
        fs::write(deck.join("a.mp4"), b"same").unwrap();
        fs::write(deck.join("b.mp4"), b"same").unwrap();
        fs::write(deck.join("c.mp4"), b"diff").unwrap();
        fs::write(deck.join("d.mp4"), b"longer").unwrap();
        fs::write(table.join("Музыка").join("e.mp4"), b"same").unwrap();
        let conn = Connection::open_in_memory().unwrap();
        db::init(&conn).unwrap();
        db::set_setting(&conn, "deck_path", &deck.to_string_lossy()).unwrap();
        db::set_setting(&conn, "table_path", &table.to_string_lossy()).unwrap();
        conn.execute("INSERT INTO pile(table_path, name, ord) VALUES (?1, 'Музыка', 0)", params![table.to_string_lossy()])
            .unwrap();
        for (i, n) in ["a.mp4", "b.mp4", "c.mp4", "d.mp4"].iter().enumerate() {
            conn.execute(
                "INSERT INTO card(deck_path, file_name, size, mtime, position, taken_at) VALUES (?1, ?2, 4, 0, ?3, ?3)",
                params![deck.to_string_lossy(), n, i as f64],
            )
            .unwrap();
        }
        assert!(refresh_exact(&conn).unwrap());
        assert_eq!(conn.query_row("SELECT COUNT(*) FROM dupe", [], |r| r.get::<_, i64>(0)).unwrap(), 3);
        let hashed: i64 = conn.query_row("SELECT COUNT(*) FROM fingerprint", [], |r| r.get(0)).unwrap();
        assert_eq!(hashed, 4);
        assert!(!refresh_exact(&conn).unwrap());

        let views = dupes_for(&conn, 1).unwrap();
        assert_eq!(views.len(), 2);
        assert_eq!((views[0].place, views[0].better), ("pile", true));
        assert_eq!(views[0].pile_name.as_deref(), Some("Музыка"));
        assert_eq!((views[1].place, views[1].better, views[1].card_id), ("deck", false, Some(2)));

        let a = deck.join("a.mp4").to_string_lossy().into_owned();
        let b = deck.join("b.mp4").to_string_lossy().into_owned();
        let g = groups(&conn).unwrap();
        assert_eq!((g.groups.len(), g.groups[0].items.len(), g.groups[0].pairs.len()), (1, 3, 3));
        assert_eq!((g.groups[0].items[0].best, g.groups[0].items[0].place), (true, "pile"));
        assert_eq!((g.printed, g.total), (0, 5));

        dismiss(&conn, &b, &a).unwrap();
        assert!(!refresh_exact(&conn).unwrap());
        assert_eq!(dupes_for(&conn, 1).unwrap().len(), 1);
        let g = groups(&conn).unwrap();
        assert_eq!((g.groups.len(), g.groups[0].items.len(), g.groups[0].pairs.len()), (1, 3, 2));
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupItem {
    path: String,
    #[serde(rename = "where")]
    place: &'static str,
    pile_name: Option<String>,
    card_id: Option<i64>,
    duration_ms: Option<i64>,
    width: Option<i64>,
    height: Option<i64>,
    bitrate: Option<i64>,
    size: i64,
    best: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    kind: String,
    confidence: i64,
    trim: Option<[i64; 3]>,
    items: Vec<GroupItem>,
    pairs: Vec<[String; 2]>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Groups {
    groups: Vec<Group>,
    printed: i64,
    total: i64,
}

const GROUP_MIN: i64 = 50;

pub fn groups(conn: &Connection) -> AppResult<Groups> {
    let deck = db::get_setting(conn, "deck_path")?.unwrap_or_default();
    let table = db::get_setting(conn, "table_path")?.unwrap_or_default();
    let rows: Vec<(String, String, String, i64, Option<i64>)> = conn
        .prepare("SELECT a, b, kind, confidence, offset_ms FROM dupe WHERE confidence >= ?1")?
        .query_map(params![GROUP_MIN], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?
        .collect::<Result<_, _>>()?;
    let rows: Vec<_> = rows
        .into_iter()
        .filter(|(a, b, ..)| Path::new(a).is_file() && Path::new(b).is_file() && !same_series(conn, &deck, a, b))
        .collect();
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut parent: Vec<usize> = Vec::new();
    fn root(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    for (a, b, ..) in &rows {
        for p in [a, b] {
            if !index.contains_key(p) {
                index.insert(p.clone(), parent.len());
                parent.push(parent.len());
            }
        }
        let (ra, rb) = (root(&mut parent, index[a]), root(&mut parent, index[b]));
        parent[ra] = rb;
    }
    let roots: HashMap<String, usize> = index.iter().map(|(p, &i)| (p.clone(), root(&mut parent, i))).collect();
    let mut members: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    for (p, r) in &roots {
        members.entry(*r).or_default().push(p.clone());
    }
    let mut out = Vec::new();
    for (r, mut list) in members {
        list.sort();
        let edges: Vec<&(String, String, String, i64, Option<i64>)> = rows.iter().filter(|(a, ..)| roots[a] == r).collect();
        let top = edges.iter().filter(|e| e.2 != "exact").max_by_key(|e| e.3).or_else(|| edges.iter().max_by_key(|e| e.3)).unwrap();
        let mut copies = Vec::new();
        for p in &list {
            let (copy, w, h, d) = copy_of(conn, p, &deck)?;
            copies.push((p.clone(), copy, w, h, d));
        }
        let best = (0..copies.len()).max_by(|&i, &j| {
            if better(&copies[i].1, &copies[j].1) {
                std::cmp::Ordering::Greater
            } else if better(&copies[j].1, &copies[i].1) {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        });
        let trim = (top.2 == "trim").then(|| {
            let da = copies.iter().find(|c| c.0 == top.0).and_then(|c| c.4).unwrap_or(0);
            let db = copies.iter().find(|c| c.0 == top.1).and_then(|c| c.4).unwrap_or(0);
            let off = top.4.unwrap_or(0);
            if db <= da { [off, off + db, da] } else { [-off, -off + da, db] }
        });
        let mut items: Vec<GroupItem> = copies
            .into_iter()
            .enumerate()
            .map(|(i, (path, copy, width, height, duration_ms))| GroupItem {
                pile_name: copy.in_pile.then(|| pile_of(Path::new(&path), &table)).flatten(),
                place: if copy.in_pile { "pile" } else { "deck" },
                card_id: copy.card_id,
                duration_ms,
                width,
                height,
                bitrate: Some(copy.bitrate).filter(|b| *b > 0),
                size: fs::metadata(&path).map(|m| m.len() as i64).unwrap_or(0),
                best: Some(i) == best,
                path,
            })
            .collect();
        items.sort_by_key(|i| !i.best);
        out.push(Group {
            kind: top.2.clone(),
            confidence: edges.iter().map(|e| e.3).max().unwrap_or(0),
            trim,
            items,
            pairs: edges.iter().map(|e| [e.0.clone(), e.1.clone()]).collect(),
        });
    }
    out.sort_by_key(|g| (g.kind != "exact", -g.confidence));
    let (printed, total) = printed(conn)?;
    Ok(Groups { groups: out, printed, total })
}

fn pile_of(p: &Path, table: &str) -> Option<String> {
    let dir = p.parent()?;
    (dir.parent()? == Path::new(table)).then(|| dir.file_name().map(|n| n.to_string_lossy().into_owned())).flatten()
}

pub fn printed(conn: &Connection) -> rusqlite::Result<(i64, i64)> {
    let known: HashMap<String, (i64, i64)> = conn
        .prepare("SELECT path, size, mtime FROM fingerprint WHERE state != 'new'")?
        .query_map([], |r| Ok((r.get(0)?, (r.get(1)?, r.get(2)?))))?
        .collect::<Result<_, _>>()?;
    let all = files(conn)?;
    let done = all.iter().filter(|f| known.get(&f.path) == Some(&(f.size, f.mtime))).count();
    Ok((done as i64, all.len() as i64))
}
