use crate::db;
use crate::deck::{self, CardView};
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
    conn.query_row(
        "SELECT SUM(stage != 'new'), COUNT(*) FROM card WHERE deck_path = ?1 AND status IN ('in_deck','deferred')",
        params![deck],
        |r| Ok(Progress { done: r.get::<_, Option<i64>>(0)?.unwrap_or(0), total: r.get(1)?, paused }),
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

pub fn spawn(app: AppHandle, db_path: PathBuf, cache: PathBuf, paused: Arc<AtomicBool>, wake: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        #[cfg(windows)]
        mf::start();
        let Ok(conn) = db::open(&db_path) else { return };
        let mut last_emit = Instant::now() - Duration::from_secs(1);
        let mut was_busy = false;
        loop {
            let is_paused = paused.load(Ordering::Relaxed);
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
