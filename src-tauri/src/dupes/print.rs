use image::{imageops, GrayImage, RgbImage};
use std::path::Path;

pub const FPS: i64 = 4;
pub const GRID: u32 = 128;
const STILL: u32 = 256;
const MAX_STILLS: i64 = 180;
const BORDER: u8 = 24;
pub const AUDIO_RATE: usize = 5512;
const WIN: usize = 2048;
pub const HOP: usize = 256;
const TICKS: i64 = 10_000_000;

pub struct Decoded {
    pub duration_ms: i64,
    pub width: u32,
    pub height: u32,
    pub rotation: u32,
    pub grays: Vec<GrayImage>,
    pub stills: Vec<RgbImage>,
    pub mono: Vec<f32>,
}

pub struct Print {
    pub duration_ms: i64,
    pub width: u32,
    pub height: u32,
    pub rotation: u32,
    pub bitrate: i64,
    pub bbox: [f32; 4],
    pub frames: Vec<u64>,
    pub audio: Vec<u32>,
    pub stills: Vec<RgbImage>,
}

pub fn bbox(grays: &[GrayImage]) -> [f32; 4] {
    let n = GRID as usize;
    let mut max = vec![0u8; n * n];
    for g in grays {
        max.iter_mut().zip(g.as_raw()).for_each(|(m, p)| *m = (*m).max(*p));
    }
    let rows: Vec<usize> = (0..n).filter(|y| max[y * n..(y + 1) * n].iter().any(|p| *p > BORDER)).collect();
    let cols: Vec<usize> = (0..n).filter(|x| (0..n).any(|y| max[y * n + x] > BORDER)).collect();
    if rows.len() < 8 || cols.len() < 8 {
        return [0.0, 1.0, 0.0, 1.0];
    }
    let f = |v: usize| v as f32 / n as f32;
    [f(rows[0]), f(rows[rows.len() - 1] + 1), f(cols[0]), f(cols[cols.len() - 1] + 1)]
}

fn region(b: [f32; 4], w: u32, h: u32) -> (u32, u32, u32, u32) {
    let (y0, y1) = ((b[0] * h as f32) as u32, ((b[1] * h as f32).ceil() as u32).min(h));
    let (x0, x1) = ((b[2] * w as f32) as u32, ((b[3] * w as f32).ceil() as u32).min(w));
    (x0, y0, (x1 - x0).max(1), (y1 - y0).max(1))
}

pub fn crop_rgb(img: &RgbImage, b: [f32; 4]) -> RgbImage {
    let (x, y, w, h) = region(b, img.width(), img.height());
    imageops::crop_imm(img, x, y, w, h).to_image()
}

fn dct_row(k: usize, n: usize) -> f32 {
    (std::f32::consts::PI * (2 * n + 1) as f32 * k as f32 / 64.0).cos()
}

pub fn phash(g: &GrayImage, b: [f32; 4]) -> u64 {
    let (x0, y0, w, h) = region(b, g.width(), g.height());
    let mut m = [[0f32; 32]; 32];
    for (y, row) in m.iter_mut().enumerate() {
        let (ya, yb) = (y0 + y as u32 * h / 32, (y0 + (y as u32 + 1) * h / 32).max(y0 + y as u32 * h / 32 + 1));
        for (x, cell) in row.iter_mut().enumerate() {
            let (xa, xb) = (x0 + x as u32 * w / 32, (x0 + (x as u32 + 1) * w / 32).max(x0 + x as u32 * w / 32 + 1));
            let mut sum = 0u32;
            for yy in ya..yb {
                for xx in xa..xb {
                    sum += g.get_pixel(xx, yy)[0] as u32;
                }
            }
            *cell = sum as f32 / ((yb - ya) * (xb - xa)) as f32;
        }
    }
    let mut low = [0f32; 64];
    for k in 1..9 {
        let t: Vec<f32> = (0..32).map(|x| (0..32).map(|n| dct_row(k, n) * m[n][x]).sum()).collect();
        for l in 1..9 {
            low[(k - 1) * 8 + l - 1] = (0..32).map(|x| t[x] * dct_row(l, x)).sum();
        }
    }
    let mut sorted = low;
    sorted.sort_by(|a, b| a.total_cmp(b));
    let median = (sorted[31] + sorted[32]) / 2.0;
    low.iter().fold(0u64, |acc, v| (acc << 1) | (*v > median) as u64)
}

