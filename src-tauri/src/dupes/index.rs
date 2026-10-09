use super::matcher::{FRAME_HAM, SAME_MIN};
use super::near::{self, Entry, AUDIO_HITS};
use rusqlite::Connection;
use std::collections::{HashMap, HashSet};

const CHUNKS: usize = 4;
const BUCKETS: usize = 1 << 16;
const RADIUS: u32 = FRAME_HAM / CHUNKS as u32;

struct Slot {
    entry: Entry,
    alive: bool,
    photo: bool,
    distinct: Vec<(u64, u32)>,
}

pub struct Index {
    slots: Vec<Slot>,
    by_path: HashMap<String, usize>,
    rows: HashMap<String, i64>,
    frames: Vec<Vec<(u32, u32)>>,
    audio: HashMap<u32, Vec<(u32, u32)>>,
    photos: Vec<u32>,
    probes: Vec<u16>,
}

fn counted<T: Copy + Eq + std::hash::Hash>(items: &[T]) -> Vec<(T, u32)> {
    let mut map: HashMap<T, u32> = HashMap::new();
    for &x in items {
        *map.entry(x).or_default() += 1;
    }
    map.into_iter().collect()
}

fn chunk(h: u64, c: usize) -> usize {
    (h >> (16 * c)) as u16 as usize
}

impl Index {
    pub fn new(entries: Vec<Entry>) -> Self {
        let mut index = Index {
            slots: Vec::with_capacity(entries.len()),
            by_path: HashMap::new(),
            rows: HashMap::new(),
            frames: vec![Vec::new(); CHUNKS * BUCKETS],
            audio: HashMap::new(),
            photos: Vec::new(),
            probes: (0..=u16::MAX).filter(|m| m.count_ones() <= RADIUS).collect(),
        };
        for e in entries {
            index.insert(e);
        }
        index
    }

    pub fn insert(&mut self, entry: Entry) {
        self.remove(&entry.path);
        let id = self.slots.len() as u32;
        let photo = crate::deck::photo_name(&entry.path);
        let distinct = if photo { Vec::new() } else { counted(&entry.frames) };
        if photo {
            self.photos.push(id);
        } else {
            for (d, &(h, _)) in distinct.iter().enumerate() {
                for c in 0..CHUNKS {
                    self.frames[c * BUCKETS + chunk(h, c)].push((id, d as u32));
                }
            }
            for (h, n) in counted(&entry.audio) {
                self.audio.entry(h).or_default().push((id, n));
            }
        }
        self.by_path.insert(entry.path.clone(), id as usize);
        self.slots.push(Slot { entry, alive: true, photo, distinct });
    }

    pub fn load(conn: &Connection) -> rusqlite::Result<Self> {
        let mut index = Index::new(Vec::new());
        for (row, e) in near::entries_with_rows(conn)? {
            let path = e.path.clone();
            index.insert(e);
            index.rows.insert(path, row);
        }
        Ok(index)
    }

    pub fn sync(&mut self, conn: &Connection) -> rusqlite::Result<()> {
        let live: HashMap<String, i64> = conn
            .prepare("SELECT path, rowid FROM fingerprint WHERE state = 'ok'")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?;
        if self.slots.len() > 2 * live.len().max(64) {
            *self = Index::load(conn)?;
            return Ok(());
        }
        let gone: Vec<String> = self.by_path.keys().filter(|p| !live.contains_key(*p)).cloned().collect();
        for p in &gone {
            self.remove(p);
        }
        let stale: Vec<(String, i64)> =
            live.into_iter().filter(|(p, row)| !self.by_path.contains_key(p) || self.rows.get(p) != Some(row)).collect();
        for (p, row) in stale {
            if let Some(e) = near::entry(conn, &p)? {
                self.insert(e);
                self.rows.insert(p, row);
            }
        }
        Ok(())
    }

    pub fn remove(&mut self, path: &str) {
        self.rows.remove(path);
        if let Some(i) = self.by_path.remove(path) {
            self.slots[i].alive = false;
        }
    }

