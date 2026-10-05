use crate::error::{io_error, AppResult};
use rusqlite::{params, Connection, Row};
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub const VIDEO_EXT: [&str; 6] = ["mp4", "mov", "mkv", "webm", "avi", "m4v"];
pub const PHOTO_EXT: [&str; 5] = ["jpg", "jpeg", "png", "webp", "gif"];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardView {
    pub id: i64,
    pub file_name: String,
    pub path: String,
    pub size: i64,
    pub taken_at: Option<i64>,
    pub duration_ms: Option<i64>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub orientation: Option<String>,
    pub frames: i64,
    pub stage: String,
    pub error: Option<String>,
    pub status: String,
    pub kind: String,
    pub camera: Option<String>,
    pub taken_from: Option<String>,
    pub series_id: Option<i64>,
}

pub const CARD_COLUMNS: &str =
    "id, deck_path, file_name, size, taken_at, duration_ms, width, height, orientation, frames, stage, error, status, kind, camera, taken_from, series_id";

pub fn card_from_row(r: &Row) -> rusqlite::Result<CardView> {
    let deck: String = r.get(1)?;
    let name: String = r.get(2)?;
    Ok(CardView {
        id: r.get(0)?,
        path: Path::new(&deck).join(&name).to_string_lossy().into_owned(),
        file_name: name,
        size: r.get(3)?,
        taken_at: r.get(4)?,
        duration_ms: r.get(5)?,
        width: r.get(6)?,
        height: r.get(7)?,
        orientation: r.get(8)?,
        frames: r.get(9)?,
        stage: r.get(10)?,
        error: r.get(11)?,
        status: r.get(12)?,
        kind: r.get(13)?,
        camera: r.get(14)?,
        taken_from: r.get(15)?,
        series_id: r.get(16)?,
    })
}

fn has_ext(path: &Path, list: &[&str]) -> bool {
    path.is_file()
        && !path.file_name().map(|n| n.to_string_lossy().starts_with('.')).unwrap_or(true)
        && path.extension().map(|e| list.contains(&e.to_string_lossy().to_lowercase().as_str())).unwrap_or(false)
}

pub fn is_video(path: &Path) -> bool {
    has_ext(path, &VIDEO_EXT)
}

pub fn is_photo(path: &Path) -> bool {
    has_ext(path, &PHOTO_EXT)
}

pub fn is_media(path: &Path) -> bool {
    is_video(path) || is_photo(path)
}

pub fn media_name(name: &str) -> bool {
    let ext = Path::new(name).extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    !name.starts_with('.') && (VIDEO_EXT.contains(&ext.as_str()) || PHOTO_EXT.contains(&ext.as_str()))
}

pub fn photo_name(name: &str) -> bool {
    Path::new(name).extension().map(|e| PHOTO_EXT.contains(&e.to_string_lossy().to_lowercase().as_str())).unwrap_or(false)
}

pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

pub fn taken_from_name(name: &str) -> Option<i64> {
    let stem = name.split('.').next()?;
    let digits: String = stem.chars().filter(|c| c.is_ascii_digit()).collect();
    if stem.len() == 13 && digits.len() == 13 {
        let ms: i64 = digits.parse().ok()?;
        return (ms > 946_684_800_000 && ms < 4_102_444_800_000).then_some(ms);
    }
    if digits.len() >= 14 && (stem.starts_with("VID") || stem.starts_with("IMG") || stem.starts_with("PXL")) {
        let n = |a: usize, b: usize| digits[a..b].parse::<i64>().ok();
        let (y, mo, d, h, mi, s) = (n(0, 4)?, n(4, 6)?, n(6, 8)?, n(8, 10)?, n(10, 12)?, n(12, 14)?);
        if !(2000..2100).contains(&y) || !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || s > 59 {
            return None;
        }
        return Some(((days_from_civil(y, mo, d) * 24 + h) * 60 + mi) * 60_000 + s * 1000);
    }
    None
}

