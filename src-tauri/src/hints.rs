use crate::db;
use crate::deck;
use image::{imageops, RgbImage};
use ort::ep;
use rusqlite::{params, Connection};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use ort::session::Session;
use ort::value::Tensor;
use std::fmt::Display;
use std::fs;
use std::path::{Path, PathBuf};

const ORT_DLL: &[u8] = include_bytes!("../resources/onnxruntime.dll");
const DML_DLL: &[u8] = include_bytes!("../resources/DirectML.dll");
const MODEL: &[u8] = include_bytes!("../resources/dinov2-small-fp16.onnx");

const SIDE: u32 = 224;
const SHORT: u32 = 256;
const MEAN: [f32; 3] = [0.485, 0.456, 0.406];
const STD: [f32; 3] = [0.229, 0.224, 0.225];
const K: usize = 3;
const MIN_EXAMPLES: usize = 3;
const SHOW: f32 = 0.5;
const NEXT: f32 = 0.05;
const FLOOR: f32 = 0.2;
const TEMPERATURE: f32 = 0.05;

fn text<E: Display>(e: E) -> String {
    e.to_string()
}

fn unpack(dir: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    for (name, bytes) in [("onnxruntime.dll", ORT_DLL), ("DirectML.dll", DML_DLL), ("dinov2-small.onnx", MODEL)] {
        let path = dir.join(name);
        if fs::metadata(&path).map(|m| m.len() == bytes.len() as u64).unwrap_or(false) {
            continue;
        }
        let part = dir.join(format!("{name}.part"));
        fs::write(&part, bytes)?;
        fs::rename(&part, &path)?;
    }
    Ok(())
}

pub struct Engine {
    session: Session,
    pub device: &'static str,
}

impl Engine {
    pub fn load(runtime: &Path) -> Result<Engine, String> {
        unpack(runtime).map_err(text)?;
        #[cfg(windows)]
        unsafe {
            let _ = windows::Win32::System::LibraryLoader::LoadLibraryW(&windows::core::HSTRING::from(
                runtime.join("DirectML.dll").as_os_str(),
            ));
        }
        ort::init_from(runtime.join("onnxruntime.dll")).map_err(text)?.commit();
        let model = runtime.join("dinov2-small.onnx");
        let gpu = Session::builder()
            .map_err(text)?
            .with_memory_pattern(false)
            .map_err(text)?
            .with_parallel_execution(false)
            .map_err(text)?
            .with_execution_providers([ep::DirectML::default().build().error_on_failure()])
            .map_err(text)
            .and_then(|mut b| b.commit_from_file(&model).map_err(text));
        match gpu {
            Ok(session) => Ok(Engine { session, device: "gpu" }),
            Err(_) => {
                let session = Session::builder().map_err(text)?.commit_from_file(&model).map_err(text)?;
                Ok(Engine { session, device: "cpu" })
            }
        }
    }

    pub fn embed(&mut self, images: &[RgbImage]) -> Result<Vec<Vec<f32>>, String> {
        if images.is_empty() {
            return Ok(Vec::new());
        }
        let plane = (SIDE * SIDE) as usize;
        let mut data = vec![0f32; images.len() * 3 * plane];
        for (n, img) in images.iter().enumerate() {
            let crop = prepare(img);
            for (i, p) in crop.pixels().enumerate() {
                for c in 0..3 {
                    data[(n * 3 + c) * plane + i] = (p[c] as f32 / 255.0 - MEAN[c]) / STD[c];
                }
            }
        }
        let input = Tensor::from_array(([images.len(), 3, SIDE as usize, SIDE as usize], data)).map_err(text)?;
        let outputs = self.session.run(ort::inputs![input]).map_err(text)?;
        let (shape, out) = outputs[0].try_extract_tensor::<f32>().map_err(text)?;
        let (tokens, width) = (shape[1] as usize, shape[2] as usize);
        Ok((0..images.len())
            .map(|n| {
                let base = n * tokens * width;
                let mut v = vec![0f32; width * 2];
                v[..width].copy_from_slice(&out[base..base + width]);
                for t in 1..tokens {
                    let row = &out[base + t * width..base + (t + 1) * width];
                    for (acc, x) in v[width..].iter_mut().zip(row) {
                        *acc += x / (tokens - 1) as f32;
                    }
                }
                normalize(v)
            })
            .collect())
    }
}