    pub fn candidates(&self, me: &Entry) -> Vec<&Entry> {
        if crate::deck::photo_name(&me.path) {
            return self.photos.iter().map(|&i| &self.slots[i as usize]).filter(|s| s.alive).map(|s| &s.entry).collect();
        }
        let mine = counted(&me.frames);
        let mut theirs: HashMap<u32, HashSet<u32>> = HashMap::new();
        let mut ours: HashMap<u32, HashSet<usize>> = HashMap::new();
        for (k, &(h, _)) in mine.iter().enumerate() {
            for c in 0..CHUNKS {
                let v = chunk(h, c);
                for &m in &self.probes {
                    for &(e, d) in &self.frames[c * BUCKETS + (v ^ m as usize)] {
                        let slot = &self.slots[e as usize];
                        if slot.alive && (slot.distinct[d as usize].0 ^ h).count_ones() <= FRAME_HAM {
                            theirs.entry(e).or_default().insert(d);
                            ours.entry(e).or_default().insert(k);
                        }
                    }
                }
            }
        }
        let mut picked: HashSet<u32> = HashSet::new();
        for (e, ds) in &theirs {
            let slot = &self.slots[*e as usize];
            let (hits, len) = if slot.entry.frames.len() <= me.frames.len() {
                (ds.iter().map(|&d| slot.distinct[d as usize].1).sum::<u32>(), slot.entry.frames.len())
            } else {
                (ours[e].iter().map(|&k| mine[k].1).sum::<u32>(), me.frames.len())
            };
            if hits as f32 / len as f32 >= SAME_MIN {
                picked.insert(*e);
            }
        }
        let mut heard: HashMap<u32, u32> = HashMap::new();
        for h in me.audio.iter().copied().collect::<HashSet<u32>>() {
            for &(e, n) in self.audio.get(&h).into_iter().flatten() {
                *heard.entry(e).or_default() += n;
            }
        }
        picked.extend(heard.into_iter().filter(|&(_, n)| n as usize >= AUDIO_HITS).map(|(e, _)| e));
        let mut out: Vec<u32> = picked.into_iter().filter(|&e| self.slots[e as usize].alive && !self.slots[e as usize].photo).collect();
        out.sort_unstable();
        out.into_iter().map(|e| &self.slots[e as usize].entry).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dupes::matcher;
    use crate::dupes::near::compare;
    use rusqlite::Connection;

    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }

        fn below(&mut self, n: u64) -> u64 {
            self.next() % n
        }

