use super::{deck_files, walk, FileInfo};
use crate::db;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::Path;

const KEY: &str = "dupe_search";

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct SearchSpec {
    pub piles: Vec<String>,
    pub deck: bool,
    pub folders: Vec<String>,
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub skipped: Vec<String>,
}

impl SearchSpec {
    pub fn active(&self) -> bool {
        self.state == "running" || self.state == "done"
    }

    pub fn is_empty(&self) -> bool {
        self.piles.is_empty() && !self.deck && self.folders.is_empty()
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Root {
    pub path: String,
    pub files: usize,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub total: usize,
    pub roots: Vec<Root>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub piles: Vec<String>,
    pub deck: bool,
    pub folders: Vec<String>,
    pub skipped: Vec<String>,
    pub state: String,
    pub files: usize,
}

pub fn load(conn: &Connection) -> rusqlite::Result<Option<SearchSpec>> {
    Ok(db::get_setting(conn, KEY)?.and_then(|s| serde_json::from_str(&s).ok()))
}

pub fn save(conn: &Connection, spec: &SearchSpec) -> rusqlite::Result<()> {
    db::set_setting(conn, KEY, &serde_json::to_string(spec).unwrap_or_default())
}

pub fn clear(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM settings WHERE key = ?1", [KEY]).map(|_| ())
}

fn unreachable(dir: &Path) -> Option<String> {
    if !dir.exists() {
        return Some("Папка не найдена — возможно, диск отключён.".into());
    }
    fs::read_dir(dir).err().map(|_| "Нет доступа к папке.".into())
}

pub fn collect(conn: &Connection, spec: &SearchSpec) -> rusqlite::Result<(Vec<FileInfo>, Vec<Root>)> {
    let table = db::get_setting(conn, "table_path")?.unwrap_or_default();
    let mut out = Vec::new();
    if spec.deck {
        out.extend(deck_files(conn)?);
    }
    if !table.is_empty() {
        for name in &spec.piles {
            walk(&Path::new(&table).join(name), &mut out);
        }
    }
    let mut roots = Vec::new();
    for folder in &spec.folders {
        let dir = Path::new(folder);
        if let Some(error) = unreachable(dir) {
            roots.push(Root { path: folder.clone(), files: 0, error: Some(error) });
            continue;
        }
        let mut found = Vec::new();
        walk(dir, &mut found);
        roots.push(Root { path: folder.clone(), files: found.len(), error: None });
        out.extend(found);
    }
    let mut seen = HashSet::new();
    out.retain(|f| seen.insert(f.path.to_lowercase()));
    Ok((out, roots))
}

pub fn files(conn: &Connection, spec: &SearchSpec) -> rusqlite::Result<Vec<FileInfo>> {
    Ok(collect(conn, spec)?.0)
}

pub fn preview(conn: &Connection, spec: &SearchSpec) -> rusqlite::Result<Preview> {
    let (files, roots) = collect(conn, spec)?;
    Ok(Preview { total: files.len(), roots })
}

pub fn start(conn: &Connection, mut spec: SearchSpec) -> rusqlite::Result<SearchSpec> {
    let (_, roots) = collect(conn, &spec)?;
    spec.skipped = roots.into_iter().filter(|r| r.error.is_some()).map(|r| r.path).collect();
    spec.state = "running".into();
    save(conn, &spec)?;
    Ok(spec)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dupes::Scope;
    use rusqlite::params;

    fn setup() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = Connection::open_in_memory().unwrap();
        db::init(&conn).unwrap();
        let table = dir.path().join("table");
        fs::create_dir_all(table.join("Мемы").join("2019")).unwrap();
        fs::create_dir_all(table.join("Друзья")).unwrap();
        fs::write(table.join("Мемы").join("a.mp4"), b"a").unwrap();
        fs::write(table.join("Мемы").join("2019").join("b.mp4"), b"b").unwrap();
        fs::write(table.join("Друзья").join("c.mp4"), b"c").unwrap();
        db::set_setting(&conn, "table_path", &table.to_string_lossy()).unwrap();
        (dir, conn)
    }

    #[test]
    fn nested_folders_count_once_and_hidden_skipped() {
        let (dir, conn) = setup();
        let ext = dir.path().join("ext");
        fs::create_dir_all(ext.join("inner")).unwrap();
        fs::create_dir_all(ext.join(".cache")).unwrap();
        fs::write(ext.join("x.mp4"), b"x").unwrap();
        fs::write(ext.join("inner").join("y.jpg"), b"y").unwrap();
        fs::write(ext.join(".cache").join("z.mp4"), b"z").unwrap();
        fs::write(ext.join("notes.txt"), b"n").unwrap();
        let mems = dir.path().join("table").join("Мемы");
        let spec = SearchSpec {
            piles: vec!["Мемы".into()],
            deck: false,
            folders: vec![ext.to_string_lossy().into(), ext.join("inner").to_string_lossy().into(), mems.to_string_lossy().into()],
            ..Default::default()
        };
        let p = preview(&conn, &spec).unwrap();
        assert_eq!(p.total, 4);
        assert_eq!(p.roots.iter().map(|r| r.files).collect::<Vec<_>>(), vec![2, 1, 2]);
    }

    #[test]
    fn missing_folder_is_skipped_and_others_searched() {
        let (dir, conn) = setup();
        let gone = dir.path().join("нет");
        let spec = SearchSpec { piles: vec!["Друзья".into()], folders: vec![gone.to_string_lossy().into()], ..Default::default() };
        let p = preview(&conn, &spec).unwrap();
        assert_eq!(p.total, 1);
        assert!(p.roots[0].error.is_some());
        let started = start(&conn, spec).unwrap();
        assert_eq!(started.skipped, vec![gone.to_string_lossy().into_owned()]);
        assert!(matches!(Scope::current(&conn).unwrap(), Scope::Search(_)));
        assert!(matches!(Scope::work(&conn).unwrap(), Scope::Search(_)));
        save(&conn, &SearchSpec { state: "done".into(), ..started.clone() }).unwrap();
        assert!(matches!(Scope::current(&conn).unwrap(), Scope::Search(_)));
        assert_eq!(Scope::work(&conn).unwrap(), Scope::Off);
        clear(&conn).unwrap();
        assert_eq!(Scope::current(&conn).unwrap(), Scope::Off);
    }

    #[test]
    fn deck_joins_only_when_asked() {
        let (dir, conn) = setup();
        let deck = dir.path().join("deck");
        fs::create_dir_all(&deck).unwrap();
        fs::write(deck.join("d.mp4"), b"d").unwrap();
        db::set_setting(&conn, "deck_path", &deck.to_string_lossy()).unwrap();
        conn.execute("INSERT INTO card(deck_path, file_name, size, mtime, position) VALUES (?1, 'd.mp4', 1, 0, 0)", params![deck.to_string_lossy()]).unwrap();
        let spec = SearchSpec { piles: vec!["Друзья".into()], ..Default::default() };
        assert_eq!(files(&conn, &spec).unwrap().len(), 1);
        assert_eq!(files(&conn, &SearchSpec { deck: true, ..spec }).unwrap().len(), 2);
    }
}
