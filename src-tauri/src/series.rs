use crate::deck::{self, CardView, CARD_COLUMNS};
use crate::error::AppResult;
use rusqlite::{params, Connection};
use serde::Serialize;
use std::collections::BTreeMap;

pub const GAP_MS: i64 = 10_000;
const HAM_COPY: u32 = 2;
const HAM: u32 = 18;
const HAM_FILE: u32 = 10;
const CORR: f32 = 0.8;
const CORR_FILE: f32 = 0.9;

#[derive(Debug, Clone, Serialize)]
pub struct SeriesView {
    pub id: i64,
    pub best: i64,
    pub cards: Vec<CardView>,
}

struct Shot {
    id: i64,
    taken: i64,
    from_file: bool,
    phash: u64,
    look: Vec<u8>,
}

fn linked(a: &Shot, b: &Shot) -> bool {
    let ham = (a.phash ^ b.phash).count_ones();
    let strict = a.from_file && b.from_file;
    let (limit, corr) = if strict { (HAM_FILE, CORR_FILE) } else { (HAM, CORR) };
    (b.taken - a.taken).abs() <= GAP_MS && ham > HAM_COPY && (ham <= limit || crate::photo::look_corr(&a.look, &b.look) >= corr)
}

fn chains(shots: &[Shot]) -> Vec<Vec<i64>> {
    let mut out: Vec<Vec<i64>> = Vec::new();
    let mut run: Vec<i64> = Vec::new();
    for (i, s) in shots.iter().enumerate() {
        if i > 0 && linked(&shots[i - 1], s) {
            run.push(s.id);
        } else {
            if run.len() >= 2 {
                out.push(std::mem::take(&mut run));
            }
            run = vec![s.id];
        }
    }
    if run.len() >= 2 {
        out.push(run);
    }
    out
}

