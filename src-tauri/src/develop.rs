use crate::db;
use crate::deck::{self, CardView};
use crate::dupes;
use crate::hints::{self, Engine};
use std::collections::HashSet;
use rusqlite::{params, Connection};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

pub const FRAMES: u32 = 8;
const THUMB: u32 = 320;

pub struct Meta {
    pub duration_ms: i64,
    pub width: u32,
    pub height: u32,
    pub frames: Vec<image::RgbImage>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Progress {
    done: i64,
    total: i64,
    paused: bool,
    printed: i64,
}

#[cfg(windows)]
mod mf {
    use super::{Meta, FRAMES, THUMB};
    use image::{imageops, RgbImage};
    use std::path::Path;
    use windows::core::{GUID, HSTRING};
    use windows::Win32::Media::MediaFoundation::*;
    use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
    use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

    const VIDEO: u32 = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;

    pub fn start() {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let _ = MFStartup(MF_VERSION, MFSTARTUP_FULL);
        }
    }

    pub fn read(path: &Path) -> windows::core::Result<Meta> {
        unsafe {
            let mut attrs = None;
            MFCreateAttributes(&mut attrs, 1)?;
            let attrs = attrs.unwrap();
            attrs.SetUINT32(&MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING, 1)?;
            let reader = MFCreateSourceReaderFromURL(&HSTRING::from(path.as_os_str()), &attrs)?;
            reader.SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false)?;
            reader.SetStreamSelection(VIDEO, true)?;

            let native = reader.GetNativeMediaType(VIDEO, 0)?;
            let rotation = native.GetUINT32(&MF_MT_VIDEO_ROTATION).unwrap_or(0);
            let size = native.GetUINT64(&MF_MT_FRAME_SIZE)?;
            let (nw, nh) = ((size >> 32) as u32, (size & 0xFFFF_FFFF) as u32);

            let wanted = MFCreateMediaType()?;
            wanted.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
            wanted.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32)?;
            let scale = (THUMB as f32 / nw.max(nh).max(1) as f32).min(1.0);
            let tw = ((nw as f32 * scale).round() as u32).max(2) & !1;
            let th = ((nh as f32 * scale).round() as u32).max(2) & !1;
            wanted.SetUINT64(&MF_MT_FRAME_SIZE, (tw as u64) << 32 | th as u64)?;
            if reader.SetCurrentMediaType(VIDEO, None, &wanted).is_err() {
                wanted.DeleteItem(&MF_MT_FRAME_SIZE)?;
                reader.SetCurrentMediaType(VIDEO, None, &wanted)?;
            }
            let current = reader.GetCurrentMediaType(VIDEO)?;
            let size = current.GetUINT64(&MF_MT_FRAME_SIZE)?;
            let (w, h) = ((size >> 32) as u32, (size & 0xFFFF_FFFF) as u32);
            let stride = current.GetUINT32(&MF_MT_DEFAULT_STRIDE).map(|s| s as i32).unwrap_or((w * 4) as i32);

            let duration = reader
                .GetPresentationAttribute(MF_SOURCE_READER_MEDIASOURCE.0 as u32, &MF_PD_DURATION)
                .ok()
                .and_then(|v| u64::try_from(&v).ok())
                .unwrap_or(0) as i64;