fn prepare(img: &RgbImage) -> RgbImage {
    let (w, h) = img.dimensions();
    let scale = SHORT as f32 / w.min(h) as f32;
    let (nw, nh) = (((w as f32 * scale).round() as u32).max(SIDE), ((h as f32 * scale).round() as u32).max(SIDE));
    let resized = imageops::resize(img, nw, nh, imageops::FilterType::CatmullRom);
    imageops::crop_imm(&resized, (nw - SIDE) / 2, (nh - SIDE) / 2, SIDE, SIDE).to_image()
}

pub fn normalize(mut v: Vec<f32>) -> Vec<f32> {
    let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if n > 0.0 {
        v.iter_mut().for_each(|x| *x /= n);
    }
    v
}

pub fn mean(vectors: &[Vec<f32>]) -> Option<Vec<f32>> {
    let first = vectors.first()?;
    let mut acc = vec![0f32; first.len()];
    for v in vectors {
        acc.iter_mut().zip(v).for_each(|(a, x)| *a += x);
    }
    Some(normalize(acc))
}

pub fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

pub fn to_blob(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

pub fn from_blob(b: &[u8]) -> Vec<f32> {
    b.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()
}

pub fn enabled(conn: &Connection) -> bool {
    db::get_setting(conn, "hints_enabled").ok().flatten().as_deref() != Some("0")
}

pub fn card_frames(cache: &Path, id: i64) -> Vec<RgbImage> {
    (2..6)
        .filter_map(|i| image::open(cache.join(id.to_string()).join(format!("{i}.jpg"))).ok().map(|img| img.to_rgb8()))
        .collect()
}

pub fn to_embed(conn: &Connection, limit: usize) -> rusqlite::Result<Vec<i64>> {
    let deck = db::get_setting(conn, "deck_path")?.unwrap_or_default();
    let mut stmt = conn.prepare(
        "SELECT id FROM card WHERE stage = 'frames' AND status != 'gone'
         ORDER BY (deck_path = ?1 AND status = 'in_deck') DESC, status = 'placed', status = 'deferred', position LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![deck, limit as i64], |r| r.get(0))?;
    rows.collect()
}

pub fn store(conn: &Connection, card_id: i64, vector: Option<&[f32]>) -> rusqlite::Result<()> {
    if let Some(v) = vector {
        conn.execute(
            "INSERT INTO embedding(card_id, vector) VALUES (?1, ?2) ON CONFLICT(card_id) DO UPDATE SET vector = excluded.vector",
            params![card_id, to_blob(v)],
        )?;
    }
    conn.execute("UPDATE card SET stage = 'embedded' WHERE id = ?1 AND stage = 'frames'", params![card_id])?;
    Ok(())
}

pub fn learn(conn: &Connection) -> rusqlite::Result<usize> {
    conn.execute(
        "INSERT INTO example(card_id, pile_id, path, vector)
         SELECT c.id, c.pile_id, c.current_path, e.vector FROM card c
         JOIN embedding e ON e.card_id = c.id
         JOIN pile p ON p.id = c.pile_id
         WHERE c.status = 'placed' AND p.is_trash = 0 AND c.current_path IS NOT NULL
           AND NOT EXISTS (SELECT 1 FROM example x WHERE x.card_id = c.id AND x.pile_id = c.pile_id)",
        [],
    )
}

pub fn prune(conn: &Connection) -> rusqlite::Result<usize> {
    conn.execute(
        "DELETE FROM example WHERE card_id IS NULL AND id NOT IN (SELECT MIN(id) FROM example WHERE card_id IS NULL GROUP BY path, pile_id)",
        [],
    )?;
    conn.execute(
        "DELETE FROM example WHERE card_id IS NULL AND path IN (SELECT current_path FROM card WHERE current_path IS NOT NULL)",
        [],
    )?;
    conn.execute(
        "DELETE FROM example WHERE card_id IS NOT NULL AND NOT EXISTS (
           SELECT 1 FROM card c WHERE c.id = example.card_id AND c.status = 'placed' AND c.pile_id = example.pile_id)",
        [],
    )
}