fn mtime_ms(meta: &fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub struct SyncResult {
    pub added: Vec<i64>,
    pub gone: Vec<i64>,
    pub pending: bool,
}

#[cfg(windows)]
fn ready(path: &Path) -> bool {
    use std::os::windows::fs::OpenOptionsExt;
    fs::OpenOptions::new().read(true).share_mode(0).open(path).is_ok()
}

#[cfg(not(windows))]
fn ready(_: &Path) -> bool {
    true
}

pub fn sync(conn: &Connection, deck: &str) -> AppResult<SyncResult> {
    sync_seen(conn, deck, &mut |_| {})
}

pub fn sync_seen(conn: &Connection, deck: &str, seen: &mut dyn FnMut(usize)) -> AppResult<SyncResult> {
    let dir = Path::new(deck);
    let mut files: Vec<(String, i64, i64)> = fs::read_dir(dir)
        .map_err(|e| io_error(&e, dir))?
        .filter_map(|e| e.ok())
        .filter(|e| is_media(&e.path()))
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            Some((e.file_name().to_string_lossy().into_owned(), meta.len() as i64, mtime_ms(&meta)))
        })
        .collect();
    files.sort_by_key(|f| f.0.to_lowercase());

    let tx = rusqlite::Transaction::new_unchecked(conn, rusqlite::TransactionBehavior::Immediate)?;
    let rows: HashMap<String, (i64, i64, i64)> = tx
        .prepare("SELECT id, file_name, size, mtime FROM card WHERE deck_path = ?1 AND status IN ('in_deck','deferred')")?
        .query_map(params![deck], |r| Ok((r.get::<_, String>(1)?.to_lowercase(), (r.get(0)?, r.get(2)?, r.get(3)?))))?
        .collect::<Result<_, _>>()?;

    let mut busy = std::collections::HashSet::new();
    let mut busy_cards = std::collections::HashSet::new();
    for row in tx
        .prepare("SELECT i.from_path, i.card_id FROM move_item i JOIN move m ON m.id = i.move_id WHERE m.state IN ('pending','undoing')")?
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<i64>>(1)?)))?
    {
        let (path, card) = row?;
        busy.insert(path.to_lowercase());
        busy_cards.extend(card);
    }
    let present: HashMap<String, ()> = files.iter().map(|f| (f.0.to_lowercase(), ())).collect();
    let mut gone = Vec::new();
    for (name, (id, _, _)) in &rows {
        if !present.contains_key(name) && !busy_cards.contains(id) && !dir.join(name).exists() {
            tx.execute("UPDATE card SET status = 'gone' WHERE id = ?1", params![id])?;
            gone.push(*id);
        }
    }

    let mut position: f64 = tx.query_row(
        "SELECT COALESCE(MAX(position), 0) FROM card WHERE deck_path = ?1 AND status IN ('in_deck','deferred')",
        params![deck],
        |r| r.get(0),
    )?;
    let mut insert = tx.prepare(
        "INSERT INTO card(deck_path, file_name, size, mtime, taken_at, taken_from, kind, position) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
    )?;
    let mut added = Vec::new();
    let mut pending = false;
    let total = files.len();
    for (i, (name, size, mtime)) in files.into_iter().enumerate() {
        if i % 50 == 0 {
            seen(i);
        }
        match rows.get(&name.to_lowercase()) {
            Some((id, s, m)) if (*s, *m) != (size, mtime) => {
                tx.execute(
                    "UPDATE card SET size = ?2, mtime = ?3, stage = 'new', frames = 0, error = NULL WHERE id = ?1",
                    params![id, size, mtime],
                )?;
            }
            Some(_) => {}
            None if !dir.join(&name).exists() => {}
            None if !ready(&dir.join(&name)) || busy.contains(&dir.join(&name).to_string_lossy().to_lowercase()) => pending = true,
            None => {
                let named = taken_from_name(&name);
                position += 1.0;
                insert.execute(params![
                    deck,
                    name,
                    size,
                    mtime,
                    named.unwrap_or(mtime),
                    if named.is_some() { "name" } else { "file" },
                    if photo_name(&name) { "photo" } else { "video" },
                    position
                ])?;
                added.push(tx.last_insert_rowid());
            }
        }
    }
    drop(insert);
    tx.commit()?;
    seen(total);
    Ok(SyncResult { added, gone, pending })
}

