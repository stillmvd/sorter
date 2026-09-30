use crate::dupes::print::{phash, GRID};
use image::{imageops, DynamicImage, GrayImage, ImageDecoder, ImageReader, RgbImage};
use std::path::Path;

pub const THUMB: u32 = 320;

pub struct PhotoMeta {
    pub width: u32,
    pub height: u32,
    pub taken: Option<i64>,
    pub camera: Option<String>,
    pub thumb: RgbImage,
    pub phash: u64,
    pub sharp: f32,
    pub look: Vec<u8>,
}

pub const LOOK: u32 = 16;

pub fn look_corr(a: &[u8], b: &[u8]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let norm = |v: &[u8]| {
        let n = v.len() as f32;
        let mean = v.iter().map(|x| *x as f32).sum::<f32>() / n;
        let sd = (v.iter().map(|x| (*x as f32 - mean).powi(2)).sum::<f32>() / n).sqrt().max(1e-3);
        v.iter().map(|x| (*x as f32 - mean) / sd).collect::<Vec<f32>>()
    };
    let (a, b) = (norm(a), norm(b));
    a.iter().zip(&b).map(|(x, y)| x * y).sum::<f32>() / a.len() as f32
}

pub fn read(path: &Path) -> Result<PhotoMeta, String> {
    let mut decoder = ImageReader::open(path)
        .map_err(|e| e.to_string())?
        .with_guessed_format()
        .map_err(|e| e.to_string())?
        .into_decoder()
        .map_err(|e| e.to_string())?;
    let raw = decoder.exif_metadata().ok().flatten();
    let orientation = decoder.orientation().unwrap_or(image::metadata::Orientation::NoTransforms);
    let mut img = DynamicImage::from_decoder(decoder).map_err(|e| e.to_string())?;
    img.apply_orientation(orientation);
    let (taken, camera) = raw.as_deref().map(exif_fields).unwrap_or((None, None));
    let thumb = img.thumbnail(THUMB, THUMB).to_rgb8();
    let gray = imageops::grayscale(&thumb);
    let small = imageops::resize(&gray, GRID, GRID, imageops::FilterType::Triangle);
    Ok(PhotoMeta {
        width: img.width(),
        height: img.height(),
        taken,
        camera,
        phash: phash(&small, [0.0, 1.0, 0.0, 1.0]),
        sharp: sharpness(&gray),
        look: imageops::resize(&gray, LOOK, LOOK, imageops::FilterType::Triangle).into_raw(),
        thumb,
    })
}

pub fn sharpness(g: &GrayImage) -> f32 {
    let (w, h) = g.dimensions();
    if w < 3 || h < 3 {
        return 0.0;
    }
    let px = |x: u32, y: u32| g.get_pixel(x, y)[0] as f32;
    let mut values = Vec::with_capacity(((w - 2) * (h - 2)) as usize);
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            values.push(px(x - 1, y) + px(x + 1, y) + px(x, y - 1) + px(x, y + 1) - 4.0 * px(x, y));
        }
    }
    let mean = values.iter().sum::<f32>() / values.len() as f32;
    values.iter().map(|v| (v - mean) * (v - mean)).sum::<f32>() / values.len() as f32
}

struct Tiff<'a> {
    data: &'a [u8],
    le: bool,
}

impl Tiff<'_> {
    fn u16(&self, at: usize) -> Option<u16> {
        let b: [u8; 2] = self.data.get(at..at + 2)?.try_into().ok()?;
        Some(if self.le { u16::from_le_bytes(b) } else { u16::from_be_bytes(b) })
    }

    fn u32(&self, at: usize) -> Option<u32> {
        let b: [u8; 4] = self.data.get(at..at + 4)?.try_into().ok()?;
        Some(if self.le { u32::from_le_bytes(b) } else { u32::from_be_bytes(b) })
    }

    fn entries(&self, at: usize) -> Vec<(u16, usize)> {
        let Some(n) = self.u16(at) else { return Vec::new() };
        (0..n as usize).map(|i| at + 2 + i * 12).filter_map(|e| Some((self.u16(e)?, e))).collect()
    }

    fn text(&self, entry: usize) -> Option<String> {
        if self.u16(entry + 2)? != 2 {
            return None;
        }
        let count = self.u32(entry + 4)? as usize;
        let at = if count <= 4 { entry + 8 } else { self.u32(entry + 8)? as usize };
        let bytes = self.data.get(at..at + count)?;
        let s = String::from_utf8_lossy(bytes).trim_matches(|c: char| c == '\0' || c.is_whitespace()).to_string();
        (!s.is_empty()).then_some(s)
    }
}

pub fn exif_fields(raw: &[u8]) -> (Option<i64>, Option<String>) {
    let data = raw.strip_prefix(b"Exif\0\0").unwrap_or(raw);
    let le = match data.get(0..2) {
        Some(b"II") => true,
        Some(b"MM") => false,
        _ => return (None, None),
    };
    let t = Tiff { data, le };
    let Some(ifd0) = t.u32(4) else { return (None, None) };
    let (mut make, mut model, mut date, mut original) = (None, None, None, None);
    for (tag, e) in t.entries(ifd0 as usize) {
        match tag {
            0x010F => make = t.text(e),
            0x0110 => model = t.text(e),
            0x0132 => date = t.text(e),
            0x8769 => {
                if let Some(sub) = t.u32(e + 8) {
                    original = t.entries(sub as usize).into_iter().find(|(tag, _)| *tag == 0x9003).and_then(|(_, e)| t.text(e));
                }
            }
            _ => {}
        }
    }
    let camera = match (make, model) {
        (Some(make), Some(model)) if model.to_lowercase().starts_with(&make.to_lowercase()) => Some(model),
        (Some(make), Some(model)) => Some(format!("{make} {model}")),
        (make, model) => model.or(make),
    };
    (original.or(date).and_then(|d| parse_date(&d)), camera)
}