pub fn cold_files(conn: &Connection) -> rusqlite::Result<Vec<(i64, PathBuf)>> {
    let table = db::get_setting(conn, "table_path")?.unwrap_or_default();
    let known: HashSet<String> = conn
        .prepare("SELECT path FROM example UNION SELECT current_path FROM card WHERE current_path IS NOT NULL")?
        .query_map([], |r| r.get::<_, String>(0))?
        .map(|p| p.map(|p| p.to_lowercase()))
        .collect::<Result<_, _>>()?;
    let piles: Vec<(i64, String)> = conn
        .prepare("SELECT id, name FROM pile WHERE table_path = ?1 AND is_trash = 0 AND exists_on_disk = 1 ORDER BY ord")?
        .query_map(params![table], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    let mut out = Vec::new();
    for (id, name) in piles {
        let Ok(dir) = fs::read_dir(Path::new(&table).join(&name)) else { continue };
        for path in dir.filter_map(|e| e.ok().map(|e| e.path())) {
            if deck::is_video(&path) && !known.contains(&path.to_string_lossy().to_lowercase()) {
                out.push((id, path));
            }
        }
    }
    Ok(out)
}

pub fn add_cold(conn: &Connection, pile_id: i64, path: &Path, vector: &[f32]) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO example(card_id, pile_id, path, vector) VALUES (NULL, ?1, ?2, ?3)",
        params![pile_id, path.to_string_lossy(), to_blob(vector)],
    )?;
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hint {
    pile_id: i64,
    score: f32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HintsView {
    enabled: bool,
    ready: bool,
    examples: i64,
    hints: Vec<Hint>,
}

pub fn suggest(conn: &Connection, card_ids: &[i64]) -> rusqlite::Result<HintsView> {
    let enabled = enabled(conn);
    let table = db::get_setting(conn, "table_path")?.unwrap_or_default();
    let examples: i64 = conn.query_row(
        "SELECT COUNT(*) FROM example x JOIN pile p ON p.id = x.pile_id WHERE p.table_path = ?1 AND p.is_trash = 0 AND p.exists_on_disk = 1",
        params![table],
        |r| r.get(0),
    )?;
    let mut vectors = Vec::new();
    for id in card_ids {
        let v: Option<Vec<u8>> = conn
            .query_row("SELECT vector FROM embedding WHERE card_id = ?1", params![id], |r| r.get(0))
            .ok();
        if let Some(v) = v {
            vectors.push(from_blob(&v));
        }
    }
    let ready = enabled && !card_ids.is_empty() && vectors.len() == card_ids.len();
    let mut hints = Vec::new();
    if let (true, Some(q)) = (ready, mean(&vectors)) {
        let mut stmt = conn.prepare(
            "SELECT x.pile_id, x.vector FROM example x JOIN pile p ON p.id = x.pile_id
             WHERE p.table_path = ?1 AND p.is_trash = 0 AND p.exists_on_disk = 1",
        )?;
        let mut by_pile: HashMap<i64, Vec<f32>> = HashMap::new();
        for row in stmt.query_map(params![table], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Vec<u8>>(1)?)))? {
            let (pile, blob) = row?;
            by_pile.entry(pile).or_default().push(dot(&q, &from_blob(&blob)));
        }
        let mut ranked: Vec<(i64, f32)> = by_pile
            .into_iter()
            .filter(|(_, s)| s.len() >= MIN_EXAMPLES)
            .map(|(pile, mut s)| {
                s.sort_by(|a, b| b.total_cmp(a));
                (pile, s.iter().take(K).sum::<f32>() / K as f32)
            })
            .collect();
        ranked.sort_by(|a, b| b.1.total_cmp(&a.1));
        if let Some(&(_, top)) = ranked.first() {
            let total: f32 = ranked.iter().map(|(_, s)| ((s - top) / TEMPERATURE).exp()).sum();
            let prob = |s: f32| ((s - top) / TEMPERATURE).exp() / total;
            if top >= FLOOR && prob(top) >= SHOW {
                hints = ranked
                    .iter()
                    .take(2)
                    .map(|&(pile_id, s)| Hint { pile_id, score: prob(s) })
                    .filter(|h| h.score >= NEXT)
                    .collect();
            }
        }
    }
    Ok(HintsView { enabled, ready, examples, hints })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn axis(i: usize, noise: f32) -> Vec<f32> {
        let mut v = vec![noise; 8];
        v[i] = 1.0;
        normalize(v)
    }

    fn fixture() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        db::init(&conn).unwrap();
        db::set_setting(&conn, "table_path", "T").unwrap();
        conn.execute_batch(
            "INSERT INTO pile(id, table_path, name, ord) VALUES (1, 'T', 'Кошки', 0), (2, 'T', 'Машины', 1), (4, 'T', 'Мало', 3);
             INSERT INTO pile(id, table_path, name, ord, is_trash) VALUES (3, 'T', ':trash', 2, 1);
             INSERT INTO card(id, deck_path, file_name, size, mtime, position) VALUES (10, 'D', 'a.mp4', 1, 0, 1), (11, 'D', 'b.mp4', 1, 0, 2);",
        )
        .unwrap();
        for (pile, axis_i, n) in [(1, 0, 4), (2, 1, 3), (3, 2, 5), (4, 3, 2)] {
            for k in 0..n {
                add_cold(&conn, pile, Path::new(&format!("T/{pile}/{k}.mp4")), &axis(axis_i, 0.05 * k as f32)).unwrap();
            }
        }
        conn
    }

    #[test]
    fn suggests_nearest_pile_and_skips_trash_and_small() {
        let conn = fixture();
        store(&conn, 10, Some(&axis(0, 0.1))).unwrap();
        let view = suggest(&conn, &[10]).unwrap();
        assert!(view.ready);
        assert_eq!(view.examples, 9);
        assert_eq!(view.hints.len(), 1);
        assert_eq!(view.hints[0].pile_id, 1);

        store(&conn, 11, Some(&axis(2, 0.0))).unwrap();
        assert!(suggest(&conn, &[11]).unwrap().hints.is_empty());

        conn.execute("UPDATE card SET stage = 'frames' WHERE id = 10", []).unwrap();
        let none = suggest(&conn, &[10, 12]).unwrap();
        assert!(!none.ready && none.hints.is_empty());
    }

    #[test]
    fn cold_scan_skips_known_cyrillic_paths() {
        let dir = tempfile::tempdir().unwrap();
        let conn = Connection::open_in_memory().unwrap();
        db::init(&conn).unwrap();
        db::set_setting(&conn, "table_path", &dir.path().to_string_lossy()).unwrap();
        fs::create_dir(dir.path().join("Друзья")).unwrap();
        fs::write(dir.path().join("Друзья").join("a.mp4"), b"x").unwrap();
        conn.execute("INSERT INTO pile(id, table_path, name, ord) VALUES (1, ?1, 'Друзья', 0)", params![dir.path().to_string_lossy()]).unwrap();
        let files = cold_files(&conn).unwrap();
        assert_eq!(files.len(), 1);
        add_cold(&conn, 1, &files[0].1, &axis(0, 0.0)).unwrap();
        assert!(cold_files(&conn).unwrap().is_empty());
        add_cold(&conn, 1, &files[0].1, &axis(0, 0.0)).unwrap();
        prune(&conn).unwrap();
        assert_eq!(conn.query_row("SELECT COUNT(*) FROM example", [], |r| r.get::<_, i64>(0)).unwrap(), 1);
    }

    #[test]
    fn learns_from_moves_and_forgets_on_undo() {
        let conn = fixture();
        store(&conn, 10, Some(&axis(3, 0.0))).unwrap();
        assert!(suggest(&conn, &[10]).unwrap().hints.is_empty());
        conn.execute("UPDATE card SET status = 'placed', pile_id = 4, current_path = 'T/4/a.mp4' WHERE id = 10", []).unwrap();
        assert_eq!(learn(&conn).unwrap(), 1);
        assert_eq!(learn(&conn).unwrap(), 0);
        store(&conn, 11, Some(&axis(3, 0.02))).unwrap();
        assert_eq!(suggest(&conn, &[11]).unwrap().hints[0].pile_id, 4);
        conn.execute("UPDATE card SET status = 'in_deck', pile_id = NULL, current_path = NULL WHERE id = 10", []).unwrap();
        assert_eq!(prune(&conn).unwrap(), 1);
        assert!(suggest(&conn, &[11]).unwrap().hints.is_empty());
    }

    #[test]
    #[ignore]
    fn embeds_cached_frames() {
        let cache = Path::new(&std::env::var("APPDATA").unwrap()).join(r"com.stillmvd.sorter\cache");
        let runtime = std::env::temp_dir().join("sorter-runtime-test");
        let t = Instant::now();
        let mut engine = Engine::load(&runtime).unwrap();
        println!("загрузка {} ms, устройство {}", t.elapsed().as_millis(), engine.device);
        let dirs: Vec<_> = fs::read_dir(&cache).unwrap().filter_map(|e| e.ok().map(|e| e.path())).take(6).collect();
        let mut cards = Vec::new();
        for d in &dirs {
            let frames: Vec<RgbImage> = (2..6).filter_map(|i| image::open(d.join(format!("{i}.jpg"))).ok().map(|i| i.to_rgb8())).collect();
            let t = Instant::now();
            let vs = engine.embed(&frames).unwrap();
            println!("{:?}: {} кадра за {} ms", d.file_name().unwrap(), vs.len(), t.elapsed().as_millis());
            cards.push(mean(&vs).unwrap());
        }
        for (i, a) in cards.iter().enumerate() {
            let row: Vec<String> = cards.iter().map(|b| format!("{:.2}", dot(a, b))).collect();
            println!("{i}: {}", row.join(" "));
        }
        assert_eq!(cards[0].len(), 768);
    }
}