pub fn regroup(conn: &Connection, deck: &str) -> rusqlite::Result<bool> {
    let shots: Vec<Shot> = conn
        .prepare(
            "SELECT id, taken_at, taken_from, phash, look FROM card
             WHERE deck_path = ?1 AND status IN ('in_deck','deferred') AND kind = 'photo' AND solo = 0
               AND phash IS NOT NULL AND taken_at IS NOT NULL
             ORDER BY taken_at, id",
        )?
        .query_map(params![deck], |r| {
            Ok(Shot {
                id: r.get(0)?,
                taken: r.get(1)?,
                from_file: r.get::<_, Option<String>>(2)?.as_deref() == Some("file"),
                phash: r.get::<_, i64>(3)? as u64,
                look: r.get::<_, Option<Vec<u8>>>(4)?.unwrap_or_default(),
            })
        })?
        .collect::<Result<_, _>>()?;
    let mut wanted: BTreeMap<i64, i64> = BTreeMap::new();
    for chain in chains(&shots) {
        for id in &chain {
            wanted.insert(*id, chain[0]);
        }
    }
    let current: BTreeMap<i64, i64> = conn
        .prepare("SELECT id, series_id FROM card WHERE deck_path = ?1 AND series_id IS NOT NULL")?
        .query_map(params![deck], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    if current == wanted {
        return Ok(false);
    }
    let tx = conn.unchecked_transaction()?;
    tx.execute("UPDATE card SET series_id = NULL WHERE deck_path = ?1 AND series_id IS NOT NULL", params![deck])?;
    for (id, series) in &wanted {
        tx.execute("UPDATE card SET series_id = ?2 WHERE id = ?1", params![id, series])?;
    }
    tx.commit()?;
    Ok(true)
}

pub fn list(conn: &Connection, deck: &str) -> AppResult<Vec<SeriesView>> {
    let sql = format!(
        "SELECT {CARD_COLUMNS}, sharp FROM card WHERE deck_path = ?1 AND series_id IS NOT NULL AND status IN ('in_deck','deferred')
         ORDER BY series_id, taken_at, id"
    );
    let rows: Vec<(CardView, f64)> = conn
        .prepare(&sql)?
        .query_map(params![deck], |r| Ok((deck::card_from_row(r)?, r.get::<_, Option<f64>>(17)?.unwrap_or(0.0))))?
        .collect::<Result<_, _>>()?;
    let mut out: Vec<SeriesView> = Vec::new();
    let mut score = f64::MIN;
    for (card, sharp) in rows {
        let series = card.series_id.unwrap_or(card.id);
        let quality = sharp * ((card.width.unwrap_or(0) * card.height.unwrap_or(0)) as f64).sqrt();
        match out.last_mut() {
            Some(s) if s.id == series => {
                if quality > score {
                    score = quality;
                    s.best = card.id;
                }
                s.cards.push(card);
            }
            _ => {
                score = quality;
                out.push(SeriesView { id: series, best: card.id, cards: vec![card] });
            }
        }
    }
    Ok(out)
}

pub fn members(conn: &Connection, series_id: i64) -> rusqlite::Result<Vec<i64>> {
    conn.prepare("SELECT id FROM card WHERE series_id = ?1 AND status IN ('in_deck','deferred') ORDER BY taken_at, id")?
        .query_map(params![series_id], |r| r.get(0))?
        .collect()
}

pub fn split(conn: &Connection, series_id: i64) -> rusqlite::Result<Vec<i64>> {
    let ids = members(conn, series_id)?;
    conn.execute("UPDATE card SET solo = 1, series_id = NULL WHERE series_id = ?1", params![series_id])?;
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn fixture(rows: &[(&str, i64, &str, u64)]) -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        db::init(&conn).unwrap();
        for (i, (name, taken, from, phash)) in rows.iter().enumerate() {
            conn.execute(
                "INSERT INTO card(deck_path, file_name, size, mtime, position, kind, taken_at, taken_from, phash, sharp, width, height)
                 VALUES ('D', ?1, 1, 0, ?2, 'photo', ?3, ?4, ?5, ?6, 100, 100)",
                params![name, i as f64, taken, from, *phash as i64, if *name == "b" { 9.0 } else { 1.0 }],
            )
            .unwrap();
        }
        conn
    }

    fn series(conn: &Connection) -> Vec<Vec<String>> {
        list(conn, "D").unwrap().into_iter().map(|s| s.cards.into_iter().map(|c| c.file_name).collect()).collect()
    }

    #[test]
    fn chains_close_similar_shots_only() {
        let near = 0b1111_1111u64;
        let conn = fixture(&[
            ("a", 0, "exif", 0),
            ("b", 2_000, "exif", near),
            ("c", 5_000, "exif", 0),
            ("d", 40_000, "exif", 0),
            ("e", 41_000, "exif", u64::MAX),
            ("f", 60_000, "exif", 0),
            ("g", 61_000, "exif", 1),
        ]);
        assert!(regroup(&conn, "D").unwrap());
        assert!(!regroup(&conn, "D").unwrap());
        assert_eq!(series(&conn), vec![vec!["a", "b", "c"]]);
        let s = &list(&conn, "D").unwrap()[0];
        assert_eq!(s.cards.iter().find(|c| c.id == s.best).unwrap().file_name, "b");
        let names: Vec<String> = deck::window(&conn, "D", 0, 10, "all").unwrap().into_iter().map(|c| c.file_name).collect();
        assert_eq!(names, vec!["a", "d", "e", "f", "g"]);
    }

    #[test]
    #[ignore]
    fn series_on_set() {
        let dir = std::path::Path::new(r"G:\sorter-test\photos");
        let Ok(manifest) = std::fs::read_to_string(dir.join("manifest.json")) else { return };
        let manifest: serde_json::Value = serde_json::from_str(&manifest).unwrap();
        let conn = Connection::open_in_memory().unwrap();
        db::init(&conn).unwrap();
        let mut names: Vec<String> =
            std::fs::read_dir(dir).unwrap().filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        for (i, name) in names.iter().enumerate() {
            let Ok(m) = crate::photo::read(&dir.join(name)) else { continue };
            let (taken, from) = match m.taken {
                Some(t) => (t, "exif"),
                None => (i as i64 * 60_000_000, "file"),
            };
            conn.execute(
                "INSERT INTO card(deck_path, file_name, size, mtime, position, kind, taken_at, taken_from, phash, sharp, width, height, look)
                 VALUES ('D', ?1, 1, 0, ?2, 'photo', ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![name, i as f64, taken, from, m.phash as i64, m.sharp, m.width, m.height, m.look],
            )
            .unwrap();
        }
        regroup(&conn, "D").unwrap();
        let mut got = series(&conn);
        got.iter_mut().for_each(|s| s.sort());
        let mut want: Vec<Vec<String>> = manifest["series"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| {
                let mut v: Vec<String> = s.as_array().unwrap().iter().map(|n| n.as_str().unwrap().to_string()).collect();
                v.sort();
                v
            })
            .collect();
        got.sort();
        want.sort();
        assert_eq!(got, want);
    }

    #[test]
    fn file_dates_need_closer_match_and_split_sticks() {
        let mid = 0b1111_1111_1111u64;
        let conn = fixture(&[("a", 0, "file", 0), ("b", 1_000, "file", mid), ("c", 30_000, "exif", 0), ("d", 31_000, "exif", mid)]);
        regroup(&conn, "D").unwrap();
        assert_eq!(series(&conn), vec![vec!["c", "d"]]);
        let id = list(&conn, "D").unwrap()[0].id;
        assert_eq!(split(&conn, id).unwrap().len(), 2);
        regroup(&conn, "D").unwrap();
        assert!(series(&conn).is_empty());
    }
}
