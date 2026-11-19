use crate::db::{get_setting, now_ms, set_setting};
use crate::deck::{self, CardView};
use crate::error::{AppError, AppResult};
use crate::paths;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

pub static STOP: AtomicBool = AtomicBool::new(false);
static READING: Mutex<Option<(String, usize)>> = Mutex::new(None);
static ADDING: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RootView {
    pub path: String,
    pub name: String,
    pub dirs: i64,
    pub video: i64,
    pub photo: i64,
    pub online: bool,
    pub reading: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableView {
    pub path: String,
    pub name: String,
    pub piles: i64,
    pub online: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RootError {
    pub path: String,
    pub code: String,
    pub message: String,
    pub review_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Draft {
    pub roots: Vec<RootView>,
    pub table: Option<TableView>,
    pub errors: Vec<RootError>,
    pub merged: Vec<Merged>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewView {
    pub id: i64,
    pub label: String,
    pub roots: Vec<RootView>,
    pub table: String,
    pub left: i64,
    pub total: i64,
    pub opened_at: i64,
    pub next: Option<CardView>,
    pub online: bool,
    pub gone: Option<String>,
    pub empty: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Merged {
    pub path: String,
    pub name: String,
    pub review_id: i64,
    pub label: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Home {
    pub draft: Draft,
    pub reviews: Vec<ReviewView>,
    pub tidy: Vec<ReviewView>,
    pub last: Option<i64>,
}

pub fn draft_id(conn: &Connection) -> rusqlite::Result<i64> {
    if let Some(id) = conn.query_row("SELECT id FROM review WHERE started = 0", [], |r| r.get(0)).optional()? {
        return Ok(id);
    }
    let table = get_setting(conn, "table_path")?;
    conn.execute("INSERT INTO review(table_path, started, created_at) VALUES (?1, 0, ?2)", params![table, now_ms()])?;
    Ok(conn.last_insert_rowid())
}

fn root_paths(conn: &Connection, review: i64) -> rusqlite::Result<Vec<(String, bool)>> {
    conn.prepare("SELECT path, online FROM review_root WHERE review_id = ?1 ORDER BY ord")?
        .query_map(params![review], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect()
}

fn root_view(conn: &Connection, path: &str, online: bool) -> rusqlite::Result<RootView> {
    let mut dirs = HashSet::new();
    let (mut video, mut photo) = (0, 0);
    let mut st = conn.prepare("SELECT file_name, kind FROM card WHERE deck_path = ?1 AND status IN ('in_deck','deferred')")?;
    for row in st.query_map(params![path], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
        let (name, kind) = row?;
        dirs.insert(paths::parent_dirs(&name).unwrap_or("").to_lowercase());
        if kind == "photo" { photo += 1 } else { video += 1 }
    }
    let reading = READING.lock().ok().and_then(|r| r.as_ref().filter(|(p, _)| paths::same(p, path)).map(|(_, n)| *n));
    Ok(RootView { path: path.to_string(), name: paths::name(path), dirs: dirs.len() as i64, video, photo, online, reading })
}

fn table_view(conn: &Connection, path: &str) -> rusqlite::Result<TableView> {
    let piles = conn.query_row(
        "SELECT COUNT(*) FROM pile WHERE table_path = ?1 AND is_trash = 0 AND exists_on_disk = 1",
        params![path],
        |r| r.get(0),
    )?;
    Ok(TableView { path: path.to_string(), name: paths::name(path), piles, online: Path::new(path).is_dir() })
}

pub fn draft(conn: &Connection) -> AppResult<Draft> {
    let id = draft_id(conn)?;
    let mut roots = Vec::new();
    for (p, _) in root_paths(conn, id)? {
        roots.push(root_view(conn, &p, Path::new(&p).is_dir())?);
    }
    let table: Option<String> = conn.query_row("SELECT table_path FROM review WHERE id = ?1", params![id], |r| r.get(0))?;
    let table = match table {
        Some(t) => Some(table_view(conn, &t)?),
        None => None,
    };
    Ok(Draft { roots, table, errors: Vec::new(), merged: Vec::new() })
}

pub fn label(conn: &Connection, review: i64) -> rusqlite::Result<String> {
    let names: Vec<String> = root_paths(conn, review)?.into_iter().map(|(p, _)| paths::name(&p)).collect();
    Ok(match names.len() {
        0 => String::new(),
        1 | 2 => names.join(", "),
        n => format!("{}, {} и ещё {}", names[0], names[1], n - 2),
    })
}

fn left_of(conn: &Connection, review: i64) -> rusqlite::Result<(i64, i64)> {
    conn.query_row(
        "SELECT COALESCE(SUM(status IN ('in_deck','deferred')), 0), COALESCE(SUM(status = 'placed'), 0) FROM card
         WHERE deck_path IN (SELECT path FROM review_root WHERE review_id = ?1)",
        params![review],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
}

pub fn offline_left(conn: &Connection, review: i64) -> rusqlite::Result<i64> {
    conn.query_row(
        "SELECT COUNT(*) FROM card WHERE status IN ('in_deck','deferred')
         AND deck_path IN (SELECT path FROM review_root WHERE review_id = ?1 AND online = 0)",
        params![review],
        |r| r.get(0),
    )
}

fn err(path: &str, code: &str, message: String, review_id: Option<i64>) -> RootError {
    RootError { path: path.to_string(), code: code.into(), message, review_id }
}

fn owner(conn: &Connection, path: &str) -> rusqlite::Result<Option<(String, i64, bool)>> {
    conn.query_row(
        "SELECT rr.path, rr.review_id, r.started FROM review_root rr JOIN review r ON r.id = rr.review_id WHERE rr.path = ?1 COLLATE NOCASE",
        params![path],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )
    .optional()
}

fn all_roots(conn: &Connection) -> rusqlite::Result<Vec<(String, i64, bool)>> {
    conn.prepare("SELECT rr.path, rr.review_id, r.started FROM review_root rr JOIN review r ON r.id = rr.review_id")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect()
}

fn unfinished(conn: &Connection, review: i64) -> rusqlite::Result<bool> {
    Ok(left_of(conn, review)?.0 > 0)
}

fn busy_error(conn: &Connection, path: &str, review: i64) -> rusqlite::Result<RootError> {
    let l = label(conn, review)?;
    Ok(err(path, "IN_REVIEW", format!("Уже в разборе «{l}» — продолжи его"), Some(review)))
}

fn rekey_into(conn: &Connection, inner: &str, outer: &str) -> rusqlite::Result<()> {
    let base = outer.trim_end_matches(std::path::MAIN_SEPARATOR).len();
    let rel = &inner[base + 1..];
    conn.execute(
        "UPDATE OR IGNORE card SET deck_path = ?2, file_name = ?3 || '\\' || file_name WHERE deck_path = ?1 COLLATE NOCASE",
        params![inner, outer, rel],
    )?;
    conn.execute("DELETE FROM review_root WHERE path = ?1 COLLATE NOCASE", params![inner])?;
    Ok(())
}

fn split_out(conn: &Connection, outer: &str, inner: &str) -> rusqlite::Result<()> {
    let base = outer.trim_end_matches(std::path::MAIN_SEPARATOR).len();
    let rel = format!("{}\\", &inner[base + 1..]);
    conn.execute(
        "UPDATE OR IGNORE card SET deck_path = ?2, file_name = substr(file_name, length(?3) + 1)
         WHERE deck_path = ?1 COLLATE NOCASE AND lower(substr(file_name, 1, length(?3))) = lower(?3)",
        params![outer, inner, rel],
    )?;
    Ok(())
}

pub enum Check {
    Add(i64),
    Skip,
    Fail(RootError),
}

pub fn check_root(conn: &Connection, draft: i64, path: &str) -> AppResult<Check> {
    let p = Path::new(path);
    if !p.is_dir() {
        let text = if p.is_file() { "Это файл — перетащи папку" } else { "Такой папки нет — выбери другую" };
        return Ok(Check::Fail(err(path, "NOT_FOLDER", text.into(), None)));
    }
    if std::fs::read_dir(p).is_err() {
        return Ok(Check::Fail(err(path, "NO_ACCESS", "Не получается прочитать эту папку — проверь доступ".into(), None)));
    }
    let table: Option<String> = conn.query_row("SELECT table_path FROM review WHERE id = ?1", params![draft], |r| r.get(0))?;
    if let Some(t) = &table
        && (paths::same(t, path) || paths::is_inside(path, t))
    {
        return Ok(Check::Fail(err(path, "INSIDE_TABLE", "Это стол или папка в нём — выбери другую папку для разбора".into(), None)));
    }
    if let Some((_, review, started)) = owner(conn, path)? {
        if review == draft {
            return Ok(Check::Skip);
        }
        if started && unfinished(conn, review)? {
            return Ok(Check::Fail(busy_error(conn, path, review)?));
        }
        conn.execute("DELETE FROM review_root WHERE path = ?1 COLLATE NOCASE", params![path])?;
    }
    let mut target = draft;
    for (root, review, started) in all_roots(conn)? {
        if paths::is_inside(path, &root) {
            if review == draft {
                return Ok(Check::Fail(err(path, "INSIDE_ROOT", format!("Уже внутри «{}»", paths::name(&root)), None)));
            }
            if started && unfinished(conn, review)? {
                return Ok(Check::Fail(busy_error(conn, path, review)?));
            }
            split_out(conn, &root, path)?;
        } else if paths::is_inside(&root, path) && started && review != draft && unfinished(conn, review)? {
            if target != draft && target != review {
                return Ok(Check::Fail(err(path, "TWO_REVIEWS", "Внутри папки из двух начатых разборов — закончи один из них".into(), Some(review))));
            }
            target = review;
        }
    }
    if target != draft {
        let t: Option<String> = conn.query_row("SELECT table_path FROM review WHERE id = ?1", params![target], |r| r.get(0))?;
        if let Some(t) = &t
            && (paths::same(t, path) || paths::is_inside(path, t))
        {
            return Ok(Check::Fail(err(path, "INSIDE_TABLE", "Это стол или папка в нём — выбери другую папку для разбора".into(), None)));
        }
    }
    for (root, _, _) in all_roots(conn)? {
        if paths::is_inside(&root, path) {
            rekey_into(conn, &root, path)?;
        }
    }
    Ok(Check::Add(target))
}

pub fn add_roots(conn: &Connection, input: &[String], seen: &mut dyn FnMut(&str, usize)) -> AppResult<Draft> {
    let _one = ADDING.lock().unwrap_or_else(|e| e.into_inner());
    STOP.store(false, Ordering::Relaxed);
    purge_hidden(conn, None)?;
    let id = draft_id(conn)?;
    let mut errors = Vec::new();
    let mut merged = Vec::new();
    for raw in input {
        let path = paths::canon(raw);
        let tx = rusqlite::Transaction::new_unchecked(conn, rusqlite::TransactionBehavior::Immediate)?;
        let check = check_root(&tx, id, &path)?;
        if let Check::Add(target) = check {
            tx.execute(
                "INSERT INTO review_root(review_id, path, ord) VALUES (?1, ?2, (SELECT COALESCE(MAX(ord), 0) + 1 FROM review_root WHERE review_id = ?1))",
                params![target, path],
            )?;
        }
        if matches!(check, Check::Fail(_)) {
            tx.rollback()?;
        } else {
            tx.commit()?;
        }
        match check {
            Check::Fail(e) => errors.push(e),
            Check::Skip => {}
            Check::Add(target) => {
                if let Ok(mut r) = READING.lock() {
                    *r = Some((path.clone(), 0));
                }
                let result = deck::sync_seen(conn, &path, &STOP, &mut |n| {
                    if let Ok(mut r) = READING.lock() {
                        *r = Some((path.clone(), n));
                    }
                    seen(&path, n);
                });
                if let Ok(mut r) = READING.lock() {
                    *r = None;
                }
                let stopped = STOP.load(Ordering::Relaxed);
                let cards: i64 = conn.query_row("SELECT COUNT(*) FROM card WHERE deck_path = ?1 COLLATE NOCASE", params![path], |r| r.get(0))?;
                let empty = result.is_ok() && root_view(conn, &path, true)?.video + root_view(conn, &path, true)?.photo == 0;
                if empty || ((stopped || result.is_err()) && cards == 0) {
                    conn.execute("DELETE FROM review_root WHERE review_id = ?1 AND path = ?2", params![target, path])?;
                }
                if stopped {
                    break;
                }
                if let Err(e) = result {
                    errors.push(err(&path, &e.code, e.message, None));
                } else if empty {
                    errors.push(err(&path, "NO_MEDIA", "Видео и фото здесь нет".into(), None));
                } else if target != id {
                    merged.push(Merged { name: paths::name(&path), path: path.clone(), review_id: target, label: label(conn, target)? });
                }
            }
        }
    }
    let mut d = draft(conn)?;
    d.errors = errors;
    d.merged = merged;
    Ok(d)
}

pub fn remove_root(conn: &Connection, path: &str) -> AppResult<Draft> {
    let id = draft_id(conn)?;
    conn.execute("DELETE FROM review_root WHERE review_id = ?1 AND path = ?2 COLLATE NOCASE", params![id, path])?;
    draft(conn)
}

pub fn set_table(conn: &Connection, raw: &str) -> AppResult<Draft> {
    purge_hidden(conn, None)?;
    let path = paths::canon(raw);
    let id = draft_id(conn)?;
    if !Path::new(&path).is_dir() {
        let mut d = draft(conn)?;
        let text = if Path::new(&path).is_file() { "Это файл — перетащи папку" } else { "Такой папки нет — выбери другую" };
        d.errors.push(err(&path, "NOT_FOLDER", text.into(), None));
        return Ok(d);
    }
    let roots = root_paths(conn, id)?;
    if let Some((r, _)) = roots.iter().find(|(r, _)| paths::same(r, &path) || paths::is_inside(r, &path)) {
        let mut d = draft(conn)?;
        d.errors.push(err(&path, "ROOT_INSIDE", format!("Внутри папка для разбора «{}» — выбери другой стол", paths::name(r)), None));
        return Ok(d);
    }
    conn.execute("UPDATE review SET table_path = ?2 WHERE id = ?1", params![id, path])?;
    crate::piles::sync(conn, &path)?;
    let stop = AtomicBool::new(false);
    for (r, _) in roots.iter().filter(|(r, _)| paths::is_inside(&path, r)) {
        deck::sync_seen(conn, r, &stop, &mut |_| {})?;
    }
    draft(conn)
}

fn view(conn: &Connection, id: i64) -> AppResult<ReviewView> {
    let (table, opened_at): (Option<String>, Option<i64>) =
        conn.query_row("SELECT table_path, opened_at FROM review WHERE id = ?1", params![id], |r| Ok((r.get(0)?, r.get(1)?)))?;
    let mut roots = Vec::new();
    for (p, online) in root_paths(conn, id)? {
        roots.push(root_view(conn, &p, online)?);
    }
    let (left, placed) = left_of(conn, id)?;
    let next: Option<i64> = conn
        .query_row(
            "SELECT id FROM card WHERE deck_path IN (SELECT path FROM review_root WHERE review_id = ?1 AND online = 1)
               AND status IN ('in_deck','deferred') AND (series_id IS NULL OR series_id = id)
             ORDER BY position LIMIT 1",
            params![id],
            |r| r.get(0),
        )
        .optional()?;
    let next = match next {
        Some(c) => deck::cards(conn, &[c])?.into_iter().next(),
        None => None,
    };
    let gone = match &table {
        Some(t) if !Path::new(t).is_dir() => Some(t.clone()),
        _ => roots.iter().find(|r| !r.online).map(|r| r.path.clone()),
    };
    Ok(ReviewView {
        id,
        label: label(conn, id)?,
        online: next.is_some() && table.as_deref().is_some_and(|t| Path::new(t).is_dir()),
        gone,
        roots,
        table: table.map(|t| paths::name(&t)).unwrap_or_default(),
        left,
        total: left + placed,
        opened_at: opened_at.unwrap_or(0),
        next,
        empty: 0,
    })
}

pub fn recheck(conn: &Connection) -> AppResult<()> {
    let all: Vec<String> = conn.prepare("SELECT path FROM review_root")?.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?;
    for p in all {
        conn.execute("UPDATE review_root SET online = ?2 WHERE path = ?1", params![p, Path::new(&p).is_dir()])?;
    }
    Ok(())
}

pub fn list(conn: &Connection) -> AppResult<(Vec<ReviewView>, Vec<ReviewView>)> {
    let ids: Vec<(i64, bool)> = conn
        .prepare("SELECT id, tidied FROM review WHERE started = 1 AND hidden = 0 ORDER BY COALESCE(opened_at, created_at) DESC")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    let (mut out, mut tidy) = (Vec::new(), Vec::new());
    for (id, tidied) in ids {
        if unfinished(conn, id)? {
            out.push(view(conn, id)?);
        } else if !tidied && root_paths(conn, id)?.iter().all(|(_, online)| *online) {
            let n = empty_dirs(conn, id)?.len();
            if n == 0 {
                keep_dirs(conn, id)?;
            } else {
                let mut v = view(conn, id)?;
                v.empty = n;
                tidy.push(v);
            }
        }
    }
    Ok((out, tidy))
}

pub fn home(conn: &Connection) -> AppResult<Home> {
    recheck(conn)?;
    let (reviews, tidy) = list(conn)?;
    let last = reviews.first().map(|r| r.id);
    Ok(Home { draft: draft(conn)?, reviews, tidy, last })
}

pub fn table_path_of(conn: &Connection, id: i64) -> AppResult<String> {
    let table: Option<String> = conn.query_row("SELECT table_path FROM review WHERE id = ?1", params![id], |r| r.get(0)).optional()?.flatten();
    table.ok_or_else(|| AppError::new("NO_REVIEW", "Этого разбора больше нет"))
}

pub fn keep_dirs(conn: &Connection, id: i64) -> AppResult<()> {
    conn.execute("UPDATE review SET tidied = 1 WHERE id = ?1", params![id])?;
    Ok(())
}

fn purge_hidden(conn: &Connection, except: Option<i64>) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM review WHERE hidden = 1 AND id IS NOT ?1", params![except])?;
    Ok(())
}

pub fn restore_review(conn: &Connection, id: i64) -> AppResult<()> {
    if conn.execute("UPDATE review SET hidden = 0 WHERE id = ?1 AND hidden = 1", params![id])? == 0 {
        return Err(AppError::new("NO_REVIEW", "Этого разбора больше нет"));
    }
    Ok(())
}

pub fn relocate(conn: &Connection, id: i64, old: &str, raw: &str) -> AppResult<()> {
    let new = paths::canon(raw);
    let p = Path::new(&new);
    if !p.is_dir() {
        let text = if p.is_file() { "Это файл — перетащи папку" } else { "Такой папки нет — выбери другую" };
        return Err(AppError::new("NOT_FOLDER", text));
    }
    if fs::read_dir(p).is_err() {
        return Err(AppError::new("NO_ACCESS", "Не получается прочитать эту папку — проверь доступ"));
    }
    let known: bool = conn
        .prepare("SELECT 1 FROM review_root WHERE review_id = ?1 AND path = ?2 COLLATE NOCASE")?
        .exists(params![id, old])?;
    if !known {
        return Err(AppError::new("NO_REVIEW", "Этой папки в разборе уже нет"));
    }
    if table_of(conn, id)?.iter().any(|t| paths::same(t, &new) || paths::is_inside(&new, t) || paths::is_inside(t, &new)) {
        return Err(AppError::new("INSIDE_TABLE", "Здесь стол или папка в нём — перетащи папку, которую разбирал"));
    }
    for (root, review, _) in all_roots(conn)? {
        if paths::same(&root, old) {
            continue;
        }
        if paths::same(&root, &new) || paths::is_inside(&new, &root) || paths::is_inside(&root, &new) {
            let l = label(conn, review)?;
            return Err(AppError::new("IN_REVIEW", format!("Эта папка уже в разборе «{l}»")));
        }
    }
    let tx = rusqlite::Transaction::new_unchecked(conn, rusqlite::TransactionBehavior::Immediate)?;
    tx.execute("UPDATE OR IGNORE card SET deck_path = ?2 WHERE deck_path = ?1 COLLATE NOCASE", params![old, new])?;
    tx.execute("UPDATE review_root SET path = ?3, online = 1 WHERE review_id = ?1 AND path = ?2 COLLATE NOCASE", params![id, old, new])?;
    tx.execute(
        "UPDATE move_item SET from_path = ?2 || substr(from_path, length(?1) + 1)
         WHERE lower(substr(from_path, 1, length(?1) + 1)) = lower(?1 || '\\')",
        params![old.trim_end_matches('\\'), new.trim_end_matches('\\')],
    )?;
    tx.commit()?;
    let stop = AtomicBool::new(false);
    deck::sync_seen(conn, &new, &stop, &mut |_| {})?;
    Ok(())
}

pub fn start(conn: &Connection) -> AppResult<ReviewView> {
    if READING.lock().is_ok_and(|r| r.is_some()) {
        return Err(AppError::new("READING", "Папки ещё читаются — подожди"));
    }
    purge_hidden(conn, None)?;
    let id = draft_id(conn)?;
    let table: Option<String> = conn.query_row("SELECT table_path FROM review WHERE id = ?1", params![id], |r| r.get(0))?;
    let Some(table) = table else {
        return Err(AppError::new("NOT_READY", "Сначала выбери, куда раскладывать"));
    };
    if root_paths(conn, id)?.is_empty() {
        return Err(AppError::new("NOT_READY", "Сначала добавь папку для разбора"));
    }
    let now = now_ms();
    let tx = conn.unchecked_transaction()?;
    tx.execute("UPDATE review SET started = 1, created_at = ?2, opened_at = ?2 WHERE id = ?1", params![id, now])?;
    tx.execute("INSERT INTO review(table_path, started, created_at) VALUES (?1, 0, ?2)", params![table, now])?;
    set_setting(&tx, "review_id", &id.to_string())?;
    set_setting(&tx, "table_path", &table)?;
    tx.commit()?;
    view(conn, id)
}

pub fn open(conn: &Connection, id: i64) -> AppResult<ReviewView> {
    purge_hidden(conn, None)?;
    let table: Option<Option<String>> =
        conn.query_row("SELECT table_path FROM review WHERE id = ?1 AND started = 1", params![id], |r| r.get(0)).optional()?;
    let Some(table) = table else {
        return Err(AppError::new("NO_REVIEW", "Этого разбора больше нет"));
    };
    recheck(conn)?;
    conn.execute("UPDATE review SET opened_at = ?2 WHERE id = ?1", params![id, now_ms()])?;
    set_setting(conn, "review_id", &id.to_string())?;
    if let Some(t) = table {
        set_setting(conn, "table_path", &t)?;
    }
    view(conn, id)
}

pub fn drop_review(conn: &Connection, id: i64) -> AppResult<()> {
    purge_hidden(conn, Some(id))?;
    conn.execute("UPDATE review SET hidden = 1 WHERE id = ?1 AND started = 1", params![id])?;
    if get_setting(conn, "review_id")?.as_deref() == Some(&id.to_string()) {
        conn.execute("DELETE FROM settings WHERE key = 'review_id'", [])?;
    }
    Ok(())
}

pub fn close(conn: &Connection) -> AppResult<()> {
    purge_hidden(conn, None)?;
    conn.execute("DELETE FROM settings WHERE key = 'review_id'", [])?;
    Ok(())
}

pub fn open_id(conn: &Connection) -> rusqlite::Result<Option<i64>> {
    Ok(get_setting(conn, "review_id")?.and_then(|v| v.parse().ok()))
}

const LITTER: [&str; 3] = ["thumbs.db", "desktop.ini", ".ds_store"];

fn scan_empty(dir: &Path, keep: bool, skip: &[String], out: &mut Vec<String>) -> bool {
    let Ok(entries) = fs::read_dir(dir) else { return false };
    let mut empty = true;
    let mut subs = Vec::new();
    for e in entries {
        let Ok(e) = e else { return false };
        match e.file_type() {
            Ok(t) if t.is_dir() => {
                let p = e.path();
                let ps = p.to_string_lossy();
                if paths::system_dir(&e.file_name().to_string_lossy()) || skip.iter().any(|k| paths::same(&ps, k) || paths::is_inside(&ps, k)) {
                    empty = false;
                    continue;
                }
                let mut inner = Vec::new();
                if scan_empty(&p, false, skip, &mut inner) {
                    subs.push(p.to_string_lossy().into_owned());
                } else {
                    empty = false;
                    out.extend(inner);
                }
            }
            Ok(t) if t.is_file() && LITTER.contains(&e.file_name().to_string_lossy().to_lowercase().as_str()) => {}
            _ => empty = false,
        }
    }
    if !empty || keep {
        out.extend(subs);
    }
    empty
}

fn roots_of(conn: &Connection, review: i64) -> rusqlite::Result<Vec<String>> {
    conn.prepare("SELECT path FROM review_root WHERE review_id = ?1 ORDER BY ord")?
        .query_map(params![review], |r| r.get(0))?
        .collect()
}

fn table_of(conn: &Connection, _review: i64) -> rusqlite::Result<Vec<String>> {
    conn.prepare("SELECT DISTINCT table_path FROM review WHERE table_path IS NOT NULL")?.query_map([], |r| r.get(0))?.collect()
}

pub fn empty_dirs(conn: &Connection, review: i64) -> AppResult<Vec<String>> {
    let skip = table_of(conn, review)?;
    let mut out = Vec::new();
    for r in roots_of(conn, review)? {
        let p = Path::new(&r);
        if p.is_dir() {
            scan_empty(p, true, &skip, &mut out);
        }
    }
    out.sort();
    Ok(out)
}

pub fn check_empty(conn: &Connection, review: i64, input: &[String]) -> AppResult<Vec<PathBuf>> {
    let roots = roots_of(conn, review)?;
    let skip = table_of(conn, review)?;
    let mut out = Vec::with_capacity(input.len());
    for d in input {
        let d = &paths::canon(d);
        let near_table = skip.iter().any(|k| paths::same(d, k) || paths::is_inside(d, k) || paths::is_inside(k, d));
        if near_table || !roots.iter().any(|r| paths::is_inside(d, r)) {
            return Err(AppError::new("NOT_IN_REVIEW", format!("«{}» не из этого разбора", paths::name(d))));
        }
        let p = Path::new(d);
        if !p.is_dir() {
            continue;
        }
        if !scan_empty(p, false, &skip, &mut Vec::new()) {
            return Err(AppError::new("NOT_EMPTY", format!("В «{}» появились файлы — папка осталась на месте", paths::name(d))));
        }
        out.push(p.to_path_buf());
    }
    if out.is_empty() {
        return Err(AppError::new("NOTHING", "Пустых папок уже нет"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_dirs_finds_topmost_and_keeps_roots() {
        let dir = tempfile::tempdir().unwrap();
        let root = paths::canon(&dir.path().to_string_lossy());
        let r = Path::new(&root);
        for sub in ["2ch\\t1\\deep", "2ch\\t2", "2ch\\t3", "2ch\\t4"] {
            fs::create_dir_all(r.join(sub)).unwrap();
        }
        fs::write(r.join("2ch\\t2\\a.mp4"), b"x").unwrap();
        fs::write(r.join("2ch\\t3\\Thumbs.db"), b"x").unwrap();
        let conn = Connection::open_in_memory().unwrap();
        crate::db::init(&conn).unwrap();
        conn.execute("INSERT INTO review(id, table_path, started, created_at) VALUES (7, NULL, 1, 0)", []).unwrap();
        let top = r.join("2ch").to_string_lossy().into_owned();
        conn.execute("INSERT INTO review_root(review_id, path, ord) VALUES (7, ?1, 0)", params![top]).unwrap();
        let found: Vec<String> = empty_dirs(&conn, 7).unwrap().iter().map(|p| paths::name(p)).collect();
        assert_eq!(found, ["t1", "t3", "t4"]);
        fs::write(r.join("2ch\\t4\\b.jpg"), b"x").unwrap();
        let all = empty_dirs(&conn, 7).unwrap();
        assert_eq!(all.len(), 2);
        let late = vec![all[0].clone(), r.join("2ch\\t4").to_string_lossy().into_owned()];
        assert_eq!(check_empty(&conn, 7, &late).unwrap_err().code, "NOT_EMPTY");
        let outside = vec![root.clone()];
        assert_eq!(check_empty(&conn, 7, &outside).unwrap_err().code, "NOT_IN_REVIEW");
        fs::create_dir_all(r.join("2ch\\table\\Мемы")).unwrap();
        let table = r.join("2ch\\table").to_string_lossy().into_owned();
        conn.execute("UPDATE review SET table_path = ?1 WHERE id = 7", params![table]).unwrap();
        assert!(!empty_dirs(&conn, 7).unwrap().iter().any(|p| p.contains("table")));
        let pile = vec![r.join("2ch\\table\\Мемы").to_string_lossy().into_owned()];
        assert_eq!(check_empty(&conn, 7, &pile).unwrap_err().code, "NOT_IN_REVIEW");
    }

    fn setup() -> (tempfile::TempDir, Connection, String) {
        let dir = tempfile::tempdir().unwrap();
        let root = paths::canon(&dir.path().to_string_lossy());
        for (sub, n) in [("2ch\\t1", 2), ("2ch\\t2", 3), ("table\\Мемы", 0)] {
            let d = Path::new(&root).join(sub);
            fs::create_dir_all(&d).unwrap();
            for i in 0..n {
                fs::write(d.join(format!("{i}.mp4")), format!("{sub}{i}")).unwrap();
            }
        }
        let conn = Connection::open_in_memory().unwrap();
        crate::db::init(&conn).unwrap();
        (dir, conn, root)
    }

    #[test]
    fn nested_root_is_absorbed_and_frames_kept() {
        let (_d, conn, root) = setup();
        let t1 = format!("{root}\\2ch\\t1");
        let ch = format!("{root}\\2ch");
        let d = add_roots(&conn, std::slice::from_ref(&t1), &mut |_, _| {}).unwrap();
        assert_eq!((d.roots.len(), d.roots[0].video), (1, 2));
        conn.execute("UPDATE card SET stage = 'embedded'", []).unwrap();
        let d = add_roots(&conn, std::slice::from_ref(&ch), &mut |_, _| {}).unwrap();
        assert_eq!(d.roots.len(), 1);
        assert_eq!((d.roots[0].name.as_str(), d.roots[0].video, d.roots[0].dirs), ("2ch", 5, 2));
        assert_eq!(conn.query_row("SELECT COUNT(*) FROM card WHERE stage = 'embedded' AND file_name LIKE 't1\\%'", [], |r| r.get::<_, i64>(0)).unwrap(), 2);
        let d = add_roots(&conn, &[t1], &mut |_, _| {}).unwrap();
        assert_eq!(d.errors[0].code, "INSIDE_ROOT");
    }

    #[test]
    fn finished_root_comes_back_without_duplicates() {
        let (_d, conn, root) = setup();
        let ch = format!("{root}\\2ch");
        let t1 = format!("{root}\\2ch\\t1");
        set_table(&conn, &format!("{root}\\table")).unwrap();
        add_roots(&conn, std::slice::from_ref(&ch), &mut |_, _| {}).unwrap();
        start(&conn).unwrap();
        conn.execute("UPDATE card SET status = 'placed'", []).unwrap();
        add_roots(&conn, std::slice::from_ref(&t1), &mut |_, _| {}).unwrap();
        let d = add_roots(&conn, std::slice::from_ref(&ch), &mut |_, _| {}).unwrap();
        assert!(d.errors.is_empty());
        assert_eq!(d.roots.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), ["2ch"]);
        let cards: i64 = conn.query_row("SELECT COUNT(*) FROM card WHERE status = 'in_deck'", [], |r| r.get(0)).unwrap();
        assert_eq!(cards, 5);
    }

    #[test]
    fn table_inside_is_refused_and_review_starts() {
        let (_d, conn, root) = setup();
        let table = format!("{root}\\table");
        set_table(&conn, &table).unwrap();
        let d = add_roots(&conn, &[format!("{root}\\table\\Мемы"), format!("{root}\\nope")], &mut |_, _| {}).unwrap();
        assert_eq!(d.errors.iter().map(|e| e.code.as_str()).collect::<Vec<_>>(), vec!["INSIDE_TABLE", "NOT_FOLDER"]);
        assert!(start(&conn).is_err());
        add_roots(&conn, &[format!("{root}\\2ch")], &mut |_, _| {}).unwrap();
        let r = start(&conn).unwrap();
        assert_eq!((r.left, r.label.as_str()), (5, "2ch"));
        assert_eq!(deck::window(&conn, 0, 10, "all").unwrap().len(), 5);
        let d = add_roots(&conn, &[format!("{root}\\2ch\\t2")], &mut |_, _| {}).unwrap();
        assert_eq!(d.errors[0].code, "IN_REVIEW");
        assert_eq!(list(&conn).unwrap().0.len(), 1);
        conn.execute("UPDATE card SET status = 'placed'", []).unwrap();
        assert!(list(&conn).unwrap().0.is_empty());
        let d = add_roots(&conn, &[format!("{root}\\2ch\\t2")], &mut |_, _| {}).unwrap();
        assert!(d.errors.is_empty() && d.roots.len() == 1);
    }

    #[test]
    fn parent_joins_started_review() {
        let (_d, conn, root) = setup();
        set_table(&conn, &format!("{root}\\table")).unwrap();
        add_roots(&conn, &[format!("{root}\\2ch\\t1")], &mut |_, _| {}).unwrap();
        let r = start(&conn).unwrap();
        conn.execute("UPDATE card SET status = 'placed' WHERE file_name = '0.mp4'", []).unwrap();
        fs::remove_file(Path::new(&root).join("2ch\\t1\\0.mp4")).unwrap();
        let d = add_roots(&conn, &[format!("{root}\\2ch")], &mut |_, _| {}).unwrap();
        assert!(d.errors.is_empty() && d.roots.is_empty());
        assert_eq!((d.merged[0].review_id, d.merged[0].name.as_str()), (r.id, "2ch"));
        let v = list(&conn).unwrap().0;
        assert_eq!((v[0].label.as_str(), v[0].left, v[0].total), ("2ch", 4, 5));
    }

    #[test]
    fn moved_root_is_found_again() {
        let (_d, conn, root) = setup();
        set_table(&conn, &format!("{root}\\table")).unwrap();
        let old = format!("{root}\\2ch\\t2");
        add_roots(&conn, std::slice::from_ref(&old), &mut |_, _| {}).unwrap();
        let r = start(&conn).unwrap();
        let new = format!("{root}\\2ch\\t2-moved");
        fs::rename(&old, &new).unwrap();
        recheck(&conn).unwrap();
        assert_eq!(list(&conn).unwrap().0[0].gone.as_deref(), Some(old.as_str()));
        assert_eq!(relocate(&conn, r.id, &old, &format!("{root}\\table\\Мемы")).unwrap_err().code, "INSIDE_TABLE");
        assert_eq!(relocate(&conn, r.id, &old, &format!("{root}\\2ch\\nope")).unwrap_err().code, "NOT_FOLDER");
        relocate(&conn, r.id, &old, &new).unwrap();
        let v = list(&conn).unwrap().0;
        assert_eq!((v[0].gone.clone(), v[0].left, v[0].online), (None, 3, true));
    }

    #[test]
    fn dropped_review_comes_back_until_next_step() {
        let (_d, conn, root) = setup();
        set_table(&conn, &format!("{root}\\table")).unwrap();
        add_roots(&conn, &[format!("{root}\\2ch")], &mut |_, _| {}).unwrap();
        let r = start(&conn).unwrap();
        drop_review(&conn, r.id).unwrap();
        assert!(list(&conn).unwrap().0.is_empty());
        restore_review(&conn, r.id).unwrap();
        assert_eq!(list(&conn).unwrap().0.len(), 1);
        drop_review(&conn, r.id).unwrap();
        close(&conn).unwrap();
        assert_eq!(restore_review(&conn, r.id).unwrap_err().code, "NO_REVIEW");
        let d = add_roots(&conn, &[format!("{root}\\2ch")], &mut |_, _| {}).unwrap();
        assert!(d.errors.is_empty() && d.roots.len() == 1);
    }

    #[test]
    fn finished_review_offers_empty_dirs_once() {
        let (_d, conn, root) = setup();
        set_table(&conn, &format!("{root}\\table")).unwrap();
        add_roots(&conn, &[format!("{root}\\2ch")], &mut |_, _| {}).unwrap();
        start(&conn).unwrap();
        conn.execute("UPDATE card SET status = 'placed'", []).unwrap();
        for f in ["0.mp4", "1.mp4"] {
            fs::remove_file(Path::new(&root).join("2ch\\t1").join(f)).unwrap();
        }
        let (open, tidy) = list(&conn).unwrap();
        assert!(open.is_empty());
        assert_eq!(tidy[0].empty, 1);
        keep_dirs(&conn, tidy[0].id).unwrap();
        assert!(list(&conn).unwrap().1.is_empty());
    }

    #[test]
    fn table_set_inside_root_drops_its_files() {
        let (_d, conn, root) = setup();
        fs::write(Path::new(&root).join("table\\Мемы\\x.mp4"), b"x").unwrap();
        let d = add_roots(&conn, std::slice::from_ref(&root), &mut |_, _| {}).unwrap();
        assert_eq!(d.roots[0].video, 6);
        let d = set_table(&conn, &format!("{root}\\table")).unwrap();
        assert_eq!(d.roots[0].video, 5);
    }
}