fn fft(re: &mut [f32], im: &mut [f32], tw: &[(f32, f32)]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let step = n / len;
        for start in (0..n).step_by(len) {
            for k in 0..len / 2 {
                let (c, s) = tw[k * step];
                let (a, b) = (start + k, start + k + len / 2);
                let (xr, xi) = (re[b] * c - im[b] * s, re[b] * s + im[b] * c);
                re[b] = re[a] - xr;
                im[b] = im[a] - xi;
                re[a] += xr;
                im[a] += xi;
            }
        }
        len <<= 1;
    }
}

pub fn audio_print(x: &[f32]) -> Vec<u32> {
    if x.len() < AUDIO_RATE {
        return Vec::new();
    }
    let frames = (x.len() - WIN) / HOP;
    let tau = std::f32::consts::TAU;
    let hann: Vec<f32> = (0..WIN).map(|i| 0.5 - 0.5 * (tau * i as f32 / (WIN - 1) as f32).cos()).collect();
    let tw: Vec<(f32, f32)> = (0..WIN / 2).map(|k| ((-tau * k as f32 / WIN as f32).cos(), (-tau * k as f32 / WIN as f32).sin())).collect();
    let bins: Vec<usize> = (0..34)
        .map(|i| {
            let edge = 300.0f64 * (2000.0f64 / 300.0).powf(i as f64 / 33.0);
            (edge / (AUDIO_RATE as f64 / 2.0) * (WIN / 2) as f64) as usize
        })
        .collect();
    let (mut re, mut im) = (vec![0f32; WIN], vec![0f32; WIN]);
    let mut prev = [0f32; 33];
    let mut out = Vec::with_capacity(frames.saturating_sub(1));
    for f in 0..frames {
        let base = f * HOP;
        for i in 0..WIN {
            re[i] = x[base + i] * hann[i];
            im[i] = 0.0;
        }
        fft(&mut re, &mut im, &tw);
        let mut e = [0f32; 33];
        for (b, slot) in e.iter_mut().enumerate() {
            *slot = (bins[b]..bins[b + 1]).map(|k| re[k] * re[k] + im[k] * im[k]).sum();
        }
        if f > 0 {
            let mut v = 0u32;
            for i in 0..32 {
                if (e[i] - e[i + 1]) - (prev[i] - prev[i + 1]) > 0.0 {
                    v |= 1 << i;
                }
            }
            out.push(v);
        }
        prev = e;
    }
    out
}

pub struct Resampler {
    ratio: f64,
    seen: u64,
    made: u64,
    acc: f32,
    count: u32,
    pub out: Vec<f32>,
}

impl Resampler {
    pub fn new(rate: u32) -> Self {
        Resampler { ratio: rate as f64 / AUDIO_RATE as f64, seen: 0, made: 0, acc: 0.0, count: 0, out: Vec::new() }
    }

    pub fn push(&mut self, s: f32) {
        self.acc += s;
        self.count += 1;
        self.seen += 1;
        if self.seen as f64 >= (self.made + 1) as f64 * self.ratio {
            self.out.push(self.acc / self.count as f32);
            self.made += 1;
            self.acc = 0.0;
            self.count = 0;
        }
    }
}

pub fn print(path: &Path) -> Option<Print> {
    #[cfg(windows)]
    let d = mf::decode(path).ok()?;
    #[cfg(not(windows))]
    let d: Decoded = return None;
    if d.grays.is_empty() {
        return None;
    }
    let size = std::fs::metadata(path).map(|m| m.len() as i64).unwrap_or(0);
    let b = bbox(&d.grays);
    Some(Print {
        duration_ms: d.duration_ms,
        width: d.width,
        height: d.height,
        rotation: d.rotation,
        bitrate: if d.duration_ms > 0 { size * 8 * 1000 / d.duration_ms } else { 0 },
        bbox: b,
        frames: d.grays.iter().map(|g| phash(g, b)).collect(),
        audio: audio_print(&d.mono),
        stills: d.stills.iter().map(|s| crop_rgb(s, b)).collect(),
    })
}

