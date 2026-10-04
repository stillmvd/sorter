use crate::db;
use crate::deck;
use crate::dupes;
use crate::hints::{self, Engine};
use crate::photo;
use std::collections::{HashSet, VecDeque};
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

pub static HINTS_FAILED: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    done: i64,
    total: i64,
    paused: bool,
    printed: i64,
    prints: i64,
    hints: bool,
    embedded: i64,
    stage: &'static str,
    speed: f64,
    ready: bool,
    scope: &'static str,
    search: bool,
}

#[cfg(windows)]
mod mf {
    use super::{Meta, FRAMES, THUMB};
    use image::imageops;
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
            MFCreateAttributes(&mut attrs, 3)?;
            let attrs = attrs.unwrap();
            attrs.SetUINT32(&MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING, 1)?;
            let hw = crate::dupes::print::mf::hardware(&attrs)?;
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
            let (tw, th) = if hw && (rotation == 90 || rotation == 270) { (th, tw) } else { (tw, th) };
            wanted.SetUINT64(&MF_MT_FRAME_SIZE, (tw as u64) << 32 | th as u64)?;
            if reader.SetCurrentMediaType(VIDEO, None, &wanted).is_err() {
                wanted.DeleteItem(&MF_MT_FRAME_SIZE)?;
                reader.SetCurrentMediaType(VIDEO, None, &wanted)?;
            }
            let current = reader.GetCurrentMediaType(VIDEO)?;
            let size = current.GetUINT64(&MF_MT_FRAME_SIZE)?;
            let (w, h) = ((size >> 32) as u32, (size & 0xFFFF_FFFF) as u32);

            let duration = reader
                .GetPresentationAttribute(MF_SOURCE_READER_MEDIASOURCE.0 as u32, &MF_PD_DURATION)
                .ok()
                .and_then(|v| u64::try_from(&v).ok())
                .filter(|d| *d > 0)
                .map(|d| d as i64)
                .or_else(|| super::fragmented_duration(path))
                .unwrap_or(0);