pub fn window(conn: &Connection, deck: &str, from: i64, count: i64, kind: &str) -> AppResult<Vec<CardView>> {
    let sql = format!(
        "SELECT {CARD_COLUMNS} FROM card WHERE deck_path = ?1 AND status IN ('in_deck','deferred')
           AND (?4 = 'all' OR kind = ?4) AND (series_id IS NULL OR series_id = id)
         ORDER BY position LIMIT ?2 OFFSET ?3"
    );
    let cards = conn
        .prepare(&sql)?
        .query_map(params![deck, count, from, kind], card_from_row)?
        .collect::<Result<_, _>>()?;
    Ok(cards)
}

pub fn cards(conn: &Connection, ids: &[i64]) -> AppResult<Vec<CardView>> {
    let sql = format!("SELECT {CARD_COLUMNS} FROM card WHERE id = ?1");
    let mut st = conn.prepare(&sql)?;
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        out.push(st.query_row(params![id], card_from_row)?);
    }
    Ok(out)
}

pub fn defer(conn: &Connection, card_id: i64) -> AppResult<()> {
    conn.execute(
        "UPDATE card SET status = 'deferred', position =
           (SELECT MAX(position) + 1 FROM card c WHERE c.deck_path = card.deck_path AND c.status IN ('in_deck','deferred'))
         WHERE id = ?1",
        params![card_id],
    )?;
    Ok(())
}

#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq)]
pub struct KindCounts {
    pub left: i64,
    pub placed: i64,
}

pub fn counts_by_kind(conn: &Connection, deck: &str) -> AppResult<(KindCounts, KindCounts)> {
    let mut out = (KindCounts::default(), KindCounts::default());
    let mut st = conn.prepare(
        "SELECT kind, SUM(status IN ('in_deck','deferred')), SUM(status = 'placed') FROM card WHERE deck_path = ?1 GROUP BY kind",
    )?;
    let rows = st.query_map(params![deck], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?)))?;
    for row in rows {
        let (kind, left, placed) = row?;
        let slot = if kind == "photo" { &mut out.1 } else { &mut out.0 };
        *slot = KindCounts { left, placed };
    }
    Ok(out)
}