            let mut frames = Vec::new();
            for i in 0..FRAMES {
                if duration > 0 {
                    let at = duration * (2 * i as i64 + 1) / (2 * FRAMES as i64);
                    let pos = PROPVARIANT::from(at);
                    if reader.SetCurrentPosition(&GUID::zeroed(), &pos).is_err() {
                        break;
                    }
                } else if i > 0 {
                    break;
                }
                let mut sample = None;
                let mut flags = 0u32;
                for _ in 0..30 {
                    reader.ReadSample(VIDEO, 0, None, Some(&mut flags), None, Some(&mut sample))?;
                    if sample.is_some() || flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                        break;
                    }
                }
                let Some(sample) = sample else { break };
                let buffer = sample.ConvertToContiguousBuffer()?;
                let mut ptr = std::ptr::null_mut();
                let mut len = 0u32;
                buffer.Lock(&mut ptr, None, Some(&mut len))?;
                let data = std::slice::from_raw_parts(ptr, len as usize);
                let abs = stride.unsigned_abs() as usize;
                let mut raw = Vec::with_capacity(w as usize * h as usize * 3);
                for y in 0..h as usize {
                    let row = if stride < 0 { h as usize - 1 - y } else { y };
                    let start = row * abs;
                    let Some(line) = data.get(start..start + w as usize * 4) else { break };
                    raw.extend(line.chunks_exact(4).flat_map(|p| [p[2], p[1], p[0]]));
                }
                buffer.Unlock()?;
                raw.resize(w as usize * h as usize * 3, 0);
                let img = RgbImage::from_raw(w, h, raw).unwrap();
                let mut small = if w.max(h) > THUMB {
                    let scale = THUMB as f32 / w.max(h) as f32;
                    imageops::resize(
                        &img,
                        ((w as f32 * scale).round() as u32).max(1),
                        ((h as f32 * scale).round() as u32).max(1),
                        imageops::FilterType::Triangle,
                    )
                } else {
                    img
                };
                small = match rotation {
                    90 => imageops::rotate90(&small),
                    180 => imageops::rotate180(&small),
                    270 => imageops::rotate270(&small),
                    _ => small,
                };
                frames.push(small);
            }
            let (width, height) = if rotation == 90 || rotation == 270 { (nh, nw) } else { (nw, nh) };
            Ok(Meta { duration_ms: duration / 10_000, width, height, frames })
        }
    }
}

pub fn orientation(w: u32, h: u32) -> &'static str {
    let (w, h) = (w as f64, h as f64);
    if h > w * 1.05 {
        "portrait"
    } else if w > h * 1.05 {
        "landscape"
    } else {
        "square"
    }
}

fn next(conn: &Connection, limit: usize) -> rusqlite::Result<Vec<(i64, PathBuf)>> {
    let deck = db::get_setting(conn, "deck_path")?.unwrap_or_default();
    let mut stmt = conn.prepare(
        "SELECT id, deck_path, file_name, status, current_path FROM card
         WHERE stage = 'new' AND status != 'gone' AND (current_path IS NOT NULL OR status != 'placed')
         ORDER BY (deck_path = ?1 AND status = 'in_deck') DESC, status = 'deferred', position LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![deck, limit as i64], |r| {
        let (id, d, name, status, current): (i64, String, String, String, Option<String>) =
            (r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?);
        let path = match (status.as_str(), current) {
            ("placed", Some(c)) => PathBuf::from(c),
            _ => Path::new(&d).join(name),
        };
        Ok((id, path))
    })?;
    rows.collect()
}

fn progress(conn: &Connection, paused: bool) -> rusqlite::Result<Progress> {
    let deck = db::get_setting(conn, "deck_path")?.unwrap_or_default();
    let (printed, _) = dupes::printed(conn)?;
    conn.query_row(
        "SELECT SUM(stage != 'new'), COUNT(*) FROM card WHERE deck_path = ?1 AND status IN ('in_deck','deferred')",
        params![deck],
        |r| Ok(Progress { done: r.get::<_, Option<i64>>(0)?.unwrap_or(0), total: r.get(1)?, paused, printed }),
    )
}

struct Developed {
    meta: Meta,
    saved: u32,
}

fn workers() -> usize {
    std::thread::available_parallelism().map(|n| n.get() / 2).unwrap_or(2).clamp(2, 4)
}

fn develop_files(cache: &Path, id: i64, path: &Path) -> Option<Developed> {
    #[cfg(windows)]
    let meta = mf::read(path).ok()?;
    #[cfg(not(windows))]
    let meta: Meta = return None;
    if meta.frames.is_empty() {
        return None;
    }
    let dir = cache.join(id.to_string());
    let _ = fs::create_dir_all(&dir);
    let mut saved = 0;
    for (i, frame) in meta.frames.iter().enumerate() {
        let file = dir.join(format!("{i}.jpg"));
        let ok = fs::File::create(&file).ok().and_then(|f| {
            let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(std::io::BufWriter::new(f), 80);
            enc.encode_image(frame).ok()
        });
        if ok.is_some() {
            saved += 1;
        }
    }
    Some(Developed { meta, saved })
}

