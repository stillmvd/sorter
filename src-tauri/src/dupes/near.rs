use super::matcher::{self, Scores, AUDIO_HAM, FRAME_HAM};
use super::print::{self, Print, FPS};
use super::index::Index;
#[cfg(test)]
use super::files;
use super::{scope_files, FileInfo, Scope};
use crate::hints::{self, Engine};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::{HashMap, HashSet, VecDeque};
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

pub fn unprinted(conn: &Connection, all: Vec<FileInfo>) -> rusqlite::Result<Vec<FileInfo>> {
    let known: HashMap<String, (i64, i64, String)> = conn
        .prepare("SELECT path, size, mtime, state FROM fingerprint")?
        .query_map([], |r| Ok((r.get(0)?, (r.get(1)?, r.get(2)?, r.get(3)?))))?
        .collect::<Result<_, _>>()?;
    Ok(all
        .into_iter()
        .filter(|f| match known.get(&f.path) {
            Some((size, mtime, state)) => state == "new" || *size != f.size || *mtime != f.mtime,
            None => true,
        })
        .collect())
}

#[cfg(test)]
pub fn next_file(conn: &Connection) -> rusqlite::Result<Option<FileInfo>> {
    Ok(unprinted(conn, files(conn)?)?.into_iter().next())
}

#[cfg(test)]
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

pub fn embed_many(engine: Option<&mut Engine>, prints: &[Option<Print>]) -> Vec<Vec<Vec<f32>>> {
    let empty = || prints.iter().map(|_| Vec::new()).collect();
    let Some(engine) = engine else { return empty() };
    let stills: Vec<image::RgbImage> = prints.iter().flatten().flat_map(|p| p.stills.iter().cloned()).collect();
    let mut flat = Vec::with_capacity(stills.len());
    for chunk in stills.chunks(BATCH * 2) {
        match engine.embed(chunk) {
            Ok(v) => flat.extend(v),
            Err(_) => return empty(),
        }
    }
    let mut rest = flat.into_iter();
    prints.iter().map(|p| p.as_ref().map(|p| rest.by_ref().take(p.stills.len()).collect()).unwrap_or_default()).collect()
}

const IDLE: Duration = Duration::from_secs(5);
const RATE_KEYS: [&str; 2] = ["dupes_rate_video_ms", "dupes_rate_photo_ms"];
const RATE_DEFAULTS: [i64; 2] = [840, 200];

pub fn rates(conn: &Connection) -> rusqlite::Result<(i64, i64)> {
    let mut out = RATE_DEFAULTS;
    for (i, key) in RATE_KEYS.iter().enumerate() {
        if let Some(v) = crate::db::get_setting(conn, key)?.and_then(|v| v.parse().ok()) {
            out[i] = v;
        }
    }
    Ok((out[0], out[1]))
}

type Key = (Scope, Option<String>, Option<String>);

fn key(conn: &Connection) -> rusqlite::Result<Key> {
    Ok((Scope::work(conn)?, crate::db::get_setting(conn, "deck_path")?, crate::db::get_setting(conn, "table_path")?))
}

pub struct Printer {
    index: Option<Index>,
    version: i64,
    idle_until: Option<Instant>,
    queue: VecDeque<FileInfo>,
    key: Option<Key>,
    rates: [(f64, u32); 2],
    pub spent: [Duration; 4],
}

impl Printer {
    pub fn new() -> Self {
        Printer { index: None, version: -1, idle_until: None, queue: VecDeque::new(), key: None, rates: [(0.0, 0); 2], spent: [Duration::ZERO; 4] }
    }