pub fn counts(conn: &Connection, deck: &str) -> AppResult<(i64, i64)> {
    Ok(conn.query_row(
        "SELECT SUM(status IN ('in_deck','deferred')), SUM(status = 'placed') FROM card WHERE deck_path = ?1",
        params![deck],
        |r| Ok((r.get::<_, Option<i64>>(0)?.unwrap_or(0), r.get::<_, Option<i64>>(1)?.unwrap_or(0))),
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use tempfile::TempDir;

    #[test]
    #[ignore]
    fn bench_big_deck() {
        let deck = r"G:\sorter-test\big";
        let t = std::time::Instant::now();
        let n = fs::read_dir(deck).unwrap().filter_map(|e| e.ok()).filter_map(|e| e.metadata().ok()).count();
        println!("read_dir+meta {n}: {:?}", t.elapsed());
        let t = std::time::Instant::now();
        let r = fs::read_dir(deck).unwrap().filter_map(|e| e.ok()).filter(|e| ready(&e.path())).count();
        println!("ready {r}: {:?}", t.elapsed());
        let dir = TempDir::new().unwrap();
        let conn = db::open(&dir.path().join("t.db")).unwrap();
        let t = std::time::Instant::now();
        let s = sync(&conn, deck).unwrap();
        println!("sync {}: {:?}", s.added.len(), t.elapsed());
    }

    #[test]
    fn dates_from_names() {
        assert_eq!(taken_from_name("1525005239349.mp4"), Some(1525005239349));
        assert_eq!(taken_from_name("VID20220216151658.mp4"), Some(1645024618000));
        assert_eq!(taken_from_name("clip.mp4"), None);
    }

    #[test]
    fn sync_keeps_card_with_move_in_flight() {
        let tmp = TempDir::new().unwrap();
        let deck = tmp.path().to_string_lossy().into_owned();
        fs::write(tmp.path().join("a.mp4"), b"a").unwrap();
        let conn = Connection::open_in_memory().unwrap();
        db::init(&conn).unwrap();
        let id = sync(&conn, &deck).unwrap().added[0];
        conn.execute("INSERT INTO pile(id, table_path, name, ord) VALUES (1, 'T', 'P', 0)", []).unwrap();
        conn.execute("INSERT INTO move(id, at, method, pile_id, state) VALUES (1, 0, 'key', 1, 'pending')", []).unwrap();
        conn.execute(
            "INSERT INTO move_item(move_id, card_id, from_path, to_path, step) VALUES (1, ?1, ?2, 'T/P/a.mp4', 'planned')",
            params![id, tmp.path().join("a.mp4").to_string_lossy()],
        )
        .unwrap();
        fs::remove_file(tmp.path().join("a.mp4")).unwrap();
        let r = sync(&conn, &deck).unwrap();
        assert!(r.gone.is_empty() && r.added.is_empty());
        let status: String = conn.query_row("SELECT status FROM card WHERE id = ?1", params![id], |r| r.get(0)).unwrap();
        assert_eq!(status, "in_deck");
    }

    #[test]
    fn sync_reports_found_files() {
        let tmp = TempDir::new().unwrap();
        for i in 0..120 {
            fs::write(tmp.path().join(format!("{i}.mp4")), b"v").unwrap();
        }
        fs::write(tmp.path().join("notes.txt"), b"x").unwrap();
        let conn = Connection::open_in_memory().unwrap();
        db::init(&conn).unwrap();
        let mut seen = Vec::new();
        sync_seen(&conn, &tmp.path().to_string_lossy(), &mut |n| seen.push(n)).unwrap();
        assert_eq!(seen, vec![0, 50, 100, 120]);
    }

    #[test]
    fn sync_adds_updates_and_marks_gone() {
        let tmp = TempDir::new().unwrap();
        let deck = tmp.path().to_string_lossy().into_owned();
        fs::write(tmp.path().join("b.mp4"), b"b").unwrap();
        fs::write(tmp.path().join("a.MP4"), b"a").unwrap();
        fs::write(tmp.path().join("notes.txt"), b"x").unwrap();
        fs::write(tmp.path().join("c.JPG"), b"c").unwrap();
        fs::create_dir(tmp.path().join("sub")).unwrap();
        fs::write(tmp.path().join("sub/c.mp4"), b"c").unwrap();
        let conn = Connection::open_in_memory().unwrap();
        db::init(&conn).unwrap();
        assert_eq!(sync(&conn, &deck).unwrap().added.len(), 3);
        let w = window(&conn, &deck, 0, 10, "all").unwrap();
        assert_eq!(w[0].file_name, "a.MP4");
        assert_eq!((w[2].file_name.as_str(), w[2].kind.as_str()), ("c.JPG", "photo"));
        assert_eq!(window(&conn, &deck, 0, 10, "photo").unwrap().len(), 1);
        assert_eq!(window(&conn, &deck, 0, 10, "video").unwrap().len(), 2);
        defer(&conn, w[0].id).unwrap();
        assert_eq!(window(&conn, &deck, 0, 10, "all").unwrap()[0].file_name, "b.mp4");
        fs::remove_file(tmp.path().join("b.mp4")).unwrap();
        let r = sync(&conn, &deck).unwrap();
        assert_eq!(r.gone.len(), 1);
        assert!(r.added.is_empty());
        assert_eq!(counts(&conn, &deck).unwrap(), (2, 0));
        let (v, p) = counts_by_kind(&conn, &deck).unwrap();
        assert_eq!((v.left, p.left), (1, 1));
        conn.execute(
            "UPDATE card SET series_id = (SELECT id FROM card WHERE file_name = 'a.MP4') WHERE file_name IN ('a.MP4', 'c.JPG')",
            [],
        )
        .unwrap();
        let names: Vec<String> = window(&conn, &deck, 0, 10, "all").unwrap().into_iter().map(|c| c.file_name).collect();
        assert_eq!(names, vec!["a.MP4".to_string()]);
        conn.execute("INSERT INTO pile(id, table_path, name, ord) VALUES (1, 'T', 'P', 0)", []).unwrap();
        conn.execute("INSERT INTO move(id, at, method, pile_id, state) VALUES (1, 0, 'key', 1, 'undoing')", []).unwrap();
        conn.execute(
            "INSERT INTO move_item(move_id, from_path, step) VALUES (1, ?1, 'restoring')",
            params![tmp.path().join("back.mp4").to_string_lossy()],
        )
        .unwrap();
        fs::write(tmp.path().join("back.mp4"), b"b").unwrap();
        let r = sync(&conn, &deck).unwrap();
        assert!(r.added.is_empty() && r.pending);
    }
}
