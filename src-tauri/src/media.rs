use crate::commands::AppState;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use tauri::http::{header, Request, Response, StatusCode};
use tauri::{AppHandle, Manager};

const CHUNK: u64 = 4 * 1024 * 1024;

fn decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn mime(path: &Path) -> &'static str {
    match path.extension().map(|e| e.to_string_lossy().to_lowercase()).as_deref() {
        Some("mp4") | Some("m4v") => "video/mp4",
        Some("webm") => "video/webm",
        Some("mkv") => "video/x-matroska",
        Some("mov") => "video/quicktime",
        Some("avi") => "video/x-msvideo",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        _ => "application/octet-stream",
    }
}

fn inside(path: &Path, root: &Path) -> bool {
    let p = path.to_string_lossy().to_lowercase();
    let r = root.to_string_lossy().to_lowercase();
    let r = r.trim_end_matches('\\');
    !r.is_empty() && p.starts_with(&format!("{r}\\")) && !p.contains("..")
}

fn reply(status: StatusCode) -> Response<Vec<u8>> {
    Response::builder()
        .status(status)
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(header::CACHE_CONTROL, "no-store")
        .body(Vec::new())
        .unwrap()
}

pub fn serve(app: &AppHandle, request: &Request<Vec<u8>>) -> Response<Vec<u8>> {
    let path = PathBuf::from(decode(request.uri().path().trim_start_matches('/')));
    let state = app.state::<AppState>();
    if !state.roots().iter().any(|root| inside(&path, root)) {
        return reply(StatusCode::FORBIDDEN);
    }
    let Ok(mut file) = File::open(&path) else {
        return reply(StatusCode::NOT_FOUND);
    };
    let len = file.metadata().map(|m| m.len()).unwrap_or(0);
    let range = request
        .headers()
        .get(header::RANGE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("bytes="))
        .and_then(|v| {
            let (a, b) = v.split_once('-')?;
            let start: u64 = a.parse().ok()?;
            let end: u64 = if b.is_empty() { len.saturating_sub(1) } else { b.parse().ok()? };
            Some((start, end.min(len.saturating_sub(1))))
        });
    let builder = Response::builder()
        .header(header::CONTENT_TYPE, mime(&path))
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(header::CACHE_CONTROL, "no-store");
    match range {
        Some((start, end)) if start < len => {
            let end = end.min(start + CHUNK - 1);
            let mut buf = vec![0; (end - start + 1) as usize];
            if file.seek(SeekFrom::Start(start)).and_then(|_| file.read_exact(&mut buf)).is_err() {
                return reply(StatusCode::INTERNAL_SERVER_ERROR);
            }
            builder
                .status(StatusCode::PARTIAL_CONTENT)
                .header(header::CONTENT_RANGE, format!("bytes {start}-{end}/{len}"))
                .body(buf)
                .unwrap()
        }
        Some(_) => reply(StatusCode::RANGE_NOT_SATISFIABLE),
        None if len > CHUNK * 4 && !mime(&path).starts_with("image/") => builder
            .status(StatusCode::PARTIAL_CONTENT)
            .header(header::CONTENT_RANGE, format!("bytes 0-{}/{len}", CHUNK - 1))
            .body({
                let mut buf = vec![0; CHUNK as usize];
                let _ = file.read_exact(&mut buf);
                buf
            })
            .unwrap(),
        None => {
            let mut buf = Vec::with_capacity(len as usize);
            let _ = file.read_to_end(&mut buf);
            builder.status(StatusCode::OK).body(buf).unwrap()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_paths() {
        assert_eq!(decode("G%3A%5Cvk%20videos%5C%D0%B0.mp4"), "G:\\vk videos\\а.mp4");
        assert!(inside(Path::new("G:\\vk videos\\a.mp4"), Path::new("g:\\VK videos")));
        assert!(!inside(Path::new("G:\\vk videos\\..\\x"), Path::new("G:\\vk videos")));
        assert_eq!(mime(Path::new("a.PNG")), "image/png");
        assert_eq!(mime(Path::new("a.webp")), "image/webp");
    }
}
