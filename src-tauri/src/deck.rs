use crate::error::{io_error, AppResult};
use rusqlite::{params, Connection, Row};
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub const VIDEO_EXT: [&str; 6] = ["mp4", "mov", "mkv", "webm", "avi", "m4v"];

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
}

pub const CARD_COLUMNS: &str =
    "id, deck_path, file_name, size, taken_at, duration_ms, width, height, orientation, frames, stage, error, status";

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
    })
}

pub fn is_video(path: &Path) -> bool {
    path.is_file()
        && !path.file_name().map(|n| n.to_string_lossy().starts_with('.')).unwrap_or(true)
        && path
            .extension()
            .map(|e| VIDEO_EXT.contains(&e.to_string_lossy().to_lowercase().as_str()))
            .unwrap_or(false)
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
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
}

pub fn sync(conn: &Connection, deck: &str) -> AppResult<SyncResult> {
    let dir = Path::new(deck);
    let mut files: Vec<(String, i64, i64)> = fs::read_dir(dir)
        .map_err(|e| io_error(&e, dir))?
        .filter_map(|e| e.ok())
        .filter(|e| is_video(&e.path()))
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            Some((e.file_name().to_string_lossy().into_owned(), meta.len() as i64, mtime_ms(&meta)))
        })
        .collect();
    files.sort_by_key(|f| f.0.to_lowercase());

    let rows: HashMap<String, (i64, i64, i64)> = conn
        .prepare("SELECT id, file_name, size, mtime FROM card WHERE deck_path = ?1 AND status IN ('in_deck','deferred')")?
        .query_map(params![deck], |r| Ok((r.get::<_, String>(1)?.to_lowercase(), (r.get(0)?, r.get(2)?, r.get(3)?))))?
        .collect::<Result<_, _>>()?;

    let present: HashMap<String, ()> = files.iter().map(|f| (f.0.to_lowercase(), ())).collect();
    let mut gone = Vec::new();
    for (name, (id, _, _)) in &rows {
        if !present.contains_key(name) {
            conn.execute("UPDATE card SET status = 'gone' WHERE id = ?1", params![id])?;
            gone.push(*id);
        }
    }

    let mut added = Vec::new();
    for (name, size, mtime) in files {
        match rows.get(&name.to_lowercase()) {
            Some((id, s, m)) if (*s, *m) != (size, mtime) => {
                conn.execute(
                    "UPDATE card SET size = ?2, mtime = ?3, stage = 'new', frames = 0, error = NULL WHERE id = ?1",
                    params![id, size, mtime],
                )?;
            }
            Some(_) => {}
            None => {
                conn.execute(
                    "INSERT INTO card(deck_path, file_name, size, mtime, taken_at, position)
                     VALUES (?1, ?2, ?3, ?4, ?5,
                       (SELECT COALESCE(MAX(position), 0) + 1 FROM card WHERE deck_path = ?1 AND status IN ('in_deck','deferred')))",
                    params![deck, name, size, mtime, taken_from_name(&name).unwrap_or(mtime)],
                )?;
                added.push(conn.last_insert_rowid());
            }
        }
    }
    Ok(SyncResult { added, gone })
}

pub fn window(conn: &Connection, deck: &str, from: i64, count: i64) -> AppResult<Vec<CardView>> {
    let sql = format!(
        "SELECT {CARD_COLUMNS} FROM card WHERE deck_path = ?1 AND status IN ('in_deck','deferred')
         ORDER BY position LIMIT ?2 OFFSET ?3"
    );
    let cards = conn
        .prepare(&sql)?
        .query_map(params![deck, count, from], card_from_row)?
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
    fn dates_from_names() {
        assert_eq!(taken_from_name("1525005239349.mp4"), Some(1525005239349));
        assert_eq!(taken_from_name("VID20220216151658.mp4"), Some(1645024618000));
        assert_eq!(taken_from_name("clip.mp4"), None);
    }

    #[test]
    fn sync_adds_updates_and_marks_gone() {
        let tmp = TempDir::new().unwrap();
        let deck = tmp.path().to_string_lossy().into_owned();
        fs::write(tmp.path().join("b.mp4"), b"b").unwrap();
        fs::write(tmp.path().join("a.MP4"), b"a").unwrap();
        fs::write(tmp.path().join("notes.txt"), b"x").unwrap();
        fs::create_dir(tmp.path().join("sub")).unwrap();
        fs::write(tmp.path().join("sub/c.mp4"), b"c").unwrap();
        let conn = Connection::open_in_memory().unwrap();
        db::init(&conn).unwrap();
        assert_eq!(sync(&conn, &deck).unwrap().added.len(), 2);
        let w = window(&conn, &deck, 0, 10).unwrap();
        assert_eq!(w[0].file_name, "a.MP4");
        defer(&conn, w[0].id).unwrap();
        assert_eq!(window(&conn, &deck, 0, 10).unwrap()[0].file_name, "b.mp4");
        fs::remove_file(tmp.path().join("b.mp4")).unwrap();
        let r = sync(&conn, &deck).unwrap();
        assert_eq!(r.gone.len(), 1);
        assert!(r.added.is_empty());
        assert_eq!(counts(&conn, &deck).unwrap(), (1, 0));
    }
}