        fn flip(&mut self, h: u64, bits: u32) -> u64 {
            let mut h = h;
            let mut used = 0u64;
            while used.count_ones() < bits {
                used |= 1 << self.below(64);
            }
            h ^= used;
            h
        }
    }

    fn entry(path: String, frames: Vec<u64>, audio: Vec<u32>) -> Entry {
        let duration_ms = frames.len() as i64 * 250;
        Entry { path, duration_ms, frames, audio, mean: Vec::new(), look: Vec::new() }
    }

    fn library(rng: &mut Rng) -> Vec<Entry> {
        let mut out: Vec<Entry> = Vec::new();
        for i in 0..90 {
            let path = format!("v{i:03}.mp4");
            let base = i % 3 == 0 && !out.is_empty();
            if base {
                let src = &out[rng.below(out.len() as u64) as usize];
                let bits = rng.below(FRAME_HAM as u64 + 3) as u32;
                let start = rng.below(src.frames.len() as u64 / 3) as usize;
                let end = src.frames.len() - rng.below(src.frames.len() as u64 / 3) as usize;
                let frames = src.frames[start..end].iter().map(|&h| if rng.below(4) == 0 { rng.next() } else { rng.flip(h, bits) }).collect();
                let audio = if rng.below(2) == 0 { src.audio.clone() } else { (0..200).map(|_| rng.next() as u32).collect() };
                out.push(entry(path, frames, audio));
            } else {
                let len = 20 + rng.below(200) as usize;
                let mut frames = Vec::with_capacity(len);
                let mut h = rng.next();
                for _ in 0..len {
                    if rng.below(5) == 0 {
                        h = rng.next();
                    }
                    frames.push(if rng.below(3) == 0 { rng.flip(h, 2) } else { h });
                }
                let mut audio: Vec<u32> = (0..len * 5).map(|_| rng.next() as u32).collect();
                if rng.below(4) == 0 {
                    let shared = rng.below(3) as u32;
                    audio.extend(std::iter::repeat_n(shared, 1 + rng.below(4) as usize));
                }
                out.push(entry(path, frames, audio));
            }
        }
        out
    }

    #[test]
    fn index_never_drops_a_pair_the_full_scan_finds() {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::init(&conn).unwrap();
        let (mut found, mut heard) = (0, 0);
        for seed in 1..=6u64 {
            let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15));
            let all = library(&mut rng);
            let mut index = Index::new(Vec::new());
            for (j, me) in all.iter().enumerate() {
                let picked: HashSet<&str> = index.candidates(me).into_iter().map(|e| e.path.as_str()).collect();
                let set: HashSet<u32> = me.audio.iter().copied().collect();
                for other in &all[..j] {
                    if other.audio.iter().filter(|h| set.contains(h)).take(AUDIO_HITS).count() >= AUDIO_HITS {
                        heard += 1;
                        assert!(picked.contains(other.path.as_str()), "{} ↔ {}: звук потерялся", me.path, other.path);
                    }
                    if compare(&conn, me, other, &set).unwrap().is_some() {
                        found += 1;
                        assert!(picked.contains(other.path.as_str()), "{} ↔ {} потерялась", me.path, other.path);
                    }
                }
                index.insert(entry(me.path.clone(), me.frames.clone(), me.audio.clone()));
            }
        }
        assert!(found > 20 && heard > 20, "мало пар в наборе: {found} / {heard}");
    }

    #[test]
    fn worst_spread_of_differences_is_still_found() {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::init(&conn).unwrap();
        let mut rng = Rng(0x1234_5678_9ABC_DEF1);
        let frames: Vec<u64> = (0..120).map(|_| rng.next()).collect();
        let spread: u64 = (0..CHUNKS as u64).map(|c| 0b111u64 << (16 * c + 5)).fold(0, |a, m| a | m);
        assert_eq!(spread.count_ones(), FRAME_HAM);
        let original = entry("a.mp4".into(), frames.clone(), vec![]);
        let copy = entry("b.mp4".into(), frames.iter().map(|h| h ^ spread).collect(), vec![]);
        assert!(compare(&conn, &copy, &original, &HashSet::new()).unwrap().is_some());
        let index = Index::new(vec![original]);
        assert_eq!(index.candidates(&copy).len(), 1);
    }

    fn pairs_of(conn: &Connection, all: &[Entry], indexed: bool) -> (HashSet<(String, String)>, u128) {
        let started = std::time::Instant::now();
        let mut index = Index::new(Vec::new());
        let mut out = HashSet::new();
        for (j, me) in all.iter().enumerate() {
            let set: HashSet<u32> = me.audio.iter().copied().collect();
            let others: Vec<&Entry> = if indexed { index.candidates(me) } else { all[..j].iter().collect() };
            for other in others {
                if let Some(p) = compare(conn, me, other, &set).unwrap() {
                    if p.confidence >= crate::dupes::near::STORE_MIN {
                        out.insert((other.path.clone(), me.path.clone()));
                    }
                }
            }
            if indexed {
                index.insert(Entry { path: me.path.clone(), duration_ms: me.duration_ms, frames: me.frames.clone(), audio: me.audio.clone(), mean: me.mean.clone(), look: me.look.clone() });
            }
        }
        (out, started.elapsed().as_millis())
    }

    #[test]
    #[ignore]
    fn real_library_same_pairs_and_scale() {
        let Ok(db) = std::env::var("SORTER_DB") else { return };
        let conn = Connection::open(db).unwrap();
        let all = crate::dupes::near::entries(&conn).unwrap();
        let videos = all.iter().filter(|e| !crate::deck::photo_name(&e.path)).count();
        let (full, full_ms) = pairs_of(&conn, &all, false);
        let (fast, fast_ms) = pairs_of(&conn, &all, true);
        println!("реальные: {} отпечатков ({videos} видео), пар {} / {}, полный перебор {full_ms} мс, индекс {fast_ms} мс", all.len(), full.len(), fast.len());
        assert_eq!(full, fast);
        let mut index = Index::new(Vec::new());
        let (mut cands, mut slow) = (0usize, Vec::new());
        for me in &all {
            let set: HashSet<u32> = me.audio.iter().copied().collect();
            for other in index.candidates(me) {
                cands += 1;
                let t = std::time::Instant::now();
                let p = compare(&conn, me, other, &set).unwrap();
                let v = matcher::align(&me.frames, &other.frames, FRAME_HAM).0;
                let hits = other.audio.iter().filter(|h| set.contains(h)).take(AUDIO_HITS).count();
                slow.push((t.elapsed().as_micros(), me.frames.len(), other.frames.len(), me.audio.len(), other.audio.len(), v, hits, p.map(|p| p.kind)));
            }
            index.insert(Entry { path: me.path.clone(), duration_ms: me.duration_ms, frames: me.frames.clone(), audio: me.audio.clone(), mean: me.mean.clone(), look: me.look.clone() });
        }
        slow.sort_by_key(|s| std::cmp::Reverse(s.0));
        println!("кандидатов {cands} из {} пар, сумма {} мс", all.len() * (all.len() - 1) / 2, slow.iter().map(|s| s.0).sum::<u128>() / 1000);
        for s in slow.iter().take(8) {
            println!("  {:?}", s);
        }

        let listed = crate::dupes::files(&conn).unwrap();
        let differ = listed
            .iter()
            .filter(|f| {
                let m = std::fs::metadata(&f.path).unwrap();
                let mtime = m.modified().unwrap().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64;
                (m.len() as i64, mtime) != (f.size, f.mtime)
            })
            .count();
        println!("обход папок: {} файлов, расхождений с fs::metadata {differ}", listed.len());
        assert_eq!(differ, 0);
        for _ in 0..3 {
            let t = std::time::Instant::now();
            let n = crate::dupes::files(&conn).unwrap().len();
            println!("files(): {n} за {} мкс", t.elapsed().as_micros());
        }

        let mut real: Vec<&Entry> = all.iter().filter(|e| !crate::deck::photo_name(&e.path) && !e.frames.is_empty()).collect();
        real.sort_by_key(|e| std::cmp::Reverse(e.frames.len()));
        let lens: Vec<usize> = real.iter().map(|e| e.frames.len()).collect();
        println!("кадров у видео: макс {}, медиана {}", lens[0], lens[lens.len() / 2]);
        let mut rng = Rng(0xDEAD_BEEF_1234_5678);
        for n in [1000usize, 3000, 6000] {
            let fake: Vec<Entry> = (0..n)
                .map(|i| {
                    let src = real[i % (real.len() / 4).max(1)];
                    let (fm, am) = (rng.next(), rng.next() as u32);
                    entry(format!("fake{i}.mp4"), src.frames.iter().map(|h| h ^ fm).collect(), src.audio.iter().map(|h| h ^ am).collect())
                })
                .collect();
            let me = real[0];
            let set: HashSet<u32> = me.audio.iter().copied().collect();
            let t = std::time::Instant::now();
            for other in &fake {
                let _ = compare(&conn, me, other, &set).unwrap();
            }
            let linear = t.elapsed().as_millis();
            let t = std::time::Instant::now();
            let index = Index::new(fake);
            let built = t.elapsed().as_millis();
            let t = std::time::Instant::now();
            let picked = index.candidates(me);
            for other in &picked {
                let _ = compare(&conn, me, other, &set).unwrap();
            }
            let fast = t.elapsed().as_millis();
            println!("{n} видео: новый файл — полный перебор {linear} мс, индекс {fast} мс (кандидатов {}), сборка индекса {built} мс", picked.len());
        }
    }

    #[test]
    #[ignore]
    fn crop_search_dump() {
        let Ok(db) = std::env::var("SORTER_DB") else { return };
        let conn = Connection::open(db).unwrap();
        let photos: Vec<Entry> = crate::dupes::near::entries(&conn).unwrap().into_iter().filter(|e| crate::deck::photo_name(&e.path)).collect();
        let started = std::time::Instant::now();
        let mut lines = Vec::new();
        for (i, a) in photos.iter().enumerate() {
            for b in &photos[..i] {
                if !a.mean.is_empty() && !b.mean.is_empty() && crate::hints::dot(&a.mean, &b.mean) >= 0.75 {
                    let (ab, ba) = (crate::dupes::print::contains(&a.look, &b.look), crate::dupes::print::contains(&b.look, &a.look));
                    lines.push(format!("{} | {} | {:?} {:?}", a.path, b.path, ab, ba));
                }
            }
        }
        println!("пар с поиском кадрирования: {}, {} мс", lines.len(), started.elapsed().as_millis());
        std::fs::write(std::env::var("CROP_DUMP").unwrap(), lines.join("
")).unwrap();
    }

    #[test]
    fn sync_follows_moves_and_forgotten_prints() {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::init(&conn).unwrap();
        let frames: Vec<u8> = [7u64, 8, 9].iter().flat_map(|h| h.to_le_bytes()).collect();
        for p in ["a.mp4", "b.mp4"] {
            conn.execute("INSERT INTO fingerprint(path, size, mtime, frames, state) VALUES (?1, 1, 1, ?2, 'ok')", rusqlite::params![p, frames])
                .unwrap();
        }
        let mut index = Index::new(near::entries(&conn).unwrap());
        let me = entry("c.mp4".into(), vec![7, 8, 9], vec![]);
        conn.execute("UPDATE fingerprint SET path = 'Мемы/a.mp4' WHERE path = 'a.mp4'", []).unwrap();
        conn.execute("DELETE FROM fingerprint WHERE path = 'b.mp4'", []).unwrap();
        index.sync(&conn).unwrap();
        let paths: Vec<&str> = index.candidates(&me).into_iter().map(|e| e.path.as_str()).collect();
        assert_eq!(paths, vec!["Мемы/a.mp4"]);
    }

    #[test]
    fn sync_rereads_a_path_taken_over_by_another_file() {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::init(&conn).unwrap();
        let blob = |hs: &[u64]| -> Vec<u8> { hs.iter().flat_map(|h| h.to_le_bytes()).collect() };
        conn.execute("INSERT INTO fingerprint(path, size, mtime, frames, state) VALUES ('Мемы/x.mp4', 1, 1, ?1, 'ok')", rusqlite::params![blob(&[1, 2, 3])])
            .unwrap();
        conn.execute("INSERT INTO fingerprint(path, size, mtime, frames, state) VALUES ('deck/x.mp4', 1, 1, ?1, 'ok')", rusqlite::params![blob(&[u64::MAX; 3])])
            .unwrap();
        let mut index = Index::load(&conn).unwrap();
        conn.execute("UPDATE OR REPLACE fingerprint SET path = 'Мемы/x.mp4' WHERE path = 'deck/x.mp4'", []).unwrap();
        index.sync(&conn).unwrap();
        assert!(index.candidates(&entry("old.mp4".into(), vec![1, 2, 3], vec![])).is_empty());
        let paths: Vec<&str> = index.candidates(&entry("new.mp4".into(), vec![u64::MAX; 3], vec![])).into_iter().map(|e| e.path.as_str()).collect();
        assert_eq!(paths, vec!["Мемы/x.mp4"]);
    }

    #[test]
    fn reprinted_file_replaces_its_old_entry() {
        let mut index = Index::new(vec![entry("a.mp4".into(), vec![1, 2, 3], vec![]), entry("b.mp4".into(), vec![1, 2, 3], vec![])]);
        let me = entry("c.mp4".into(), vec![1, 2, 3], vec![]);
        assert_eq!(index.candidates(&me).len(), 2);
        index.insert(entry("a.mp4".into(), vec![u64::MAX; 3], vec![]));
        index.remove("b.mp4");
        assert!(index.candidates(&me).is_empty());
    }
}