    pub fn step(&mut self, conn: &Connection, engine: Option<&mut Engine>) -> rusqlite::Result<Option<(usize, bool)>> {
        let now = key(conn)?;
        if self.key.as_ref() != Some(&now) {
            self.queue.clear();
            self.idle_until = None;
            self.key = Some(now.clone());
        }
        if now.0 == Scope::Off || self.idle_until.is_some_and(|t| Instant::now() < t) {
            return Ok(None);
        }
        if self.queue.is_empty() {
            self.queue = unprinted(conn, scope_files(conn, &now.0)?)?.into();
        }
        let mut batch = Vec::new();
        while batch.len() < crate::develop::workers() {
            let Some(f) = self.queue.pop_front() else { break };
            if Path::new(&f.path).is_file() {
                batch.push(f);
            }
        }
        if batch.is_empty() {
            self.idle_until = Some(Instant::now() + IDLE);
            return Ok(None);
        }
        let started = Instant::now();
        let prints: Vec<Option<Print>> = std::thread::scope(|scope| {
            let handles: Vec<_> = batch
                .iter()
                .map(|f| {
                    scope.spawn(move || {
                        crate::develop::prepare_thread();
                        print::print(Path::new(&f.path))
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().ok().flatten()).collect()
        });
        self.spent[0] += started.elapsed();
        let t = Instant::now();
        let vectors = embed_many(engine, &prints);
        self.spent[1] += t.elapsed();
        let t = Instant::now();
        if key(conn)? != now {
            return Ok(Some((0, false)));
        }
        let tx = conn.unchecked_transaction()?;
        for ((f, p), v) in batch.iter().zip(&prints).zip(&vectors) {
            save_print(&tx, f, p.as_ref(), v)?;
        }
        tx.commit()?;
        self.spent[2] += t.elapsed();
        let t = Instant::now();
        let version: i64 = conn.query_row("PRAGMA data_version", [], |r| r.get(0))?;
        match self.index.as_mut() {
            None => self.index = Some(Index::load(conn)?),
            Some(index) if version != self.version => index.sync(conn)?,
            Some(_) => {}
        }
        self.version = version;
        let index = self.index.as_mut().unwrap();
        let mut found = Vec::new();
        for ((f, p), v) in batch.iter().zip(prints).zip(vectors) {
            let Some(p) = p else { continue };
            index.remove(&f.path);
            let me = Entry {
                path: f.path.clone(),
                duration_ms: p.duration_ms,
                frames: p.frames,
                audio: p.audio,
                mean: hints::mean(&v).unwrap_or_default(),
                look: p.look,
            };
            let audio_set: HashSet<u32> = me.audio.iter().copied().collect();
            for other in index.candidates(&me) {
                if !Path::new(&other.path).is_file() {
                    continue;
                }
                if let Some(pair) = compare(conn, &me, other, &audio_set)? {
                    if pair.confidence >= STORE_MIN {
                        found.push((me.path.clone(), other.path.clone(), pair));
                    }
                }
            }
            index.insert(me);
        }
        self.spent[3] += t.elapsed();
        let mut changed = false;
        if !found.is_empty() {
            let tx = conn.unchecked_transaction()?;
            for (a, b, pair) in &found {
                changed |= store_pair(&tx, a, b, pair)?;
            }
            tx.commit()?;
        }
        let per = started.elapsed() / batch.len() as u32;
        for f in &batch {
            self.learn(conn, crate::deck::photo_name(&f.path), per)?;
        }
        Ok(Some((batch.len(), changed)))
    }

    fn learn(&mut self, conn: &Connection, photo: bool, took: Duration) -> rusqlite::Result<()> {
        let ms = took.as_millis() as f64;
        let (rate, n) = &mut self.rates[photo as usize];
        *rate = if *n == 0 { ms } else { *rate * 0.9 + ms * 0.1 };
        *n += 1;
        if *n % 25 == 0 {
            crate::db::set_setting(conn, RATE_KEYS[photo as usize], &(rate.round() as i64).to_string())?;
        }
        Ok(())
    }
}

#[cfg(all(test, windows))]
mod tests {
    #[test]
    #[ignore]
    fn bench_next_file() {
        let Ok(db) = std::env::var("SORTER_DB") else { return };
        let conn = Connection::open(db).unwrap();
        let t = std::time::Instant::now();
        for _ in 0..10 {
            next_file(&conn).unwrap();
        }
        println!("next_file: {:?}", t.elapsed() / 10);
        let t = std::time::Instant::now();
        let f = next_file(&conn).unwrap().unwrap();
        let _ = crate::dupes::print::print(Path::new(&f.path));
        println!("print: {:?}", t.elapsed());
    }

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
        while let Some((k, changed)) = printer.step(&conn, engine.as_mut()).unwrap() {
            n += k;
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

    #[test]
    #[ignore]
    fn bench_dupes_stage() {
        let (deck, table) = (r"G:\sorter-test\big", r"G:\sorter-test\big-table");
        if !Path::new(deck).is_dir() {
            return;
        }
        unsafe {
            let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_MULTITHREADED);
            let _ = windows::Win32::Media::MediaFoundation::MFStartup(
                windows::Win32::Media::MediaFoundation::MF_VERSION,
                windows::Win32::Media::MediaFoundation::MFSTARTUP_FULL,
            );
        }
        let dir = std::env::temp_dir().join("sorter-bench-dupes");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let conn = Connection::open(dir.join("db.sqlite")).unwrap();
        crate::db::init(&conn).unwrap();
        crate::db::set_setting(&conn, "deck_path", deck).unwrap();
        crate::db::set_setting(&conn, "table_path", table).unwrap();
        crate::db::set_setting(&conn, "dupes_enabled", "1").unwrap();
        crate::db::set_setting(&conn, "dupes_scope", "all").unwrap();
        crate::deck::sync(&conn, deck).unwrap();
        crate::piles::sync(&conn, table).unwrap();
        let runtime = std::env::var("APPDATA").map(|a| Path::new(&a).join("com.stillmvd.sorter").join("runtime")).unwrap();
        let mut engine = Engine::load(&runtime).ok();
        let t = Instant::now();
        let all = files(&conn).unwrap();
        println!("ЗАМЕР движок {} · files() {} шт за {:?}", engine.is_some(), all.len(), t.elapsed());
        let n: usize = std::env::var("SORTER_BENCH_N").ok().and_then(|v| v.parse().ok()).unwrap_or(200);
        let mut index = Index::load(&conn).unwrap();
        let (mut t_next, mut t_print, mut t_embed, mut t_save, mut t_cmp) = (Duration::ZERO, Duration::ZERO, Duration::ZERO, Duration::ZERO, Duration::ZERO);
        let (mut videos, mut photos, mut done) = (0, 0, 0);
        let total = Instant::now();
        for _ in 0..n {
            let t = Instant::now();
            let Some(file) = next_file(&conn).unwrap() else { break };
            t_next += t.elapsed();
            if crate::deck::photo_name(&file.path) {
                photos += 1;
            } else {
                videos += 1;
            }
            done += 1;
            let t = Instant::now();
            let print = print::print(Path::new(&file.path));
            t_print += t.elapsed();
            let t = Instant::now();
            let vectors = print.as_ref().map(|p| embed(engine.as_mut(), p)).unwrap_or_default();
            t_embed += t.elapsed();
            let t = Instant::now();
            save_print(&conn, &file, print.as_ref(), &vectors).unwrap();
            t_save += t.elapsed();
            let Some(p) = print else { continue };
            let t = Instant::now();
            let me = Entry { path: file.path.clone(), duration_ms: p.duration_ms, frames: p.frames, audio: p.audio, mean: hints::mean(&vectors).unwrap_or_default(), look: p.look };
            let audio_set: HashSet<u32> = me.audio.iter().copied().collect();
            for other in index.candidates(&me) {
                let _ = compare(&conn, &me, other, &audio_set).unwrap();
            }
            index.insert(me);
            t_cmp += t.elapsed();
        }
        let per = |d: Duration| d.as_millis() as f64 / done.max(1) as f64;
        println!(
            "ЗАМЕР {done} файлов ({videos} видео, {photos} фото) за {:?}; на файл, мс: next_file {:.1} · print {:.1} · embed {:.1} · save {:.1} · compare {:.1}",
            total.elapsed(),
            per(t_next),
            per(t_print),
            per(t_embed),
            per(t_save),
            per(t_cmp)
        );
        let t = Instant::now();
        let pairs = crate::dupes::exact_pairs(&conn, files(&conn).unwrap()).unwrap();
        println!("ЗАМЕР exact_pairs (SHA) {} пар за {:?}", pairs.len(), t.elapsed());
    }

    #[test]
    #[ignore]
    fn bench_photo_pairs() {
        let Ok(db) = std::env::var("SORTER_DB") else { return };
        let conn = Connection::open(db).unwrap();
        let mut photos: Vec<(i64, Entry)> = entries_with_rows(&conn).unwrap().into_iter().filter(|(_, e)| crate::deck::photo_name(&e.path)).collect();
        photos.sort_by_key(|(row, _)| *row);
        let n: usize = std::env::var("SORTER_BENCH_N").ok().and_then(|v| v.parse().ok()).unwrap_or(photos.len()).min(photos.len());
        let (mut pairs, mut same, mut close, mut crops) = (0u64, 0u64, 0u64, 0u64);
        let mut distinct: HashSet<(u64, u64)> = HashSet::new();
        let (mut t_cheap, mut t_contains) = (Duration::ZERO, Duration::ZERO);
        let total = Instant::now();
        for i in 0..n {
            let a = &photos[i].1;
            for (_, b) in &photos[..i] {
                pairs += 1;
                let t = Instant::now();
                let ham = match (a.frames.first(), b.frames.first()) {
                    (Some(x), Some(y)) => (x ^ y).count_ones(),
                    _ => 64,
                };
                let semantic = if a.mean.is_empty() || b.mean.is_empty() { 0.0 } else { hints::dot(&a.mean, &b.mean) };
                t_cheap += t.elapsed();
                if ham <= PHOTO_HAM {
                    same += 1;
                    continue;
                }
                if semantic < PHOTO_CANDIDATE {
                    continue;
                }
                close += 1;
                let (ha, hb) = (a.frames.first().copied().unwrap_or(0), b.frames.first().copied().unwrap_or(0));
                distinct.insert((ha.min(hb), ha.max(hb)));
                let t = Instant::now();
                if compare_photos(a, b).is_some() {
                    crops += 1;
                }
                t_contains += t.elapsed();
            }
            if (i + 1) % 250 == 0 {
                println!("{} фото · {:?} · пар {pairs} · до contains {close} · contains {:?}", i + 1, total.elapsed(), t_contains);
            }
        }
        println!(
            "ЗАМЕР-ФОТО {n} фото за {:?}: пар {pairs}, same {same}, до contains {close} ({:.1} мс на пару), кадрирований {crops}, разных пар картинок {}; дешёвая часть {:?}, contains {:?}",
            total.elapsed(),
            t_contains.as_secs_f64() * 1000.0 / close.max(1) as f64,
            distinct.len(),
            t_cheap,
            t_contains
        );
    }

    #[test]
    #[ignore]
    fn bench_printer() {
        let (deck, table) = (r"G:\sorter-test\big", r"G:\sorter-test\big-table");
        if !Path::new(deck).is_dir() {
            return;
        }
        unsafe {
            let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_MULTITHREADED);
        }
        crate::develop::prepare_thread();
        let dir = std::env::temp_dir().join("sorter-bench-printer");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let conn = Connection::open(dir.join("db.sqlite")).unwrap();
        crate::db::init(&conn).unwrap();
        crate::db::set_setting(&conn, "deck_path", deck).unwrap();
        crate::db::set_setting(&conn, "table_path", table).unwrap();
        crate::db::set_setting(&conn, "dupes_enabled", "1").unwrap();
        crate::db::set_setting(&conn, "dupes_scope", "all").unwrap();
        crate::deck::sync(&conn, deck).unwrap();
        crate::piles::sync(&conn, table).unwrap();
        let runtime = std::env::var("APPDATA").map(|a| Path::new(&a).join("com.stillmvd.sorter").join("runtime")).unwrap();
        let mut engine = Engine::load(&runtime).ok();
        let n: usize = std::env::var("SORTER_BENCH_N").ok().and_then(|v| v.parse().ok()).unwrap_or(200);
        let mut printer = Printer::new();
        let (mut done, mut worst, mut mark) = (0, Duration::ZERO, 250);
        let total = Instant::now();
        let mut window = (Instant::now(), [Duration::ZERO; 4]);
        while done < n {
            let t = Instant::now();
            let Some((k, _)) = printer.step(&conn, engine.as_mut()).unwrap() else { break };
            worst = worst.max(t.elapsed());
            done += k;
            if done >= mark {
                let d: Vec<String> = printer.spent.iter().zip(window.1).map(|(a, b)| format!("{:.0}", (*a - b).as_secs_f64())).collect();
                let photos: i64 = conn
                    .query_row("SELECT COUNT(*) FROM (SELECT path FROM fingerprint WHERE state != 'new' ORDER BY rowid DESC LIMIT 250) WHERE lower(path) LIKE '%.jpg' OR lower(path) LIKE '%.png'", [], |r| r.get(0))
                    .unwrap();
                println!("ОКНО {done}: {:.0} с на 250 · фото {photos} · декод/эмбед/запись/сравнение {} с", window.0.elapsed().as_secs_f64(), d.join("/"));
                window = (Instant::now(), printer.spent);
                mark += 250;
            }
        }
        println!(
            "ЗАМЕР-ПОСЛЕ движок {} · потоков {} · {done} файлов за {:?} = {:.1} мс на файл · самая долгая пачка {:?}",
            engine.is_some(),
            crate::develop::workers(),
            total.elapsed(),
            total.elapsed().as_millis() as f64 / done.max(1) as f64,
            worst
        );
    }
}