#[cfg(windows)]
pub mod mf {
    use super::*;
    use windows::core::{Interface, HSTRING};
    use windows::Win32::Foundation::HMODULE;
    use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
    use windows::Win32::Graphics::Direct3D11::*;
    use windows::Win32::Media::MediaFoundation::*;

    const VIDEO: u32 = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
    const AUDIO: u32 = MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32;
    const EOS: u32 = MF_SOURCE_READERF_ENDOFSTREAM.0 as u32;

    struct Shared(Option<ID3D11Device>);
    unsafe impl Send for Shared {}
    unsafe impl Sync for Shared {}
    static DEVICE: std::sync::OnceLock<Shared> = std::sync::OnceLock::new();

    pub fn hardware(attrs: &IMFAttributes) -> windows::core::Result<bool> {
        if let Some(manager) = DEVICE.get_or_init(|| Shared(device().ok())).0.as_ref().and_then(|d| manager(d).ok()) {
            unsafe {
                attrs.SetUnknown(&MF_SOURCE_READER_D3D_MANAGER, &manager)?;
                attrs.SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1)?;
            }
            return Ok(true);
        }
        Ok(false)
    }

    fn device() -> windows::core::Result<ID3D11Device> {
        unsafe {
            let mut device = None;
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_VIDEO_SUPPORT | D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                None,
            )?;
            let device = device.unwrap();
            let _ = device.cast::<ID3D11Multithread>()?.SetMultithreadProtected(true);
            Ok(device)
        }
    }

    fn manager(device: &ID3D11Device) -> windows::core::Result<IMFDXGIDeviceManager> {
        unsafe {
            let (mut token, mut manager) = (0u32, None);
            MFCreateDXGIDeviceManager(&mut token, &mut manager)?;
            let manager = manager.unwrap();
            manager.ResetDevice(device, token)?;
            Ok(manager)
        }
    }

    pub fn frame(sample: &IMFSample, w: u32, h: u32) -> windows::core::Result<RgbImage> {
        unsafe {
            let buffer = sample.ConvertToContiguousBuffer()?;
            let mut img = RgbImage::new(w, h);
            let row = w as usize * 4;
            let mut fill = |data: *const u8, pitch: isize| {
                for y in 0..h as usize {
                    let line = std::slice::from_raw_parts(data.offset(y as isize * pitch), row);
                    for (x, p) in line.chunks_exact(4).enumerate() {
                        img.put_pixel(x as u32, y as u32, image::Rgb([p[2], p[1], p[0]]));
                    }
                }
            };
            if let Ok(b2) = buffer.cast::<IMF2DBuffer>() {
                let (mut ptr, mut pitch) = (std::ptr::null_mut(), 0i32);
                b2.Lock2D(&mut ptr, &mut pitch)?;
                fill(ptr, pitch as isize);
                b2.Unlock2D()?;
            } else {
                let mut ptr = std::ptr::null_mut();
                buffer.Lock(&mut ptr, None, None)?;
                fill(ptr, row as isize);
                buffer.Unlock()?;
            }
            Ok(img)
        }
    }

    fn gray(img: &RgbImage) -> GrayImage {
        imageops::resize(&imageops::grayscale(img), GRID, GRID, imageops::FilterType::Triangle)
    }

    pub fn decode(path: &Path) -> windows::core::Result<Decoded> {
        std::thread::scope(|scope| {
            let sound = scope.spawn(|| {
                unsafe {
                    let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_MULTITHREADED);
                }
                audio(&HSTRING::from(path.as_os_str())).unwrap_or_default()
            });
            let mut d = video(path)?;
            d.mono = sound.join().unwrap_or_default();
            Ok(d)
        })
    }

    fn video(path: &Path) -> windows::core::Result<Decoded> {
        unsafe {
            let mut attrs = None;
            MFCreateAttributes(&mut attrs, 3)?;
            let attrs = attrs.unwrap();
            attrs.SetUINT32(&MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING, 1)?;
            hardware(&attrs)?;
            let url = HSTRING::from(path.as_os_str());
            let reader = MFCreateSourceReaderFromURL(&url, &attrs)?;
            reader.SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false)?;
            reader.SetStreamSelection(VIDEO, true)?;
            let native = reader.GetNativeMediaType(VIDEO, 0)?;
            let rotation = native.GetUINT32(&MF_MT_VIDEO_ROTATION).unwrap_or(0);
            let size = native.GetUINT64(&MF_MT_FRAME_SIZE)?;
            let (w, h) = ((size >> 32) as u32, (size & 0xFFFF_FFFF) as u32);
            let wanted = MFCreateMediaType()?;
            wanted.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
            wanted.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32)?;
            let scale = STILL as f32 / w.max(h).max(1) as f32;
            let (ow, oh) = (((w as f32 * scale).round() as u32).max(2) & !1, ((h as f32 * scale).round() as u32).max(2) & !1);
            let turned = rotation == 90 || rotation == 270;
            let (ow, oh) = if turned { (oh, ow) } else { (ow, oh) };
            wanted.SetUINT64(&MF_MT_FRAME_SIZE, (ow as u64) << 32 | oh as u64)?;
            reader.SetCurrentMediaType(VIDEO, None, &wanted)?;
            let duration = reader
                .GetPresentationAttribute(MF_SOURCE_READER_MEDIASOURCE.0 as u32, &MF_PD_DURATION)
                .ok()
                .and_then(|v| u64::try_from(&v).ok())
                .unwrap_or(0) as i64;

            let step = TICKS / FPS;
            let still_step = (duration / MAX_STILLS).max(TICKS);
            let (mut next, mut next_still) = (0i64, 0i64);
            let (mut grays, mut stills) = (Vec::new(), Vec::new());
            let mut empty = 0;
            let mut start = None;
            loop {
                let (mut flags, mut ts, mut sample) = (0u32, 0i64, None);
                reader.ReadSample(VIDEO, 0, None, Some(&mut flags), Some(&mut ts), Some(&mut sample))?;
                if flags & EOS != 0 {
                    break;
                }
                let Some(sample) = sample else {
                    empty += 1;
                    if empty > 60 {
                        break;
                    }
                    continue;
                };
                empty = 0;
                let ts = ts - *start.get_or_insert(ts);
                if ts < next && ts < next_still {
                    continue;
                }
                let img = frame(&sample, ow, oh)?;
                if ts >= next {
                    let g = gray(&img);
                    while ts >= next {
                        grays.push(g.clone());
                        next += step;
                    }
                }
                if ts >= next_still {
                    stills.push(img);
                    while ts >= next_still {
                        next_still += still_step;
                    }
                }
            }
            let (width, height) = if rotation == 90 || rotation == 270 { (h, w) } else { (w, h) };
            let duration_ms = if duration > 0 { duration / 10_000 } else { grays.len() as i64 * 1000 / FPS };
            Ok(Decoded { duration_ms, width, height, rotation, grays, stills, mono: Vec::new() })
        }
    }

    fn audio(url: &HSTRING) -> windows::core::Result<Vec<f32>> {
        unsafe {
            let reader = MFCreateSourceReaderFromURL(url, None)?;
            reader.SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false)?;
            reader.SetStreamSelection(AUDIO, true)?;
            let wanted = MFCreateMediaType()?;
            wanted.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
            wanted.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_Float)?;
            reader.SetCurrentMediaType(AUDIO, None, &wanted)?;
            let current = reader.GetCurrentMediaType(AUDIO)?;
            let channels = current.GetUINT32(&MF_MT_AUDIO_NUM_CHANNELS)?.max(1) as usize;
            let rate = current.GetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND)?;
            let mut rs = Resampler::new(rate);
            loop {
                let (mut flags, mut sample) = (0u32, None);
                reader.ReadSample(AUDIO, 0, None, Some(&mut flags), None, Some(&mut sample))?;
                if flags & EOS != 0 {
                    break;
                }
                let Some(sample) = sample else { continue };
                let buffer = sample.ConvertToContiguousBuffer()?;
                let mut ptr = std::ptr::null_mut();
                let mut len = 0u32;
                buffer.Lock(&mut ptr, None, Some(&mut len))?;
                let data = std::slice::from_raw_parts(ptr as *const f32, len as usize / 4);
                for frame in data.chunks_exact(channels) {
                    rs.push(frame.iter().sum::<f32>() / channels as f32);
                }
                buffer.Unlock()?;
            }
            Ok(rs.out)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn border_box_cuts_black_bars() {
        let g = GrayImage::from_fn(GRID, GRID, |_, y| image::Luma([if (16..112).contains(&y) { 200 } else { 0 }]));
        assert_eq!(bbox(&[g]), [16.0 / 128.0, 112.0 / 128.0, 0.0, 1.0]);
    }

    #[test]
    fn phash_survives_brightness_and_bars() {
        let img = |dy: u32, gain: f32| {
            GrayImage::from_fn(GRID, GRID, |x, y| {
                if y < dy || y >= GRID - dy {
                    return image::Luma([0]);
                }
                let yy = (y - dy) as f32 * 128.0 / (GRID - 2 * dy) as f32;
                image::Luma([((((x as f32 / 9.0).sin() * (yy / 13.0).cos() + ((x as f32 + yy) / 17.0).sin()) * 50.0 + 120.0) * gain) as u8])
            })
        };
        let a = img(0, 1.0);
        let b = img(20, 0.8);
        let ha = phash(&a, bbox(&[a.clone()]));
        let hb = phash(&b, bbox(&[b.clone()]));
        assert!((ha ^ hb).count_ones() <= 6, "{}", (ha ^ hb).count_ones());
    }

    #[test]
    fn audio_print_matches_shifted_tone_sequence() {
        let tone: Vec<f32> = (0..AUDIO_RATE * 6)
            .map(|i| {
                let t = i as f32 / AUDIO_RATE as f32;
                let f = 400.0 + 300.0 * ((t * 1.3).sin() + (t * 3.1).cos());
                (std::f32::consts::TAU * f * t).sin()
            })
            .collect();
        let a = audio_print(&tone);
        let b = audio_print(&tone[HOP * 40..]);
        let (score, off) = super::super::matcher::align(&a, &b, super::super::matcher::AUDIO_HAM);
        assert!(score > 0.9, "{score}");
        assert_eq!(off, 40);
    }

    #[test]
    fn resampler_keeps_length() {
        let mut rs = Resampler::new(48_000);
        (0..48_000).for_each(|i| rs.push(i as f32));
        assert!((rs.out.len() as i64 - AUDIO_RATE as i64).abs() <= 1);
    }
}

