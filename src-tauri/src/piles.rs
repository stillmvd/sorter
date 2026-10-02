use crate::deck::is_media;
use crate::error::{io_error, AppError, AppResult};
use crate::moves::{safe_rename, validate_pile_name};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

pub const KEYS: [&str; 19] = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "Q", "W", "E", "R", "T", "Y", "U", "I", "O", "P"];
pub const TRASH_KEY: &str = "Delete";
const TRASH_NAME: &str = ":trash";
const TRASH_ORD: i64 = 1_000_000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PileView {
    pub id: i64,
    pub name: String,
    pub key: Option<String>,
    pub is_trash: bool,
    pub count: i64,
    pub examples: i64,
}

fn folders(table: &Path) -> AppResult<Vec<String>> {
    let mut names: Vec<String> = fs::read_dir(table)
        .map_err(|e| io_error(&e, table))?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| !n.starts_with('.') && !n.starts_with('$'))
        .collect();
    names.sort_by_key(|n| n.to_lowercase());
    Ok(names)
}

pub fn sync(conn: &Connection, table: &str) -> AppResult<()> {
    let on_disk = folders(Path::new(table))?;
    let lower: HashSet<String> = on_disk.iter().map(|n| n.to_lowercase()).collect();
    let rows: Vec<(i64, String)> = conn
        .prepare("SELECT id, name FROM pile WHERE table_path = ?1 AND is_trash = 0")?
        .query_map(params![table], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    let known: HashSet<String> = rows.iter().map(|(_, n)| n.to_lowercase()).collect();
    for (id, name) in &rows {
        conn.execute(
            "UPDATE pile SET exists_on_disk = ?2 WHERE id = ?1",
            params![id, lower.contains(&name.to_lowercase())],
        )?;
    }
    for name in on_disk.iter().filter(|n| !known.contains(&n.to_lowercase())) {
        let used: HashSet<String> = conn
            .prepare("SELECT key FROM pile WHERE table_path = ?1 AND key IS NOT NULL")?
            .query_map(params![table], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        let key = KEYS.iter().find(|k| !used.contains(**k)).copied();
        conn.execute(
            "INSERT INTO pile(table_path, name, key, ord) VALUES (?1, ?2, ?3,
               (SELECT COALESCE(MAX(ord), -1) + 1 FROM pile WHERE table_path = ?1 AND is_trash = 0))",
            params![table, name, key],
        )?;
    }
    conn.execute(
        "INSERT OR IGNORE INTO pile(table_path, name, key, is_trash, ord) VALUES (?1, ?2, ?3, 1, ?4)",
        params![table, TRASH_NAME, TRASH_KEY, TRASH_ORD],
    )?;
    Ok(())
}

fn count_media(dir: &Path) -> i64 {
    fs::read_dir(dir)
        .map(|it| it.filter_map(|e| e.ok()).filter(|e| is_media(&e.path())).count() as i64)
        .unwrap_or(0)
}

pub fn list(conn: &Connection, table: &str) -> AppResult<Vec<PileView>> {
    let rows: Vec<(i64, String, Option<String>, bool, i64)> = conn
        .prepare(
            "SELECT p.id, p.name, p.key, p.is_trash, (SELECT COUNT(*) FROM example e WHERE e.pile_id = p.id)
             FROM pile p WHERE p.table_path = ?1 AND (p.exists_on_disk = 1 OR p.is_trash = 1)
             ORDER BY p.is_trash, p.ord",
        )?
        .query_map(params![table], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?
        .collect::<Result<_, _>>()?;
    let mut out = Vec::with_capacity(rows.len());
    for (id, name, key, is_trash, examples) in rows {
        let count = if is_trash {
            conn.query_row(
                "SELECT COUNT(*) FROM move_item i JOIN move m ON m.id = i.move_id WHERE m.pile_id = ?1 AND m.state = 'done'",
                params![id],
                |r| r.get(0),
            )?
        } else {
            count_media(&Path::new(table).join(&name))
        };
        let name = if is_trash { "Корзина".to_string() } else { name };
        out.push(PileView { id, name, key, is_trash, count, examples });
    }
    Ok(out)
}

pub fn create(conn: &Connection, table: &str, name: &str) -> AppResult<i64> {
    let name = validate_pile_name(name)?;
    if name.to_lowercase() == "корзина" {
        return Err(AppError::new("BAD_NAME", "Имя «Корзина» занято встроенной стопкой — выбери другое."));
    }
    let dir = Path::new(table).join(&name);
    if dir.exists() {
        return Err(AppError::new("EXISTS", format!("Стопка «{name}» уже есть.")));
    }
    fs::create_dir(&dir).map_err(|e| io_error(&e, &dir))?;
    sync(conn, table)?;
    Ok(conn.query_row(
        "SELECT id FROM pile WHERE table_path = ?1 AND name = ?2 COLLATE NOCASE",
        params![table, name],
        |r| r.get(0),
    )?)
}

fn replace_prefix(column: &str, table: &str) -> String {
    format!(
        "UPDATE {table} SET {column} = ?2 || substr({column}, length(?1) + 1)
         WHERE {column} IS NOT NULL AND lower(substr({column}, 1, length(?1))) = lower(?1)"
    )
}

pub fn rename(conn: &Connection, pile_id: i64, name: &str) -> AppResult<()> {
    let name = validate_pile_name(name)?;
    let (table, old, is_trash): (String, String, bool) = conn
        .query_row("SELECT table_path, name, is_trash FROM pile WHERE id = ?1", params![pile_id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .optional()?
        .ok_or_else(|| AppError::new("PILE_GONE", "Этой стопки больше нет — её папку убрали со стола. Выбери другую стопку или создай её заново."))?;
    if is_trash {
        return Err(AppError::new("BAD_NAME", "Корзину переименовать нельзя."));
    }
    if old == name {
        return Ok(());
    }
    let from = Path::new(&table).join(&old);
    let to = Path::new(&table).join(&name);
    if !old.eq_ignore_ascii_case(&name) && to.exists() {
        return Err(AppError::new("EXISTS", format!("Стопка «{name}» уже есть.")));
    }
    safe_rename(&from, &to).map_err(|e| io_error(&e, &from))?;
    let old_prefix = format!("{}\\", from.to_string_lossy());
    let new_prefix = format!("{}\\", to.to_string_lossy());
    conn.execute("UPDATE pile SET name = ?2 WHERE id = ?1", params![pile_id, name])?;
    for (column, tbl) in [("to_path", "move_item"), ("current_path", "card"), ("path", "example")] {
        conn.execute(&replace_prefix(column, tbl), params![old_prefix, new_prefix])?;
    }
    Ok(())
}

pub fn remove(conn: &Connection, pile_id: i64) -> AppResult<()> {
    let (table, name, is_trash): (String, String, bool) = conn
        .query_row("SELECT table_path, name, is_trash FROM pile WHERE id = ?1", params![pile_id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .optional()?
        .ok_or_else(|| AppError::new("PILE_GONE", "Этой стопки больше нет — её папку убрали со стола. Выбери другую стопку или создай её заново."))?;
    if is_trash {
        return Err(AppError::new("BAD_NAME", "Корзину удалить нельзя."));
    }
    let dir = Path::new(&table).join(&name);
    let has_files = fs::read_dir(&dir).map(|mut it| it.next().is_some()).unwrap_or(false);
    if has_files {
        return Err(AppError::new(
            "PILE_NOT_EMPTY",
            format!("В стопке «{name}» есть файлы. Sorter удаляет только пустые стопки — сначала забери или переложи их."),
        ));
    }
    if dir.exists() {
        fs::remove_dir(&dir).map_err(|e| io_error(&e, &dir))?;
    }
    conn.execute("UPDATE pile SET exists_on_disk = 0, key = NULL WHERE id = ?1", params![pile_id])?;
    Ok(())
}

pub fn set_key(conn: &Connection, pile_id: i64, key: Option<&str>) -> AppResult<()> {
    let table: String = conn.query_row(
        "SELECT table_path FROM pile WHERE id = ?1 AND is_trash = 0",
        params![pile_id],
        |r| r.get(0),
    )?;
    if let Some(k) = key {
        if !KEYS.contains(&k) {
            return Err(AppError::new("BAD_KEY", "Эту клавишу нельзя назначить — подойдут 1–9 и Q–P."));
        }
        conn.execute(
            "UPDATE pile SET key = NULL WHERE table_path = ?1 AND key = ?2",
            params![table, k],
        )?;
    }
    conn.execute("UPDATE pile SET key = ?2 WHERE id = ?1", params![pile_id, key])?;
    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use tempfile::TempDir;

    fn setup(names: &[&str]) -> (TempDir, String, Connection) {
        let tmp = TempDir::new().unwrap();
        for n in names {
            fs::create_dir(tmp.path().join(n)).unwrap();
        }
        let conn = Connection::open_in_memory().unwrap();
        db::init(&conn).unwrap();
        let table = tmp.path().to_string_lossy().into_owned();
        sync(&conn, &table).unwrap();
        (tmp, table, conn)
    }

    #[test]
    fn keys_in_order_and_trash_last() {
        let (_t, table, conn) = setup(&["Музыка", "Мемы", "Авто"]);
        let piles = list(&conn, &table).unwrap();
        let v: Vec<(String, Option<String>)> = piles.iter().map(|p| (p.name.clone(), p.key.clone())).collect();
        assert_eq!(v[0], ("Авто".into(), Some("1".into())));
        assert_eq!(v[2], ("Музыка".into(), Some("3".into())));
        assert_eq!(v[3], ("Корзина".into(), Some("Delete".into())));
    }

    #[test]
    fn set_key_steals_from_other() {
        let (_t, table, conn) = setup(&["А", "Б"]);
        let piles = list(&conn, &table).unwrap();
        set_key(&conn, piles[1].id, Some("1")).unwrap();
        let piles = list(&conn, &table).unwrap();
        assert_eq!(piles[0].key, None);
        assert_eq!(piles[1].key.as_deref(), Some("1"));
    }

    #[test]
    fn rename_moves_folder_and_journal() {
        let (t, table, conn) = setup(&["Кино"]);
        let id = list(&conn, &table).unwrap()[0].id;
        let old = t.path().join("Кино").join("a.mp4");
        conn.execute("INSERT INTO move(at, method, pile_id, state) VALUES (0,'key',?1,'done')", params![id]).unwrap();
        conn.execute(
            "INSERT INTO card(deck_path, file_name, size, mtime, position, status) VALUES ('d','a.mp4',1,0,0,'placed')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO move_item(move_id, card_id, from_path, to_path, step) VALUES (1,1,'d\\a.mp4',?1,'done')",
            params![old.to_string_lossy()],
        )
        .unwrap();
        rename(&conn, id, "Кино и сериалы").unwrap();
        assert!(t.path().join("Кино и сериалы").exists());
        let to: String = conn.query_row("SELECT to_path FROM move_item", [], |r| r.get(0)).unwrap();
        assert!(to.contains("Кино и сериалы"));
    }

    #[test]
    fn removes_only_empty() {
        let (t, table, conn) = setup(&["Пусто", "Полно"]);
        fs::write(t.path().join("Полно").join("a.mp4"), b"a").unwrap();
        fs::write(t.path().join("Полно").join("b.jpg"), b"b").unwrap();
        fs::write(t.path().join("Полно").join("c.txt"), b"c").unwrap();
        let piles = list(&conn, &table).unwrap();
        let full = piles.iter().find(|p| p.name == "Полно").unwrap();
        assert_eq!(full.count, 2);
        let id = |n: &str| piles.iter().find(|p| p.name == n).unwrap().id;
        assert_eq!(remove(&conn, id("Полно")).unwrap_err().code, "PILE_NOT_EMPTY");
        remove(&conn, id("Пусто")).unwrap();
        assert!(!t.path().join("Пусто").exists());
        assert!(t.path().join("Полно").join("a.mp4").exists());
        assert_eq!(list(&conn, &table).unwrap().len(), 2);
        create(&conn, &table, "Пусто").unwrap();
        assert_eq!(list(&conn, &table).unwrap().len(), 3);
    }

    #[test]
    fn create_rejects_existing() {
        let (_t, table, conn) = setup(&["Мемы"]);
        assert_eq!(create(&conn, &table, "мемы").unwrap_err().code, "EXISTS");
        assert!(create(&conn, &table, "Танцы").is_ok());
    }
}
