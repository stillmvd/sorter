use std::fs;
use std::io;
use std::path::{Path, MAIN_SEPARATOR};
use std::sync::atomic::{AtomicBool, Ordering};

pub fn canon(p: &str) -> String {
    let trimmed = p.trim().trim_matches('"');
    let raw = match fs::canonicalize(trimmed) {
        Ok(c) => c.to_string_lossy().into_owned(),
        Err(_) => trimmed.replace('/', "\\"),
    };
    let s = if let Some(rest) = raw.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = raw.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        raw
    };
    let t = s.trim_end_matches(['\\', '/']);
    if t.len() == 2 && t.ends_with(':') { format!("{t}\\") } else { t.to_string() }
}

pub fn is_inside(child: &str, parent: &str) -> bool {
    let c = child.to_lowercase();
    let p = parent.to_lowercase();
    let p = p.trim_end_matches(MAIN_SEPARATOR);
    c.len() > p.len() && c.starts_with(p) && c[p.len()..].starts_with(MAIN_SEPARATOR)
}

pub fn same(a: &str, b: &str) -> bool {
    a.to_lowercase().trim_end_matches(MAIN_SEPARATOR) == b.to_lowercase().trim_end_matches(MAIN_SEPARATOR)
}

pub fn name(p: &str) -> String {
    Path::new(p).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| p.to_string())
}

const SKIP_NAMES: [&str; 2] = ["$recycle.bin", "system volume information"];

pub fn walk(root: &str, skip: &[String], stop: &AtomicBool, seen: &mut dyn FnMut(usize)) -> io::Result<Vec<(String, i64, i64)>> {
    let base = Path::new(root);
    fs::read_dir(base)?;
    let mut out = Vec::new();
    let mut stack = vec![base.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if stop.load(Ordering::Relaxed) {
            return Err(io::Error::new(io::ErrorKind::Interrupted, "stopped"));
        }
        let Ok(entries) = fs::read_dir(&dir) else { continue };
        for e in entries.filter_map(|e| e.ok()) {
            let Ok(ft) = e.file_type() else { continue };
            let path = e.path();
            if ft.is_symlink() {
                continue;
            }
            if ft.is_dir() {
                let lower = e.file_name().to_string_lossy().to_lowercase();
                let full = path.to_string_lossy();
                if SKIP_NAMES.contains(&lower.as_str()) || skip.iter().any(|s| same(s, &full) || is_inside(&full, s)) {
                    continue;
                }
                stack.push(path);
                continue;
            }
            if !crate::deck::is_media(&path) {
                continue;
            }
            let Ok(meta) = e.metadata() else { continue };
            let Ok(rel) = path.strip_prefix(base) else { continue };
            out.push((rel.to_string_lossy().into_owned(), meta.len() as i64, crate::deck::mtime_ms(&meta)));
            if out.len() % 200 == 0 {
                seen(out.len());
            }
        }
    }
    seen(out.len());
    Ok(out)
}

pub fn parent_dirs(rel: &str) -> Option<&str> {
    rel.rfind(MAIN_SEPARATOR).map(|i| &rel[..i])
}

pub fn origin(root: &str, rel: &str) -> String {
    let mut parts = vec![name(root)];
    if let Some(dirs) = parent_dirs(rel) {
        parts.extend(dirs.split(MAIN_SEPARATOR).map(str::to_string));
    }
    parts.join(" › ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canon_and_inside() {
        let dir = tempfile::tempdir().unwrap();
        let root = canon(&dir.path().to_string_lossy());
        assert!(!root.starts_with(r"\\?\"));
        assert_eq!(canon(&format!("{}\\", dir.path().to_string_lossy().to_uppercase())), root);
        assert!(is_inside(&format!("{root}\\a\\b"), &root));
        assert!(!is_inside(&format!("{root}x\\a"), &root));
        assert!(!is_inside(&root, &root));
        assert_eq!(canon("G:/no/such/"), r"G:\no\such");
    }

    #[test]
    fn walk_goes_deep_and_skips_table() {
        let dir = tempfile::tempdir().unwrap();
        let root = canon(&dir.path().to_string_lossy());
        for (sub, n) in [("", 2), ("a", 3), ("a\\b", 1), ("stol\\x", 4)] {
            let d = Path::new(&root).join(sub);
            fs::create_dir_all(&d).unwrap();
            for i in 0..n {
                fs::write(d.join(format!("{i}.mp4")), b"v").unwrap();
            }
        }
        fs::write(Path::new(&root).join("note.txt"), b"t").unwrap();
        let stop = AtomicBool::new(false);
        let skip = vec![format!("{root}\\STOL")];
        let mut files = walk(&root, &skip, &stop, &mut |_| {}).unwrap();
        files.sort();
        assert_eq!(files.len(), 6);
        assert!(files.iter().any(|f| f.0 == "a\\b\\0.mp4"));
        assert_eq!(origin(&root, "a\\b\\0.mp4"), format!("{} › a › b", name(&root)));
        assert_eq!(origin(&root, "0.mp4"), name(&root));
        stop.store(true, Ordering::Relaxed);
        assert!(walk(&root, &[], &stop, &mut |_| {}).is_err());
    }
}