fn parse_date(s: &str) -> Option<i64> {
    let n = |a: usize, b: usize| s.get(a..b)?.parse::<i64>().ok();
    let (y, mo, d, h, mi, sec) = (n(0, 4)?, n(5, 7)?, n(8, 10)?, n(11, 13)?, n(14, 16)?, n(17, 19)?);
    if !(1990..2100).contains(&y) || !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || sec > 60 {
        return None;
    }
    Some(((crate::deck::days_from_civil(y, mo, d) * 24 + h) * 60 + mi) * 60_000 + sec * 1000)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(le: bool, tag: u16, typ: u16, count: u32, value: u32) -> Vec<u8> {
        let mut v = Vec::new();
        let w16 = |x: u16| if le { x.to_le_bytes() } else { x.to_be_bytes() };
        let w32 = |x: u32| if le { x.to_le_bytes() } else { x.to_be_bytes() };
        v.extend(w16(tag));
        v.extend(w16(typ));
        v.extend(w32(count));
        v.extend(w32(value));
        v
    }

    fn build(le: bool, with_exif: bool) -> Vec<u8> {
        let w16 = |x: u16| if le { x.to_le_bytes() } else { x.to_be_bytes() };
        let w32 = |x: u32| if le { x.to_le_bytes() } else { x.to_be_bytes() };
        let make = b"Xiaomi\0";
        let model = b"Xiaomi Redmi Note 5\0";
        let date = b"2018:06:01 18:59:02\0";
        let original = b"2018:06:01 18:59:07\0";
        let n = if with_exif { 4 } else { 3 };
        let ifd0 = 8u32;
        let strings = ifd0 + 2 + 12 * n + 4;
        let (o_make, o_model) = (strings, strings + make.len() as u32);
        let o_date = o_model + model.len() as u32;
        let o_sub = o_date + date.len() as u32;
        let o_orig = o_sub + 2 + 12 + 4;
        let mut v = Vec::new();
        v.extend(if le { *b"II" } else { *b"MM" });
        v.extend(w16(42));
        v.extend(w32(ifd0));
        v.extend(w16(n as u16));
        v.extend(entry(le, 0x010F, 2, make.len() as u32, o_make));
        v.extend(entry(le, 0x0110, 2, model.len() as u32, o_model));
        v.extend(entry(le, 0x0132, 2, date.len() as u32, o_date));
        if with_exif {
            v.extend(entry(le, 0x8769, 4, 1, o_sub));
        }
        v.extend(w32(0));
        v.extend(make);
        v.extend(model);
        v.extend(date);
        if with_exif {
            v.extend(w16(1));
            v.extend(entry(le, 0x9003, 2, original.len() as u32, o_orig));
            v.extend(w32(0));
            v.extend(original);
        }
        v
    }

    #[test]
    fn reads_date_and_camera_in_both_byte_orders() {
        let taken = parse_date("2018:06:01 18:59:07");
        for le in [true, false] {
            assert_eq!(exif_fields(&build(le, true)), (taken, Some("Xiaomi Redmi Note 5".into())));
            let mut prefixed = b"Exif\0\0".to_vec();
            prefixed.extend(build(le, true));
            assert_eq!(exif_fields(&prefixed).0, taken);
        }
        assert_eq!(exif_fields(&build(true, false)).0, parse_date("2018:06:01 18:59:02"));
        assert_eq!(parse_date("2018:06:01 18:59:07").unwrap() - parse_date("2018:06:01 18:59:02").unwrap(), 5000);
    }

    #[test]
    fn garbage_is_empty() {
        assert_eq!(exif_fields(b"nonsense"), (None, None));
        assert_eq!(exif_fields(&build(true, true)[..20]), (None, None));
        assert_eq!(parse_date("0000:00:00 00:00:00"), None);
    }

    #[test]
    fn sharp_beats_blurred() {
        let sharp = GrayImage::from_fn(64, 64, |x, y| image::Luma([if (x / 4 + y / 4) % 2 == 0 { 255 } else { 0 }]));
        let blurred = imageops::blur(&sharp, 3.0);
        assert!(sharpness(&sharp) > sharpness(&blurred) * 4.0);
    }

    #[test]
    #[ignore]
    fn reads_test_set() {
        let dir = Path::new(r"G:\sorter-test\photos");
        if !dir.exists() {
            return;
        }
        let rotated = read(&dir.join("rotated_exif6.jpg")).unwrap();
        assert!(rotated.height > rotated.width);
        assert!(read(&dir.join("IMG_series1_1.jpg")).unwrap().taken.is_some());
        assert_eq!(read(&dir.join("IMG_series1_1.jpg")).unwrap().camera.as_deref(), Some("Xiaomi Redmi Note 5"));
        assert!(read(&dir.join("no_exif.jpg")).unwrap().taken.is_none());
        for name in ["transparent.png", "photo.webp", "animated.gif", "still.gif"] {
            let m = read(&dir.join(name)).unwrap();
            assert!(m.thumb.width() > 0, "{name}");
        }
        assert!(read(&dir.join("broken.jpg")).is_err());
        let a = read(&dir.join("IMG_series1_1.jpg")).unwrap();
        let b = read(&dir.join("IMG_series1_2.jpg")).unwrap();
        let c = read(&dir.join("IMG_single_1.jpg")).unwrap();
        println!("серия {} чужой {}", (a.phash ^ b.phash).count_ones(), (a.phash ^ c.phash).count_ones());
        let t = std::time::Instant::now();
        let _ = read(&dir.join("dupe_orig.jpg")).unwrap();
        println!("проявка {:?}", t.elapsed());
    }
}