            let mut frames = Vec::new();
            let mut seek = true;
            for i in 0..FRAMES {
                let at = duration * (2 * i as i64 + 1) / (2 * FRAMES as i64);
                if duration > 0 {
                    if seek && reader.SetCurrentPosition(&GUID::zeroed(), &PROPVARIANT::from(at)).is_err() {
                        seek = false;
                    }
                } else if i > 0 {
                    break;
                }
                let mut sample = None;
                let mut flags = 0u32;
                let mut ts = 0i64;
                for _ in 0..if seek { 30 } else { 100_000 } {
                    sample = None;
                    reader.ReadSample(VIDEO, 0, None, Some(&mut flags), Some(&mut ts), Some(&mut sample))?;
                    if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 || (sample.is_some() && (seek || ts >= at)) {
                        break;
                    }
                }
                let Some(sample) = sample else { break };
                let img = crate::dupes::print::mf::frame(&sample, w, h)?;
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
                small = match if hw { 0 } else { rotation } {
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

fn fragmented_duration(path: &Path) -> Option<i64> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = fs::File::open(path).ok()?;
    let end = f.metadata().ok()?.len();
    let mut tracks: std::collections::HashMap<u32, f64> = std::collections::HashMap::new();
    let mut pos = 0u64;
    let mut head = [0u8; 16];
    while pos + 8 <= end {
        f.seek(SeekFrom::Start(pos)).ok()?;
        f.read_exact(&mut head[..8]).ok()?;
        let mut size = u32::from_be_bytes(head[..4].try_into().ok()?) as u64;
        let kind: [u8; 4] = head[4..8].try_into().ok()?;
        let mut at = 8;
        if size == 1 {
            f.read_exact(&mut head[8..16]).ok()?;
            size = u64::from_be_bytes(head[8..16].try_into().ok()?);
            at = 16;
        } else if size == 0 {
            size = end - pos;
        }
        if size < at {
            break;
        }
        if &kind == b"sidx" && size <= 1 << 20 {
            let mut body = vec![0u8; (size - at) as usize];
            f.read_exact(&mut body).ok()?;
            let u32_at = |o: usize| body.get(o..o + 4).map(|b| u32::from_be_bytes(b.try_into().unwrap()));
            let track = u32_at(4)?;
            let scale = u32_at(8)?.max(1) as f64;
            let refs_at = if body[0] == 0 { 20 } else { 28 };
            let count = body.get(refs_at + 2..refs_at + 4).map(|b| u16::from_be_bytes([b[0], b[1]]))? as usize;
            let total: u64 = (0..count).filter_map(|k| u32_at(refs_at + 4 + k * 12 + 4)).map(u64::from).sum();
            *tracks.entry(track).or_default() += total as f64 / scale;
        }
        pos += size;
    }
    tracks.values().cloned().fold(None, |m: Option<f64>, v| Some(m.map_or(v, |m| m.max(v)))).filter(|s| *s > 0.0).map(|s| (s * 1e7) as i64)
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

pub fn status(conn: &Connection, paused: bool, printed: (i64, i64)) -> rusqlite::Result<Status> {
    let deck = db::get_setting(conn, "deck_path")?.unwrap_or_default();
    let (total, done, embedded): (i64, i64, i64) = conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(stage != 'new'), 0), COALESCE(SUM(stage IN ('embedded','broken')), 0)
         FROM card WHERE deck_path = ?1 AND status IN ('in_deck','deferred')",
        params![deck],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    let hints = hints::enabled(conn) && !HINTS_FAILED.load(Ordering::Relaxed);
    let scope = dupes::Scope::current(conn)?;
    let search = matches!(scope, dupes::Scope::Search(_));
    let stage = if let dupes::Scope::Search(spec) = &scope {
        if spec.state == "running" {
            "dupes"
        } else {
            "done"
        }
    } else if done < total {
        "frames"
    } else if printed.0 < printed.1 {
        "dupes"
    } else if hints && embedded < total {
        "hints"
    } else {
        "done"
    };
    Ok(Status {
        done,
        total,
        paused,
        printed: printed.0,
        prints: printed.1,
        hints,
        embedded,
        stage,
        speed: 0.0,
        ready: stage == "done",
        scope: scope.name(),
        search,
    })
}

struct Report {
    at: Instant,
    dirty: bool,
    stage: &'static str,
    samples: VecDeque<(Instant, i64)>,
}

impl Report {
    fn new() -> Self {
        Report { at: Instant::now() - Duration::from_secs(1), dirty: false, stage: "", samples: VecDeque::new() }
    }

    fn tick(&mut self, app: &AppHandle, conn: &Connection, paused: bool, printed: &mut Printed, force: bool) {
        self.dirty = true;
        if !force && self.at.elapsed() < Duration::from_millis(250) {
            return;
        }
        let Ok(mut s) = status(conn, paused, printed.get(conn)) else { return };
        let value = match s.stage {
            "frames" => s.done,
            "dupes" => s.printed,
            "hints" => s.embedded,
            _ => 0,
        };
        let now = Instant::now();
        if self.stage != s.stage || paused {
            self.stage = s.stage;
            self.samples.clear();
        }
        self.samples.push_back((now, value));
        while self.samples.len() > 1 && now.duration_since(self.samples[0].0) > Duration::from_secs(10) {
            self.samples.pop_front();
        }
        if let (Some(a), Some(b)) = (self.samples.front(), self.samples.back()) {
            let dt = b.0.duration_since(a.0).as_secs_f64();
            if dt >= 1.0 && !paused {
                s.speed = (b.1 - a.1).max(0) as f64 / dt;
            }
        }
        let _ = app.emit("develop://progress", s);
        self.at = now;
        self.dirty = false;
    }
}

enum Developed {
    Video { meta: Meta, saved: u32 },
    Photo(photo::PhotoMeta),
}

fn save_jpeg(file: &Path, img: &image::RgbImage) -> bool {
    fs::File::create(file)
        .ok()
        .and_then(|f| {
            let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(std::io::BufWriter::new(f), 82);
            enc.encode_image(img).ok()
        })
        .is_some()
}

fn lower_priority() {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::System::Threading::{GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_BELOW_NORMAL};
        let _ = SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_BELOW_NORMAL);
    }
}

struct Printed {
    value: (i64, i64),
    at: Instant,
}

impl Printed {
    fn get(&mut self, conn: &Connection) -> (i64, i64) {
        if self.at.elapsed() > Duration::from_secs(5) {
            self.value = dupes::printed(conn).unwrap_or(self.value);
            self.at = Instant::now();
        }
        self.value
    }

    fn bump(&mut self, n: usize) {
        self.value.0 = (self.value.0 + n as i64).min(self.value.1);
    }

    fn reset(&mut self) {
        self.at = Instant::now() - Duration::from_secs(60);
    }
}

pub(crate) fn prepare_thread() {
    lower_priority();
    #[cfg(windows)]
    mf::start();
}