#[cfg(all(test, windows))]
mod bench {
    use super::*;
    use std::time::Instant;

    #[test]
    #[ignore]
    fn bench_prints() {
        unsafe {
            let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_MULTITHREADED);
            let _ = windows::Win32::Media::MediaFoundation::MFStartup(
                windows::Win32::Media::MediaFoundation::MF_VERSION,
                windows::Win32::Media::MediaFoundation::MFSTARTUP_FULL,
            );
        }
        let mut paths = vec![std::path::PathBuf::from(r"G:\sorter-test\variants\orig.mp4")];
        if let Ok(dir) = std::fs::read_dir(r"G:\sorter-test\deck") {
            paths.extend(dir.filter_map(|e| e.ok().map(|e| e.path())).take(std::env::var("BENCH_N").ok().and_then(|n| n.parse().ok()).unwrap_or(20)));
        }
        let (mut video_ms, mut spent) = (0i64, 0u128);
        for p in paths.iter().filter(|p| p.exists()) {
            let t = Instant::now();
            let Some(pr) = print(p) else {
                println!("FAIL {}", p.display());
                continue;
            };
            let ms = t.elapsed().as_millis();
            video_ms += pr.duration_ms;
            spent += ms;
            println!(
                "{} {}ms видео {}ms {}x{} rot {} кадров {} звук {} стоп {} box {:?}",
                p.file_name().unwrap().to_string_lossy(),
                ms,
                pr.duration_ms,
                pr.width,
                pr.height,
                pr.rotation,
                pr.frames.len(),
                pr.audio.len(),
                pr.stills.len(),
                pr.bbox
            );
        }
        println!("ИТОГО {spent} ms на {} с видео → {:.2} с на минуту", video_ms / 1000, spent as f64 / 1000.0 / (video_ms as f64 / 60_000.0));
    }
}

