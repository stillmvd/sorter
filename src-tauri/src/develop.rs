use crate::db;
use crate::deck::{self, CardView};
use rusqlite::{params, Connection, OptionalExtension};
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
            attrs.SetUINT32(&MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, 1)?;
            let reader = MFCreateSourceReaderFromURL(&HSTRING::from(path.as_os_str()), &attrs)?;
            reader.SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false)?;
            reader.SetStreamSelection(VIDEO, true)?;

            let rotation = reader
                .GetNativeMediaType(VIDEO, 0)
                .and_then(|t| t.GetUINT32(&MF_MT_VIDEO_ROTATION))
                .unwrap_or(0);

            let wanted = MFCreateMediaType()?;
            wanted.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
            wanted.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32)?;
            reader.SetCurrentMediaType(VIDEO, None, &wanted)?;
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
                let mut img = RgbImage::new(w, h);
                let abs = stride.unsigned_abs() as usize;
                for y in 0..h as usize {
                    let row = if stride < 0 { h as usize - 1 - y } else { y };
                    let start = row * abs;
                    if start + w as usize * 4 > data.len() {
                        break;
                    }
                    for x in 0..w as usize {
                        let p = start + x * 4;
                        img.put_pixel(x as u32, y as u32, image::Rgb([data[p + 2], data[p + 1], data[p]]));
                    }
                }
                buffer.Unlock()?;
                let scale = THUMB as f32 / w.max(h) as f32;
                let mut small = imageops::resize(
                    &img,
                    ((w as f32 * scale).round() as u32).max(1),
                    ((h as f32 * scale).round() as u32).max(1),
                    imageops::FilterType::Triangle,
                );
                small = match rotation {
                    90 => imageops::rotate90(&small),
                    180 => imageops::rotate180(&small),
                    270 => imageops::rotate270(&small),
                    _ => small,
                };
                frames.push(small);
            }
            let (width, height) = if rotation == 90 || rotation == 270 { (h, w) } else { (w, h) };
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

fn next(conn: &Connection) -> rusqlite::Result<Option<(i64, PathBuf)>> {
    let deck = db::get_setting(conn, "deck_path")?.unwrap_or_default();
    conn.query_row(
        "SELECT id, deck_path, file_name, status, current_path FROM card
         WHERE stage = 'new' AND status != 'gone' AND (current_path IS NOT NULL OR status != 'placed')
         ORDER BY (deck_path = ?1 AND status = 'in_deck') DESC, status = 'deferred', position LIMIT 1",
        params![deck],
        |r| {
            let (id, d, name, status, current): (i64, String, String, String, Option<String>) =
                (r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?);
            let path = match (status.as_str(), current) {
                ("placed", Some(c)) => PathBuf::from(c),
                _ => Path::new(&d).join(name),
            };
            Ok((id, path))
        },
    )
    .optional()
}

fn progress(conn: &Connection, paused: bool) -> rusqlite::Result<Progress> {
    let deck = db::get_setting(conn, "deck_path")?.unwrap_or_default();
    conn.query_row(
        "SELECT SUM(stage != 'new'), COUNT(*) FROM card WHERE deck_path = ?1 AND status IN ('in_deck','deferred')",
        params![deck],
        |r| Ok(Progress { done: r.get::<_, Option<i64>>(0)?.unwrap_or(0), total: r.get(1)?, paused }),
    )
}

fn develop_one(conn: &Connection, cache: &Path, id: i64, path: &Path) -> rusqlite::Result<()> {
    #[cfg(windows)]
    let result = mf::read(path).map_err(|e| e.message().to_string());
    #[cfg(not(windows))]
    let result: Result<Meta, String> = Err("не Windows".into());
    match result {
        Ok(meta) if !meta.frames.is_empty() => {
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
            conn.execute(
                "UPDATE card SET duration_ms = ?2, width = ?3, height = ?4, orientation = ?5, frames = ?6, stage = 'frames', error = NULL WHERE id = ?1",
                params![id, meta.duration_ms, meta.width, meta.height, orientation(meta.width, meta.height), saved],
            )?;
        }
        Ok(_) | Err(_) => {
            conn.execute(
                "UPDATE card SET stage = 'broken', error = ?2 WHERE id = ?1",
                params![id, "Не удалось прочитать кадры этого видео — разложить его всё равно можно."],
            )?;
        }
    }
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
            let job = if is_paused { None } else { next(&conn).ok().flatten() };
            match job {
                Some((id, path)) => {
                    was_busy = true;
                    if develop_one(&conn, &cache, id, &path).is_ok() {
                        if let Ok(cards) = deck::cards(&conn, &[id]) {
                            let card: Option<&CardView> = cards.first();
                            let _ = app.emit("develop://card", card);
                        }
                    }
                    if last_emit.elapsed() > Duration::from_millis(250) {
                        if let Ok(p) = progress(&conn, false) {
                            let _ = app.emit("develop://progress", p);
                        }
                        last_emit = Instant::now();
                    }
                }
                None => {
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
}