pub(crate) fn workers() -> usize {
    std::thread::available_parallelism().map(|n| n.get() / 2).unwrap_or(2).clamp(2, 4)
}

fn develop_files(cache: &Path, id: i64, path: &Path) -> Option<Developed> {
    let dir = cache.join(id.to_string());
    if deck::is_photo(path) {
        let meta = photo::read(path).ok()?;
        let _ = fs::create_dir_all(&dir);
        return save_jpeg(&dir.join("0.jpg"), &meta.thumb).then_some(Developed::Photo(meta));
    }
    #[cfg(windows)]
    let meta = mf::read(path).ok()?;
    #[cfg(not(windows))]
    let meta: Meta = return None;
    if meta.frames.is_empty() {
        return None;
    }
    let _ = fs::create_dir_all(&dir);
    let saved = meta.frames.iter().enumerate().filter(|(i, frame)| save_jpeg(&dir.join(format!("{i}.jpg")), frame)).count() as u32;
    Some(Developed::Video { meta, saved })
}

fn develop_batch(cache: &Path, jobs: &[(i64, PathBuf)]) -> Vec<Option<Developed>> {
    std::thread::scope(|scope| {
        let handles: Vec<_> = jobs
            .iter()
            .map(|(id, path)| {
                scope.spawn(move || {
                    lower_priority();
                    #[cfg(windows)]
                    mf::start();
                    develop_files(cache, *id, path)
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().ok().flatten()).collect()
    })
}

fn save(conn: &Connection, id: i64, path: &Path, result: Option<Developed>) -> rusqlite::Result<()> {
    match result {
        Some(Developed::Video { meta, saved }) => conn.execute(
            "UPDATE card SET duration_ms = ?2, width = ?3, height = ?4, orientation = ?5, frames = ?6, stage = 'frames', error = NULL WHERE id = ?1",
            params![id, meta.duration_ms, meta.width, meta.height, orientation(meta.width, meta.height), saved],
        ),
        Some(Developed::Photo(m)) => conn.execute(
            "UPDATE card SET width = ?2, height = ?3, orientation = ?4, frames = 1, stage = 'frames', error = NULL,
               camera = ?5, phash = ?6, sharp = ?7, look = ?9,
               taken_at = COALESCE(?8, taken_at), taken_from = CASE WHEN ?8 IS NULL THEN taken_from ELSE 'exif' END
             WHERE id = ?1",
            params![id, m.width, m.height, orientation(m.width, m.height), m.camera, m.phash as i64, m.sharp, m.taken, m.look],
        ),
        None if deck::is_photo(path) => conn.execute(
            "UPDATE card SET stage = 'broken', error = ?2 WHERE id = ?1",
            params![id, "Не удалось открыть это фото — разложить его всё равно можно."],
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
                    HINTS_FAILED.store(true, Ordering::Relaxed);
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
        let mut stored = Vec::new();
        for id in &ids {
            let frames = hints::card_frames(cache, *id);
            let vector = engine.embed(&frames).ok().and_then(|vs| hints::mean(&vs));
            if hints::store(conn, *id, vector.as_deref()).is_ok() {
                stored.push(*id);
            }
        }
        if let Ok(cards) = deck::cards(conn, &stored) {
            let _ = app.emit("develop://cards", cards);
        }
        if let Some((pile, path)) = cold {
            let vector = read_frames(&path)
                .and_then(|frames| {
                    let middle = if frames.len() > 2 { &frames[2..6.min(frames.len())] } else { &frames[..] };
                    engine.embed(middle).ok()
                })
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
    if deck::is_photo(path) {
        return photo::read(path).ok().map(|m| vec![m.thumb]);
    }
    #[cfg(windows)]
    return mf::read(path).ok().map(|m| m.frames).filter(|f| !f.is_empty());
    #[cfg(not(windows))]
    None
}

pub fn cache_size(cache: &Path) -> (u64, usize) {
    fn walk(dir: &Path) -> u64 {
        fs::read_dir(dir)
            .map(|it| {
                it.filter_map(|e| e.ok())
                    .map(|e| match e.file_type() {
                        Ok(t) if t.is_dir() => walk(&e.path()),
                        _ => e.metadata().map(|m| m.len()).unwrap_or(0),
                    })
                    .sum()
            })
            .unwrap_or(0)
    }
    let cards = fs::read_dir(cache).map(|it| it.filter_map(|e| e.ok()).filter(|e| e.path().is_dir()).count()).unwrap_or(0);
    (walk(cache), cards)
}

pub fn clear_cache(conn: &Connection, cache: &Path) -> rusqlite::Result<u64> {
    let (before, _) = cache_size(cache);
    if let Ok(entries) = fs::read_dir(cache) {
        for dir in entries.filter_map(|e| e.ok()) {
            let _ = fs::remove_dir_all(dir.path());
        }
    }
    conn.execute(
        "UPDATE card SET stage = CASE WHEN status IN ('in_deck','deferred') THEN 'new' ELSE 'meta' END, frames = 0
         WHERE stage IN ('frames','embedded')",
        [],
    )?;
    let (after, _) = cache_size(cache);
    Ok(before.saturating_sub(after))
}

pub fn forget_deck(conn: &Connection, cache: &Path, deck: &str, only_untouched: bool) -> rusqlite::Result<usize> {
    let placed: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM card WHERE deck_path = ?1 AND status = 'placed')", params![deck], |r| r.get(0))?;
    if only_untouched && placed {
        return Ok(0);
    }
    let cards: Vec<(i64, String, bool)> = conn
        .prepare(
            "SELECT id, file_name, EXISTS(SELECT 1 FROM move_item i WHERE i.card_id = card.id) FROM card
             WHERE deck_path = ?1 AND status != 'placed'",
        )?
        .query_map(params![deck], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<Result<_, _>>()?;
    let tx = conn.unchecked_transaction()?;
    for (id, name, moved) in &cards {
        let path = Path::new(deck).join(name).to_string_lossy().into_owned();
        tx.execute("DELETE FROM embedding WHERE card_id = ?1", params![id])?;
        if *moved {
            tx.execute("UPDATE card SET stage = 'new', frames = 0 WHERE id = ?1", params![id])?;
        } else {
            tx.execute("DELETE FROM card WHERE id = ?1", params![id])?;
        }
        tx.execute("DELETE FROM fingerprint WHERE path = ?1", params![path])?;
        tx.execute("DELETE FROM dupe WHERE a = ?1 OR b = ?1", params![path])?;
        tx.execute("DELETE FROM dupe_dismissed WHERE a = ?1 OR b = ?1", params![path])?;
    }
    tx.commit()?;
    let dirs: Vec<PathBuf> = cards.iter().map(|(id, ..)| cache.join(id.to_string())).collect();
    let n = dirs.len();
    let gone = std::thread::spawn(move || dirs.iter().for_each(|d| drop(fs::remove_dir_all(d))));
    if cfg!(test) {
        let _ = gone.join();
    }
    Ok(n)
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
        lower_priority();
        let mut last_emit = Instant::now() - Duration::from_secs(1);
        let mut was_busy = false;
        let mut learner = Learner::new();
        let mut printer = dupes::near::Printer::new();
        let mut dupes_at = Instant::now() - Duration::from_secs(60);
        let mut printed = Printed { value: (0, 0), at: Instant::now() - Duration::from_secs(60) };
        let mut report = Report::new();
        let mut developed: Vec<i64> = Vec::new();
        let mut series_dirty = false;
        let mut series_at = Instant::now();
        let mut scope_name = "";
        loop {
            let is_paused = paused.load(Ordering::Relaxed);
            let scope = dupes::Scope::work(&conn).unwrap_or(dupes::Scope::Off);
            if scope.name() != scope_name {
                scope_name = scope.name();
                printed.reset();
                dupes_at = Instant::now() - Duration::from_secs(60);
            }
            let jobs = if is_paused { Vec::new() } else { next(&conn, workers()).unwrap_or_default() };
            let idle = jobs.is_empty();
            if !idle {
                was_busy = true;
                let results = develop_batch(&cache, &jobs);
                if let Ok(tx) = conn.unchecked_transaction() {
                    for ((id, path), result) in jobs.iter().zip(results) {
                        if save(&tx, *id, path, result).is_ok() {
                            developed.push(*id);
                        }
                    }
                    let _ = tx.commit();
                }
                series_dirty |= jobs.iter().any(|(_, p)| deck::is_photo(p));
            }
            if series_dirty && (idle || series_at.elapsed() > Duration::from_secs(5)) {
                let deck_path = db::get_setting(&conn, "deck_path").ok().flatten().unwrap_or_default();
                if crate::series::regroup(&conn, &deck_path).unwrap_or(false) {
                    let _ = app.emit("deck://series", ());
                }
                series_dirty = false;
                series_at = Instant::now();
            }
            if !developed.is_empty() && (idle || last_emit.elapsed() > Duration::from_millis(250)) {
                if let Ok(cards) = deck::cards(&conn, &developed) {
                    let _ = app.emit("develop://cards", cards);
                }
                developed.clear();
                last_emit = Instant::now();
            }
            if !idle {
                report.tick(&app, &conn, false, &mut printed, false);
                continue;
            }
            let exact_every = Duration::from_secs(if was_busy { 30 } else { 3 });
            if !is_paused && scope != dupes::Scope::Off && dupes_at.elapsed() > exact_every {
                match dupes::refresh_exact(&conn) {
                    Ok(true) => {
                        let _ = app.emit("dupes://changed", ());
                    }
                    Ok(false) => {}
                    Err(e) => eprintln!("dupes: {e}"),
                }
                dupes_at = Instant::now();
            }
            if !is_paused {
                match printer.step(&conn, learner.engine(&cache)) {
                    Ok(Some((n, changed))) => {
                        if changed {
                            let _ = app.emit("dupes://changed", ());
                        }
                        printed.bump(n);
                        report.tick(&app, &conn, false, &mut printed, false);
                        continue;
                    }
                    Ok(None) => {
                        if let dupes::Scope::Search(mut spec) = scope {
                            if spec.state == "running" {
                                if let Err(e) = dupes::refresh_exact(&conn) {
                                    eprintln!("dupes: {e}");
                                }
                                spec.state = "done".into();
                                let _ = dupes::search::save(&conn, &spec);
                                let _ = app.emit("dupes://changed", ());
                                printed.reset();
                                report.tick(&app, &conn, false, &mut printed, true);
                                continue;
                            }
                        }
                    }
                    Err(e) => eprintln!("dupes: {e}"),
                }
            }
            if !is_paused && learner.step(&app, &conn, &cache) {
                report.tick(&app, &conn, false, &mut printed, false);
                continue;
            }
            if was_busy || report.dirty || wake.swap(false, Ordering::Relaxed) {
                printed.reset();
                report.tick(&app, &conn, is_paused, &mut printed, true);
                was_busy = false;
            }
            std::thread::sleep(Duration::from_millis(400));
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
    fn reads_fragmented_mp4_if_present() {
        let path = Path::new(r"G:\sorter-test\small-60\17367477074721.mp4");
        if !path.exists() {
            return;
        }
        let secs = fragmented_duration(path).unwrap() / 10_000_000;
        assert!((14..=16).contains(&secs), "{secs}");
        mf::start();
        let meta = mf::read(path).unwrap();
        assert_eq!(meta.duration_ms / 1000, secs);
        assert_eq!(meta.frames.len(), FRAMES as usize);
        assert_ne!(meta.frames[0].as_raw(), meta.frames[7].as_raw());
    }

    #[test]
    fn clear_cache_empties_dir_and_redevelops_deck() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("cache");
        let conn = Connection::open_in_memory().unwrap();
        db::init(&conn).unwrap();
        conn.execute_batch(
            "INSERT INTO card(id, deck_path, file_name, size, mtime, position, stage, frames, status) VALUES
               (1, 'D', 'deck.mp4', 1, 0, 1, 'embedded', 8, 'in_deck'),
               (2, 'D', 'placed.mp4', 1, 0, 2, 'frames', 8, 'placed'),
               (3, 'D', 'broken.mp4', 1, 0, 3, 'broken', 0, 'in_deck');",
        )
        .unwrap();
        for id in ["1", "2"] {
            fs::create_dir_all(cache.join(id)).unwrap();
            fs::write(cache.join(id).join("0.jpg"), [0u8; 100]).unwrap();
        }
        assert_eq!(cache_size(&cache), (200, 2));
        assert_eq!(clear_cache(&conn, &cache).unwrap(), 200);
        assert_eq!(cache_size(&cache), (0, 0));
        let stage = |id: i64| conn.query_row("SELECT stage FROM card WHERE id = ?1", params![id], |r| r.get::<_, String>(0)).unwrap();
        assert_eq!((stage(1).as_str(), stage(2).as_str(), stage(3).as_str()), ("new", "meta", "broken"));
    }

    #[test]
    fn dupes_stage_only_when_search_is_on() {
        let dir = tempfile::tempdir().unwrap();
        let deck = dir.path().join("deck");
        fs::create_dir_all(&deck).unwrap();
        fs::write(deck.join("a.mp4"), b"a").unwrap();
        let conn = Connection::open_in_memory().unwrap();
        db::init(&conn).unwrap();
        db::set_setting(&conn, "deck_path", &deck.to_string_lossy()).unwrap();
        conn.execute(
            "INSERT INTO card(deck_path, file_name, size, mtime, position, stage, frames) VALUES (?1, 'a.mp4', 1, 0, 0, 'frames', 8)",
            params![deck.to_string_lossy()],
        )
        .unwrap();
        let s = status(&conn, false, dupes::printed(&conn).unwrap()).unwrap();
        assert_eq!((s.stage, s.scope, s.ready, s.prints), ("done", "off", true, 0));
        db::set_setting(&conn, "dupes_enabled", "1").unwrap();
        let s = status(&conn, false, dupes::printed(&conn).unwrap()).unwrap();
        assert_eq!((s.stage, s.scope, s.ready, s.prints), ("dupes", "deck", false, 1));
        dupes::search::start(&conn, dupes::search::SearchSpec { deck: true, ..Default::default() }).unwrap();
        let s = status(&conn, false, dupes::printed(&conn).unwrap()).unwrap();
        assert_eq!((s.stage, s.search), ("dupes", true));
    }

    #[test]
    fn forget_deck_drops_untouched_deck_and_keeps_placed() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("cache");
        let conn = Connection::open_in_memory().unwrap();
        db::init(&conn).unwrap();
        conn.execute_batch(
            "INSERT INTO pile(id, table_path, name, ord) VALUES (1, 'T', 'A', 0);
             INSERT INTO card(id, deck_path, file_name, size, mtime, position, stage, frames, status) VALUES
               (1, 'D', 'a.mp4', 1, 0, 1, 'frames', 8, 'in_deck'),
               (2, 'D', 'b.mp4', 1, 0, 2, 'frames', 8, 'in_deck'),
               (3, 'E', 'c.mp4', 1, 0, 1, 'frames', 8, 'in_deck'),
               (4, 'E', 'd.mp4', 1, 0, 2, 'frames', 8, 'placed');
             INSERT INTO move(id, at, method, pile_id, state) VALUES (1, 1, 'key', 1, 'undone');
             INSERT INTO move_item(move_id, card_id, from_path, step) VALUES (1, 2, 'x', 'done');
             INSERT INTO embedding(card_id, vector) VALUES (1, x'00'), (2, x'00');",
        )
        .unwrap();
        let a = Path::new("D").join("a.mp4").to_string_lossy().into_owned();
        conn.execute("INSERT INTO fingerprint(path, size, mtime, state) VALUES (?1, 1, 0, 'ok')", params![a]).unwrap();
        conn.execute("INSERT INTO dupe(a, b, kind, confidence) VALUES (?1, 'Z', 'same', 90)", params![a]).unwrap();
        for id in ["1", "2", "3"] {
            fs::create_dir_all(cache.join(id)).unwrap();
        }
        assert_eq!(forget_deck(&conn, &cache, "E", true).unwrap(), 0);
        assert_eq!(forget_deck(&conn, &cache, "D", true).unwrap(), 2);
        let count = |sql: &str| conn.query_row(sql, [], |r| r.get::<_, i64>(0)).unwrap();
        assert_eq!(count("SELECT COUNT(*) FROM card WHERE deck_path = 'D'"), 1);
        assert_eq!(count("SELECT COUNT(*) FROM card WHERE id = 2 AND stage = 'new' AND frames = 0"), 1);
        assert_eq!(count("SELECT COUNT(*) FROM embedding"), 0);
        assert_eq!(count("SELECT COUNT(*) FROM fingerprint") + count("SELECT COUNT(*) FROM dupe"), 0);
        assert!(!cache.join("1").exists() && !cache.join("2").exists() && cache.join("3").exists());
        assert_eq!(forget_deck(&conn, &cache, "E", false).unwrap(), 1);
        assert_eq!(count("SELECT COUNT(*) FROM card WHERE deck_path = 'E'"), 1);
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

    #[test]
    #[ignore]
    fn bench_each_card() {
        mf::start();
        let Ok(dir) = fs::read_dir(r"G:\sorter-test\deck") else { return };
        let cache = std::env::temp_dir().join("sorter-bench");
        for (i, e) in dir.filter_map(|e| e.ok()).enumerate() {
            let t = Instant::now();
            let ok = develop_files(&cache, i as i64, &e.path()).is_some();
            println!("КАРТА {} ms {ok} {}", t.elapsed().as_millis(), e.path().display());
        }
    }
}
