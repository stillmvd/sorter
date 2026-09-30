use super::matcher::{self, Scores, AUDIO_HAM, FRAME_HAM};
use super::print::{self, Print, FPS};
use super::index::Index;
use super::{files, FileInfo};
use crate::hints::{self, Engine};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::{Duration, Instant};

pub const STORE_MIN: i64 = 50;
const MEAN_MIN: f32 = 0.6;
pub const AUDIO_HITS: usize = 3;
const BATCH: usize = 16;

fn u64s(b: &[u8]) -> Vec<u64> {
    b.chunks_exact(8).map(|c| u64::from_le_bytes(c.try_into().unwrap())).collect()
}

fn u32s(b: &[u8]) -> Vec<u32> {
    b.chunks_exact(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect()
}

fn vectors(b: &[u8]) -> Vec<Vec<f32>> {
    hints::from_blob(b).chunks_exact(768).map(|c| c.to_vec()).collect()
}

pub struct Entry {
    pub path: String,
    pub duration_ms: i64,
    pub frames: Vec<u64>,
    pub audio: Vec<u32>,
    pub mean: Vec<f32>,
    pub look: Vec<u8>,
}

const ENTRY: &str = "SELECT path, duration_ms, frames, audio, mean, look, rowid FROM fingerprint WHERE state = 'ok'";

fn read_entry(r: &rusqlite::Row) -> rusqlite::Result<Entry> {
    Ok(Entry {
        path: r.get(0)?,
        duration_ms: r.get::<_, Option<i64>>(1)?.unwrap_or(0),
        frames: u64s(&r.get::<_, Option<Vec<u8>>>(2)?.unwrap_or_default()),
        audio: u32s(&r.get::<_, Option<Vec<u8>>>(3)?.unwrap_or_default()),
        mean: hints::from_blob(&r.get::<_, Option<Vec<u8>>>(4)?.unwrap_or_default()),
        look: r.get::<_, Option<Vec<u8>>>(5)?.unwrap_or_default(),
    })
}

#[cfg(test)]
pub fn entries(conn: &Connection) -> rusqlite::Result<Vec<Entry>> {
    conn.prepare(ENTRY)?.query_map([], read_entry)?.collect()
}

pub fn entries_with_rows(conn: &Connection) -> rusqlite::Result<Vec<(i64, Entry)>> {
    conn.prepare(ENTRY)?.query_map([], |r| Ok((r.get(6)?, read_entry(r)?)))?.collect()
}

pub fn entry(conn: &Connection, path: &str) -> rusqlite::Result<Option<Entry>> {
    conn.query_row(&format!("{ENTRY} AND path = ?1"), params![path], read_entry).optional()
}

fn semantic(conn: &Connection, path: &str) -> rusqlite::Result<Vec<Vec<f32>>> {
    let blob: Option<Vec<u8>> =
        conn.query_row("SELECT semantic FROM fingerprint WHERE path = ?1", params![path], |r| r.get(0)).optional()?.flatten();
    Ok(vectors(&blob.unwrap_or_default()))
}

pub struct Pair {
    pub kind: &'static str,
    pub confidence: i64,
    pub offset_ms: i64,
    pub visual: f32,
    pub audio: f32,
    pub semantic: f32,
}

const PHOTO_HAM: u32 = 8;
const PHOTO_CANDIDATE: f32 = 0.75;
const PHOTO_FIT: f32 = 0.98;
const PHOTO_PART: f32 = 0.93;

pub fn compare_photos(a: &Entry, b: &Entry) -> Option<Pair> {
    let ham = match (a.frames.first(), b.frames.first()) {
        (Some(x), Some(y)) => (x ^ y).count_ones(),
        _ => 64,
    };
    if ham <= PHOTO_HAM {
        return Some(Pair { kind: "same", confidence: 100 - 5 * ham as i64, offset_ms: 0, visual: 1.0 - ham as f32 / 64.0, audio: 0.0, semantic: 0.0 });
    }
    let semantic = if a.mean.is_empty() || b.mean.is_empty() { 0.0 } else { hints::dot(&a.mean, &b.mean) };
    if semantic < PHOTO_CANDIDATE {
        return None;
    }
    let (ab, ba) = (print::contains(&a.look, &b.look), print::contains(&b.look, &a.look));
    let (fit, part) = if ab.0 >= ba.0 { ab } else { ba };
    (fit >= PHOTO_FIT && part <= PHOTO_PART).then(|| Pair {
        kind: "crop",
        confidence: ((80.0 + 1000.0 * (fit - PHOTO_FIT)).round() as i64).min(99),
        offset_ms: 0,
        visual: fit,
        audio: 0.0,
        semantic,
    })
}

pub fn compare(conn: &Connection, a: &Entry, b: &Entry, audio_set: &HashSet<u32>) -> rusqlite::Result<Option<Pair>> {
    let (pa, pb) = (crate::deck::photo_name(&a.path), crate::deck::photo_name(&b.path));
    if pa || pb {
        return Ok(if pa && pb { compare_photos(a, b) } else { None });
    }
    let (visual, offset) = matcher::align(&a.frames, &b.frames, FRAME_HAM);
    let audio_hits = b.audio.iter().filter(|h| audio_set.contains(h)).take(AUDIO_HITS).count();
    let audio = if visual >= 0.6 || audio_hits >= AUDIO_HITS { matcher::align(&a.audio, &b.audio, AUDIO_HAM).0 } else { 0.0 };
    let semantic = if visual < 0.6 && audio >= 0.9 && hints::dot(&a.mean, &b.mean) > MEAN_MIN {
        matcher::semantic_score(&semantic(conn, &a.path)?, &semantic(conn, &b.path)?)
    } else {
        0.0
    };
    let scores = Scores { exact: false, visual, audio, semantic, duration_a: a.duration_ms, duration_b: b.duration_ms };
    Ok(matcher::decide(&scores).map(|(kind, confidence)| Pair {
        kind: kind.as_str(),
        confidence,
        offset_ms: offset * 1000 / FPS,
        visual,
        audio,
        semantic,
    }))
}

fn store_pair(conn: &Connection, a: &str, b: &str, p: &Pair) -> rusqlite::Result<bool> {
    let (a, b, offset) = if a < b { (a, b, p.offset_ms) } else { (b, a, -p.offset_ms) };
    let dismissed = conn.prepare("SELECT 1 FROM dupe_dismissed WHERE a = ?1 AND b = ?2")?.exists(params![a, b])?;
    if dismissed {
        return Ok(false);
    }
    let n = conn.execute(
        "INSERT INTO dupe(a, b, kind, confidence, offset_ms, visual, audio, semantic) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(a, b) DO UPDATE SET kind = excluded.kind, confidence = excluded.confidence, offset_ms = excluded.offset_ms,
           visual = excluded.visual, audio = excluded.audio, semantic = excluded.semantic WHERE dupe.kind != 'exact'",
        params![a, b, p.kind, p.confidence, offset, p.visual, p.audio, p.semantic],
    )?;
    Ok(n > 0)
}

pub fn save_print(conn: &Connection, f: &FileInfo, p: Option<&Print>, vectors: &[Vec<f32>]) -> rusqlite::Result<()> {
    let Some(p) = p else {
        conn.execute(
            "INSERT INTO fingerprint(path, size, mtime, state) VALUES (?1, ?2, ?3, 'failed')
             ON CONFLICT(path) DO UPDATE SET size = excluded.size, mtime = excluded.mtime, state = 'failed'",
            params![f.path, f.size, f.mtime],
        )?;
        return Ok(());
    };
    let frames: Vec<u8> = p.frames.iter().flat_map(|h| h.to_le_bytes()).collect();
    let audio: Vec<u8> = p.audio.iter().flat_map(|h| h.to_le_bytes()).collect();
    let semantic: Vec<u8> = vectors.iter().flat_map(|v| hints::to_blob(v)).collect();
    let mean = hints::mean(vectors).map(|m| hints::to_blob(&m));
    let bbox = format!("{},{},{},{}", p.bbox[0], p.bbox[1], p.bbox[2], p.bbox[3]);
    conn.execute(
        "INSERT INTO fingerprint(path, size, mtime, duration_ms, width, height, bitrate, box, frames, audio, semantic, mean, look, state)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, 'ok')
         ON CONFLICT(path) DO UPDATE SET
           sha256 = CASE WHEN size = excluded.size AND mtime = excluded.mtime THEN sha256 END,
           size = excluded.size, mtime = excluded.mtime, duration_ms = excluded.duration_ms, width = excluded.width,
           height = excluded.height, bitrate = excluded.bitrate, box = excluded.box, frames = excluded.frames,
           audio = excluded.audio, semantic = excluded.semantic, mean = excluded.mean, look = excluded.look, state = 'ok'",
        params![f.path, f.size, f.mtime, p.duration_ms, p.width, p.height, p.bitrate, bbox, frames, audio, semantic, mean, p.look],
    )?;
    Ok(())
}

pub fn next_file(conn: &Connection) -> rusqlite::Result<Option<FileInfo>> {
    let known: HashMap<String, (i64, i64, String)> = conn
        .prepare("SELECT path, size, mtime, state FROM fingerprint")?
        .query_map([], |r| Ok((r.get(0)?, (r.get(1)?, r.get(2)?, r.get(3)?))))?
        .collect::<Result<_, _>>()?;
    Ok(files(conn)?.into_iter().find(|f| match known.get(&f.path) {
        Some((size, mtime, state)) => state == "new" || *size != f.size || *mtime != f.mtime,
        None => true,
    }))
}

pub fn embed(engine: Option<&mut Engine>, p: &Print) -> Vec<Vec<f32>> {
    let Some(engine) = engine else { return Vec::new() };
    let mut out = Vec::with_capacity(p.stills.len());
    for chunk in p.stills.chunks(BATCH) {
        match engine.embed(chunk) {
            Ok(v) => out.extend(v),
            Err(_) => return Vec::new(),
        }
    }
    out
}

const IDLE: Duration = Duration::from_secs(5);

pub struct Printer {
    index: Option<Index>,
    version: i64,
    idle_until: Option<Instant>,
}

impl Printer {
    pub fn new() -> Self {
        Printer { index: None, version: -1, idle_until: None }
    }

    pub fn step(&mut self, conn: &Connection, engine: Option<&mut Engine>) -> rusqlite::Result<Option<bool>> {
        if self.idle_until.is_some_and(|t| Instant::now() < t) {
            return Ok(None);
        }
        let Some(file) = next_file(conn)? else {
            self.idle_until = Some(Instant::now() + IDLE);
            return Ok(None);
        };
        self.idle_until = None;
        let print = print::print(Path::new(&file.path));
        let vectors = print.as_ref().map(|p| embed(engine, p)).unwrap_or_default();
        save_print(conn, &file, print.as_ref(), &vectors)?;
        let Some(p) = print else { return Ok(Some(false)) };
        let version: i64 = conn.query_row("PRAGMA data_version", [], |r| r.get(0))?;
        match self.index.as_mut() {
            None => self.index = Some(Index::load(conn)?),
            Some(index) if version != self.version => index.sync(conn)?,
            Some(_) => {}
        }
        self.version = version;
        let index = self.index.as_mut().unwrap();
        index.remove(&file.path);
        let me = Entry {
            path: file.path.clone(),
            duration_ms: p.duration_ms,
            frames: p.frames,
            audio: p.audio,
            mean: hints::mean(&vectors).unwrap_or_default(),
            look: p.look,
        };
        let audio_set: HashSet<u32> = me.audio.iter().copied().collect();
        let mut changed = false;
        for other in index.candidates(&me) {
            if !Path::new(&other.path).is_file() {
                continue;
            }
            if let Some(pair) = compare(conn, &me, other, &audio_set)? {
                if pair.confidence >= STORE_MIN {
                    changed |= store_pair(conn, &me.path, &other.path, &pair)?;
                }
            }
        }
        index.insert(me);
        Ok(Some(changed))
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    fn entry(path: &Path) -> Option<(Entry, Print)> {
        let p = print::print(path)?;
        Some((
            Entry { path: path.to_string_lossy().into_owned(), duration_ms: p.duration_ms, frames: p.frames.clone(), audio: p.audio.clone(), mean: Vec::new(), look: Vec::new() },
            p,
        ))
    }

    #[test]
    #[ignore]
    fn variants_find_original() {
        unsafe {
            let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_MULTITHREADED);
            let _ = windows::Win32::Media::MediaFoundation::MFStartup(
                windows::Win32::Media::MediaFoundation::MF_VERSION,
                windows::Win32::Media::MediaFoundation::MFSTARTUP_FULL,
            );
        }
        let dir = Path::new(r"G:\sorter-test\variants");
        let Some((orig, _)) = entry(&dir.join("orig.mp4")) else { return };
        let runtime = std::env::var("APPDATA").map(|a| Path::new(&a).join("com.stillmvd.sorter").join("runtime")).unwrap();
        let mut engine = Engine::load(&runtime).ok();
        let conn = Connection::open_in_memory().unwrap();
        crate::db::init(&conn).unwrap();
        let (_, op) = entry(&dir.join("orig.mp4")).unwrap();
        let ov = embed(engine.as_mut(), &op);
        save_print(&conn, &FileInfo { path: orig.path.clone(), size: 0, mtime: 0 }, Some(&op), &ov).unwrap();
        let orig = Entry { mean: hints::mean(&ov).unwrap_or_default(), ..orig };
        let set: HashSet<u32> = orig.audio.iter().copied().collect();
        let mut weak = Vec::new();
        for name in ["v_compressed", "v_360p", "v_trim_12_19", "v_trim_3s", "v_crop80", "v_pad", "v_combo_mute"] {
            let path = dir.join(format!("{name}.mp4"));
            let Some((e, p)) = entry(&path) else {
                weak.push(format!("{name}: не прочитан"));
                continue;
            };
            let v = embed(engine.as_mut(), &p);
            save_print(&conn, &FileInfo { path: e.path.clone(), size: 0, mtime: 0 }, Some(&p), &v).unwrap();
            let e = Entry { mean: hints::mean(&v).unwrap_or_default(), ..e };
            let pair = compare(&conn, &e, &orig, &set).unwrap();
            match &pair {
                Some(p) => println!("{name}: {} {}% сдвиг {} мс · кадры {:.2} звук {:.2} смысл {:.2}", p.kind, p.confidence, p.offset_ms, p.visual, p.audio, p.semantic),
                None => println!("{name}: нет"),
            }
            if pair.map(|p| p.confidence).unwrap_or(0) < 80 {
                weak.push(name.to_string());
            }
        }
        assert!(weak.is_empty(), "не нашли ≥ 80 %: {weak:?}");
    }
}

#[cfg(all(test, windows))]
mod photos {
    use super::*;

    fn entry(engine: &mut Option<Engine>, path: &Path) -> Option<Entry> {
        let p = print::print_photo(path)?;
        let v = embed(engine.as_mut(), &p);
        Some(Entry { path: path.to_string_lossy().into_owned(), duration_ms: 0, frames: p.frames, audio: Vec::new(), mean: hints::mean(&v).unwrap_or_default(), look: p.look })
    }

    #[test]
    #[ignore]
    fn photo_variants_and_false_pairs() {
        let dir = Path::new(r"G:\sorter-test\photos");
        if !dir.exists() {
            return;
        }
        let runtime = std::env::var("APPDATA").map(|a| Path::new(&a).join("com.stillmvd.sorter").join("runtime")).unwrap();
        let mut engine = Engine::load(&runtime).ok();
        let orig = entry(&mut engine, &dir.join("dupe_orig.jpg")).unwrap();
        let mut weak = Vec::new();
        for name in ["dupe_half.jpg", "dupe_q60.jpg", "dupe_crop80.jpg"] {
            let e = entry(&mut engine, &dir.join(name)).unwrap();
            let pair = compare_photos(&e, &orig);
            println!("{name}: {:?} смысл {:.3} ham {}", pair.as_ref().map(|p| (p.kind, p.confidence)), hints::dot(&e.mean, &orig.mean), (e.frames[0] ^ orig.frames[0]).count_ones()); println!("  вписан {:?} look {} {}", print::contains(&orig.look, &e.look), orig.look.len(), e.look.len());
            if pair.map(|p| p.confidence).unwrap_or(0) < 80 {
                weak.push(name);
            }
        }
        let mut all = Vec::new();
        for e in std::fs::read_dir(r"G:\vk photos").unwrap().filter_map(|e| e.ok()) {
            if let Some(x) = entry(&mut engine, &e.path()) {
                all.push(x);
            }
        }
        let mut shown = Vec::new();
        for (i, a) in all.iter().enumerate() {
            for b in &all[i + 1..] {
                if let Some(p) = compare_photos(a, b) {
                    if p.confidence >= 80 {
                        shown.push(format!("{} | {} {} {} {:.3}", a.path, b.path, p.kind, p.confidence, p.semantic));
                    }
                }
            }
        }
        shown.iter().for_each(|s| println!("ПАРА {s}"));
        assert!(weak.is_empty(), "не нашли ≥ 80: {weak:?}");
    }
}

#[cfg(all(test, windows))]
mod probe {
    use super::*;

    #[test]
    #[ignore]
    fn repeat_decode() {
        unsafe {
            let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_MULTITHREADED);
            let _ = windows::Win32::Media::MediaFoundation::MFStartup(
                windows::Win32::Media::MediaFoundation::MF_VERSION,
                windows::Win32::Media::MediaFoundation::MFSTARTUP_FULL,
            );
        }
        let dir = Path::new(r"G:\sorter-test\variants");
        for i in 0..8 {
            for name in ["orig", "v_compressed", "v_trim_3s"] {
                let p = print::print(&dir.join(format!("{name}.mp4"))).unwrap();
                println!("{i} {name} {} {}", p.frames.len(), p.frames.iter().fold(0u64, |x, y| x ^ y));
            }
        }
    }

    #[test]
    #[ignore]
    fn real_library() {
        unsafe {
            let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_MULTITHREADED);
            let _ = windows::Win32::Media::MediaFoundation::MFStartup(
                windows::Win32::Media::MediaFoundation::MF_VERSION,
                windows::Win32::Media::MediaFoundation::MFSTARTUP_FULL,
            );
        }
        let conn = Connection::open(std::env::var("SORTER_DB").unwrap()).unwrap();
        crate::db::init(&conn).unwrap();
        let runtime = std::env::var("APPDATA").map(|a| Path::new(&a).join("com.stillmvd.sorter").join("runtime")).unwrap();
        let mut engine = Engine::load(&runtime).ok();
        let mut printer = Printer::new();
        let t = std::time::Instant::now();
        let mut n = 0;
        while let Some(changed) = printer.step(&conn, engine.as_mut()).unwrap() {
            n += 1;
            if changed || n % 20 == 0 {
                println!("{n} {:?} {changed}", t.elapsed());
            }
        }
        super::super::refresh_exact(&conn).unwrap();
        let mut q = conn.prepare("SELECT a, b, kind, confidence, visual, audio, semantic FROM dupe ORDER BY kind, confidence DESC").unwrap();
        let rows = q
            .query_map([], |r| Ok(format!("{} | {} | {} {} v{:.2} a{:.2} s{:.2}", r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, i64>(3)?, r.get::<_, f64>(4).unwrap_or(0.0), r.get::<_, f64>(5).unwrap_or(0.0), r.get::<_, f64>(6).unwrap_or(0.0))))
            .unwrap();
        for r in rows {
            println!("PAIR {}", r.unwrap());
        }
        println!("ИТОГО {n} файлов за {:?}", t.elapsed());
    }
}
