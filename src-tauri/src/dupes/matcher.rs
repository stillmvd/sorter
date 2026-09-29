pub const FRAME_HAM: u32 = 12;
pub const AUDIO_HAM: u32 = 10;
const SAME_MIN: f32 = 0.6;
const CROP_AUDIO: f32 = 0.9;
const CROP_SEMANTIC: f32 = 0.85;
const TRIM_MS: i64 = 1000;

pub trait Bits: Copy {
    fn ham(self, other: Self) -> u32;
}

impl Bits for u64 {
    fn ham(self, other: Self) -> u32 {
        (self ^ other).count_ones()
    }
}

impl Bits for u32 {
    fn ham(self, other: Self) -> u32 {
        (self ^ other).count_ones()
    }
}

pub fn align<T: Bits>(a: &[T], b: &[T], max_ham: u32) -> (f32, i64) {
    if a.is_empty() || b.is_empty() {
        return (0.0, 0);
    }
    if a.len() < b.len() {
        let (s, o) = align(b, a, max_ham);
        return (s, -o);
    }
    let mut votes = vec![0u32; a.len() + b.len()];
    for (i, x) in a.iter().enumerate() {
        for (j, y) in b.iter().enumerate() {
            if x.ham(*y) <= max_ham {
                votes[i + b.len() - j] += 1;
            }
        }
    }
    let (peak, count) = votes.iter().enumerate().max_by_key(|(k, v)| (**v, std::cmp::Reverse(*k))).unwrap();
    if *count == 0 {
        return (0.0, 0);
    }
    let off = peak as i64 - b.len() as i64;
    let hits = b
        .iter()
        .enumerate()
        .filter(|(j, y)| {
            let i = *j as i64 + off;
            (i - 1..=i + 1).any(|k| k >= 0 && (k as usize) < a.len() && a[k as usize].ham(**y) <= max_ham)
        })
        .count();
    (hits as f32 / b.len() as f32, off)
}

fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

pub fn semantic_score(a: &[Vec<f32>], b: &[Vec<f32>]) -> f32 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let (a, b) = if a.len() < b.len() { (b, a) } else { (a, b) };
    let need = ((b.len() as f32 * 0.8).ceil() as usize).max(1);
    let mut best = 0.0f32;
    for off in -(b.len() as i64) + 1..a.len() as i64 {
        let vals: Vec<f32> = (0..b.len() as i64)
            .filter_map(|j| {
                let i = j + off;
                (i >= 0 && (i as usize) < a.len()).then(|| dot(&b[j as usize], &a[i as usize]))
            })
            .collect();
        if vals.len() >= need {
            best = best.max(vals.iter().sum::<f32>() / vals.len() as f32);
        }
    }
    best
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Exact,
    Same,
    Trim,
    Crop,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Exact => "exact",
            Kind::Same => "same",
            Kind::Trim => "trim",
            Kind::Crop => "crop",
        }
    }
}

pub struct Scores {
    pub exact: bool,
    pub visual: f32,
    pub audio: f32,
    pub semantic: f32,
    pub duration_a: i64,
    pub duration_b: i64,
}

pub fn decide(s: &Scores) -> Option<(Kind, i64)> {
    if s.exact {
        return Some((Kind::Exact, 100));
    }
    if s.visual >= SAME_MIN {
        let kind = if (s.duration_a - s.duration_b).abs() > TRIM_MS { Kind::Trim } else { Kind::Same };
        let bonus = if s.audio >= CROP_AUDIO { 5.0 } else { 0.0 };
        return Some((kind, ((60.0 + 40.0 * s.visual + bonus).round() as i64).min(99)));
    }
    if s.audio >= CROP_AUDIO && s.semantic >= CROP_SEMANTIC {
        let c = 50.0 + 25.0 * s.audio + 25.0 * (s.semantic - CROP_SEMANTIC) / (1.0 - CROP_SEMANTIC);
        return Some((Kind::Crop, (c.round() as i64).min(99)));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noise(seed: &mut u64) -> u64 {
        *seed ^= *seed << 13;
        *seed ^= *seed >> 7;
        *seed ^= *seed << 17;
        *seed
    }

    fn flip(x: u64, bits: u32, seed: &mut u64) -> u64 {
        (0..bits).fold(x, |x, _| x ^ (1 << (noise(seed) % 64)))
    }

    #[test]
    fn finds_shift_of_subsequence() {
        let mut seed = 7;
        let a: Vec<u64> = (0..120).map(|_| noise(&mut seed)).collect();
        let b = a[40..68].to_vec();
        assert_eq!(align(&a, &b, FRAME_HAM), (1.0, 40));
        assert_eq!(align(&b, &a, FRAME_HAM), (1.0, -40));
    }

    #[test]
    fn noise_passes_random_does_not() {
        let mut seed = 99;
        let a: Vec<u64> = (0..80).map(|_| noise(&mut seed)).collect();
        let noisy: Vec<u64> = a.iter().map(|x| flip(*x, 5, &mut seed)).collect();
        assert!(align(&a, &noisy, FRAME_HAM).0 > 0.95);
        let other: Vec<u64> = (0..80).map(|_| noise(&mut seed)).collect();
        assert!(align(&a, &other, FRAME_HAM).0 < 0.1);
    }

    #[test]
    fn semantic_needs_overlap() {
        let v = |x: f32| vec![x, (1.0 - x * x).sqrt()];
        let a: Vec<Vec<f32>> = (0..10).map(|i| v(i as f32 / 10.0)).collect();
        assert!((semantic_score(&a, &a) - 1.0).abs() < 1e-4);
        assert!((semantic_score(&a, &a[2..7]) - 1.0).abs() < 1e-4);
    }

    fn s(visual: f32, audio: f32, semantic: f32) -> Scores {
        Scores { exact: false, visual, audio, semantic, duration_a: 20_000, duration_b: 20_000 }
    }

    #[test]
    fn decision_rule() {
        assert_eq!(decide(&s(0.1, 0.76, 0.55)), None);
        assert_eq!(decide(&s(0.1, 0.1, 0.83)), None);
        let (kind, c) = decide(&s(0.2, 1.0, 0.91)).unwrap();
        assert_eq!(kind, Kind::Crop);
        assert!(c >= 80);
        assert_eq!(decide(&s(0.95, 1.0, 0.9)).unwrap(), (Kind::Same, 99));
        let trim = Scores { duration_b: 7_000, ..s(0.97, 0.0, 0.0) };
        assert_eq!(decide(&trim).unwrap().0, Kind::Trim);
        assert_eq!(decide(&Scores { exact: true, ..s(0.0, 0.0, 0.0) }).unwrap(), (Kind::Exact, 100));
        assert_eq!(decide(&s(0.57, 0.5, 0.7)), None);
    }
}