fn develop_batch(cache: &Path, jobs: &[(i64, PathBuf)]) -> Vec<Option<Developed>> {
    std::thread::scope(|scope| {
        let handles: Vec<_> = jobs
            .iter()
            .map(|(id, path)| {
                scope.spawn(move || {
                    #[cfg(windows)]
                    mf::start();
                    develop_files(cache, *id, path)
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().ok().flatten()).collect()
    })
}

fn save(conn: &Connection, id: i64, result: Option<Developed>) -> rusqlite::Result<()> {
    match result {
        Some(Developed { meta, saved }) => conn.execute(
            "UPDATE card SET duration_ms = ?2, width = ?3, height = ?4, orientation = ?5, frames = ?6, stage = 'frames', error = NULL WHERE id = ?1",
            params![id, meta.duration_ms, meta.width, meta.height, orientation(meta.width, meta.height), saved],
        ),
        None => conn.execute(
            "UPDATE card SET stage = 'broken', error = ?2 WHERE id = ?1",
            params![id, "Не удалось прочитать кадры этого видео — разложить его всё равно можно."],
        ),
    }?;
    Ok(())
}

struct Learner {
    engine: Option<Engine>,
    failed: bool,
    cold: Vec<(i64, PathBuf)>,
    cold_at: Option<Instant>,
    skip: HashSet<PathBuf>,
}

impl Learner {
    fn new() -> Self {
        Learner { engine: None, failed: false, cold: Vec::new(), cold_at: None, skip: HashSet::new() }
    }

    fn engine(&mut self, cache: &Path) -> Option<&mut Engine> {
        if self.engine.is_none() && !self.failed {
            match Engine::load(&cache.parent().unwrap_or(cache).join("runtime")) {
                Ok(e) => {
                    eprintln!("hints: движок на {}", e.device);
                    self.engine = Some(e);
                }
                Err(e) => {
                    eprintln!("hints: {e}");
                    self.failed = true;
                }
            }
        }
        self.engine.as_mut()
    }

    fn step(&mut self, app: &AppHandle, conn: &Connection, cache: &Path) -> bool {
        if self.failed || !hints::enabled(conn) {
            return false;
        }
        let ids = hints::to_embed(conn, 4).unwrap_or_default();
        let cold = if ids.is_empty() {
            if self.cold.is_empty() && self.cold_at.is_none_or(|t| t.elapsed() > Duration::from_secs(60)) {
                self.cold = hints::cold_files(conn).unwrap_or_default();
                self.cold.retain(|(_, p)| !self.skip.contains(p));
                self.cold.reverse();
                self.cold_at = Some(Instant::now());
            }
            self.cold.pop()
        } else {
            None
        };
        if ids.is_empty() && cold.is_none() {
            return false;
        }
        let Some(engine) = self.engine(cache) else { return false };
        for id in &ids {
            let frames = hints::card_frames(cache, *id);
            let vector = engine.embed(&frames).ok().and_then(|vs| hints::mean(&vs));
            if hints::store(conn, *id, vector.as_deref()).is_ok() {
                if let Ok(cards) = deck::cards(conn, &[*id]) {
                    let _ = app.emit("develop://card", cards.first());
                }
            }
        }
        if let Some((pile, path)) = cold {
            let vector = read_frames(&path)
                .and_then(|frames| engine.embed(&frames[2.min(frames.len())..6.min(frames.len())]).ok())
                .and_then(|vs| hints::mean(&vs));
            match vector {
                Some(v) => {
                    let _ = hints::add_cold(conn, pile, &path, &v);
                }
                None => {
                    self.skip.insert(path);
                }
            }
        }
        let _ = hints::learn(conn);
        let _ = app.emit("hints://changed", ());
        true
    }
}

pub fn read_frames(path: &Path) -> Option<Vec<image::RgbImage>> {
    #[cfg(windows)]
    return mf::read(path).ok().map(|m| m.frames).filter(|f| !f.is_empty());
    #[cfg(not(windows))]
    None
}

const KEEP_PLACED_MS: i64 = 7 * 24 * 60 * 60 * 1000;

pub fn tidy(conn: &Connection, cache: &Path, now: i64) -> rusqlite::Result<(usize, usize)> {
    let stale: Vec<i64> = conn
        .prepare(
            "SELECT c.id FROM card c WHERE c.stage IN ('frames','embedded') AND (c.status = 'gone' OR (c.status = 'placed' AND
               COALESCE((SELECT MAX(m.at) FROM move m JOIN move_item i ON i.move_id = m.id WHERE i.card_id = c.id AND m.state = 'done'), 0) < ?1))",
        )?
        .query_map(params![now - KEEP_PLACED_MS], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    for id in &stale {
        conn.execute("DELETE FROM embedding WHERE card_id = ?1", params![id])?;
        conn.execute("UPDATE card SET stage = 'meta', frames = 0 WHERE id = ?1", params![id])?;
    }
    let keep: std::collections::HashSet<String> = conn
        .prepare("SELECT id FROM card WHERE stage IN ('frames','embedded')")?
        .query_map([], |r| r.get::<_, i64>(0))?
        .map(|id| id.map(|id| id.to_string()))
        .collect::<Result<_, _>>()?;
    let mut dirs = 0;
    if let Ok(entries) = fs::read_dir(cache) {
        for dir in entries.filter_map(|e| e.ok()) {
            if !keep.contains(dir.file_name().to_string_lossy().as_ref()) && fs::remove_dir_all(dir.path()).is_ok() {
                dirs += 1;
            }
        }
    }
    let missing: Vec<i64> = conn
        .prepare("SELECT id, path FROM example")?
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?
        .filter_map(|r| r.ok())
        .filter(|(_, p)| !Path::new(p).exists())
        .map(|(id, _)| id)
        .collect();
    for id in &missing {
        conn.execute("DELETE FROM example WHERE id = ?1", params![id])?;
    }
    dupes::forget_missing(conn)?;
    Ok((dirs, missing.len()))
}

pub fn spawn(app: AppHandle, db_path: PathBuf, cache: PathBuf, paused: Arc<AtomicBool>, wake: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        #[cfg(windows)]
        mf::start();
        let Ok(conn) = db::open(&db_path) else { return };
        if let Err(e) = tidy(&conn, &cache, db::now_ms()) {
            eprintln!("tidy: {e}");
        }
        let mut last_emit = Instant::now() - Duration::from_secs(1);
        let mut was_busy = false;
        let mut learner = Learner::new();
        let mut printer = dupes::near::Printer::new();
        let mut dupes_at = Instant::now() - Duration::from_secs(60);
        loop {
            let is_paused = paused.load(Ordering::Relaxed);
            if !is_paused && dupes_at.elapsed() > Duration::from_secs(3) {
                match dupes::refresh_exact(&conn) {
                    Ok(true) => {
                        let _ = app.emit("dupes://changed", ());
                    }
                    Ok(false) => {}
                    Err(e) => eprintln!("dupes: {e}"),
                }
                dupes_at = Instant::now();
            }
            let jobs = if is_paused { Vec::new() } else { next(&conn, workers()).unwrap_or_default() };
            match jobs.is_empty() {
                false => {
                    was_busy = true;
                    for ((id, _), result) in jobs.iter().zip(develop_batch(&cache, &jobs)) {
                        if save(&conn, *id, result).is_ok() {
                            if let Ok(cards) = deck::cards(&conn, &[*id]) {
                                let card: Option<&CardView> = cards.first();
                                let _ = app.emit("develop://card", card);
                            }
                        }
                    }
                    if last_emit.elapsed() > Duration::from_millis(250) {
                        if let Ok(p) = progress(&conn, false) {
                            let _ = app.emit("develop://progress", p);
                        }
                        last_emit = Instant::now();
                    }
                }
                true => {
                    if !is_paused && learner.step(&app, &conn, &cache) {
                        continue;
                    }
                    if !is_paused {
                        match printer.step(&conn, learner.engine(&cache)) {
                            Ok(Some(changed)) => {
                                if changed {
                                    let _ = app.emit("dupes://changed", ());
                                }
                                if let Ok(p) = progress(&conn, false) {
                                    let _ = app.emit("develop://progress", p);
                                }
                                continue;
                            }
                            Ok(None) => {}
                            Err(e) => eprintln!("dupes: {e}"),
                        }
                    }
                    if was_busy || wake.swap(false, Ordering::Relaxed) {
                        if let Ok(p) = progress(&conn, is_paused) {
                            let _ = app.emit("develop://progress", p);
                        }
                        was_busy = false;
                    }
                    std::thread::sleep(Duration::from_millis(400));
                }
            }
        }
    });
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn reads_real_video_if_present() {
        let Some(path) = fs::read_dir(r"G:\vk videos").ok().and_then(|mut it| it.find_map(|e| e.ok().map(|e| e.path())))
        else {
            return;
        };
        mf::start();
        let meta = mf::read(&path).unwrap();
        assert!(meta.duration_ms > 0);
        assert_eq!(meta.frames.len(), FRAMES as usize);
        assert!(meta.width > 0 && meta.height > 0);
    }

    #[test]
    fn tidy_drops_old_frames_and_missing_examples() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("cache");
        let conn = Connection::open_in_memory().unwrap();
        db::init(&conn).unwrap();
        conn.execute_batch(
            "INSERT INTO pile(id, table_path, name, ord) VALUES (1, 'T', 'A', 0);
             INSERT INTO card(id, deck_path, file_name, size, mtime, position, stage, frames, status) VALUES
               (1, 'D', 'deck.mp4', 1, 0, 1, 'embedded', 8, 'in_deck'),
               (2, 'D', 'old.mp4', 1, 0, 2, 'embedded', 8, 'placed'),
               (3, 'D', 'fresh.mp4', 1, 0, 3, 'embedded', 8, 'placed'),
               (4, 'D', 'gone.mp4', 1, 0, 4, 'frames', 8, 'gone');
             INSERT INTO move(id, at, method, pile_id, state) VALUES (1, 1000, 'key', 1, 'done'), (2, 900000000000, 'key', 1, 'done');
             INSERT INTO move_item(move_id, card_id, from_path, step) VALUES (1, 2, 'x', 'done'), (2, 3, 'y', 'done');
             INSERT INTO embedding(card_id, vector) VALUES (1, x'00'), (2, x'00'), (3, x'00');",
        )
        .unwrap();
        let kept = dir.path().join("kept.mp4");
        fs::write(&kept, b"x").unwrap();
        conn.execute("INSERT INTO example(pile_id, path, vector) VALUES (1, ?1, x'00'), (1, 'Z:/nope.mp4', x'00')", params![kept.to_string_lossy()]).unwrap();
        for id in ["1", "2", "3", "4", "99"] {
            fs::create_dir_all(cache.join(id)).unwrap();
        }
        let (dirs, examples) = tidy(&conn, &cache, 900000000000 + 1000).unwrap();
        assert_eq!((dirs, examples), (3, 1));
        assert!(cache.join("1").exists() && cache.join("3").exists());
        let stage = |id: i64| conn.query_row("SELECT stage FROM card WHERE id = ?1", params![id], |r| r.get::<_, String>(0)).unwrap();
        assert_eq!((stage(1).as_str(), stage(2).as_str(), stage(3).as_str(), stage(4).as_str()), ("embedded", "meta", "embedded", "meta"));
        assert_eq!(conn.query_row("SELECT COUNT(*) FROM embedding", [], |r| r.get::<_, i64>(0)).unwrap(), 2);
    }

    #[test]
    #[ignore]
    fn bench_twenty_cards() {
        let Ok(dir) = fs::read_dir(r"G:\sorter-test\deck") else { return };
        let jobs: Vec<_> = dir.filter_map(|e| e.ok().map(|e| e.path())).take(20).enumerate().map(|(i, p)| (i as i64, p)).collect();
        let cache = std::env::temp_dir().join("sorter-bench");
        let total = Instant::now();
        let mut ok = 0;
        for chunk in jobs.chunks(workers()) {
            ok += develop_batch(&cache, chunk).iter().filter(|r| r.is_some()).count();
        }
        println!("ИТОГО {ok}/{} карт, {} потока: {} ms", jobs.len(), workers(), total.elapsed().as_millis());
    }
}
