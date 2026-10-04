use crate::db::now_ms;
use crate::error::{file_label, io_error, AppError, AppResult};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::Duration;

pub type RenameFn = fn(&Path, &Path) -> io::Result<()>;

const NOT_SAME_DEVICE: i32 = 17;
const RESERVED: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1",
    "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveItemView {
    pub card_id: Option<i64>,
    pub file_name: String,
    pub final_name: Option<String>,
    pub from_path: String,
    pub to_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveView {
    pub id: i64,
    pub at: i64,
    pub method: String,
    pub pile_id: i64,
    pub pile_name: String,
    pub is_trash: bool,
    pub state: String,
    pub error: Option<String>,
    pub items: Vec<MoveItemView>,
}

struct PileRow {
    name: String,
    table_path: String,
    is_trash: bool,
    exists: bool,
}

struct Item {
    id: i64,
    card_id: Option<i64>,
    from: PathBuf,
    to: Option<PathBuf>,
    step: String,
}

#[cfg(windows)]
pub fn safe_rename(from: &Path, to: &Path) -> io::Result<()> {
    use windows::core::HSTRING;
    use windows::Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_WRITE_THROUGH};
    unsafe { MoveFileExW(&HSTRING::from(from.as_os_str()), &HSTRING::from(to.as_os_str()), MOVEFILE_WRITE_THROUGH) }
        .map_err(|e| io::Error::from_raw_os_error(e.code().0 & 0xFFFF))
}

#[cfg(not(windows))]
pub fn safe_rename(from: &Path, to: &Path) -> io::Result<()> {
    if to.exists() {
        return Err(io::Error::from_raw_os_error(183));
    }
    fs::rename(from, to)
}

fn retry<T>(mut f: impl FnMut() -> io::Result<T>) -> io::Result<T> {
    let mut last = None;
    for attempt in 0..4 {
        match f() {
            Ok(v) => return Ok(v),
            Err(e) if matches!(e.raw_os_error(), Some(32) | Some(33)) && attempt < 3 => {
                last = Some(e);
                sleep(Duration::from_millis(100));
            }
            Err(e) => return Err(e),
        }
    }
    Err(last.unwrap_or_else(|| io::Error::other("retry")))
}

pub fn free_name(dir: &Path, name: &str, taken: &[String]) -> String {
    let taken: Vec<String> = taken.iter().map(|s| s.to_lowercase()).collect();
    let is_free = |n: &str| !dir.join(n).exists() && !taken.contains(&n.to_lowercase());
    if is_free(name) {
        return name.to_string();
    }
    let p = Path::new(name);
    let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or(name);
    let ext = p.extension().and_then(|s| s.to_str()).map(|e| format!(".{e}")).unwrap_or_default();
    (2..).map(|i| format!("{stem} ({i}){ext}")).find(|n| is_free(n)).expect("free name")
}

pub fn validate_pile_name(name: &str) -> AppResult<String> {
    let n = name.trim();
    if n.is_empty() {
        return Err(AppError::new("BAD_NAME", "Название стопки не может быть пустым."));
    }
    if n.chars().any(|c| "<>:\"/\\|?*".contains(c) || (c as u32) < 32) {
        return Err(AppError::new("BAD_NAME", "В названии нельзя использовать символы < > : \" / \\ | ? *"));
    }
    if n.ends_with('.') {
        return Err(AppError::new("BAD_NAME", "Название не может заканчиваться точкой."));
    }
    if n.chars().count() > 120 {
        return Err(AppError::new("BAD_NAME", "Название слишком длинное — оставь до 120 символов."));
    }
    let stem = n.split('.').next().unwrap_or(n).to_uppercase();
    if RESERVED.contains(&stem.as_str()) {
        return Err(AppError::new("BAD_NAME", format!("Имя «{n}» зарезервировано Windows — выбери другое.")));
    }
    Ok(n.to_string())
}

fn part_path(to: &Path) -> PathBuf {
    let name = to.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    to.with_file_name(format!(".{name}.sorter-part"))
}

fn transfer(
    from: &Path,
    to: &Path,
    rename: RenameFn,
    mut on_copied: impl FnMut() -> AppResult<()>,
) -> AppResult<()> {
    match retry(|| rename(from, to)) {
        Ok(()) => Ok(()),
        Err(e) if e.raw_os_error() == Some(NOT_SAME_DEVICE) => {
            let part = part_path(to);
            let copied = (|| -> io::Result<()> {
                fs::copy(from, &part)?;
                fs::OpenOptions::new().write(true).open(&part)?.sync_all()?;
                if fs::metadata(&part)?.len() != fs::metadata(from)?.len() {
                    return Err(io::Error::other("размер копии не совпал с исходником"));
                }
                safe_rename(&part, to)
            })();
            if let Err(e) = copied {
                let _ = fs::remove_file(&part);
                return Err(io_error(&e, from));
            }
            on_copied()?;
            retry(|| fs::remove_file(from)).map_err(|e| io_error(&e, from))
        }
        Err(e) => Err(io_error(&e, from)),
    }
}

fn same_path(a: &Path, b: &Path) -> bool {
    a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
}

fn trash_restore(from: &Path) -> AppResult<()> {
    if from.exists() {
        return Err(AppError::new(
            "FILE_IN_THE_WAY",
            format!("На месте «{}» уже лежит другой файл — убери его и попробуй снова.", file_label(from)),
        ));
    }
    let items = trash::os_limited::list().map_err(|e| AppError::new("TRASH", format!("Корзина Windows сейчас недоступна ({e}). Попробуй ещё раз или верни файл из корзины вручную.")))?;
    let found = items
        .into_iter()
        .filter(|i| same_path(&i.original_parent.join(&i.name), from))
        .max_by_key(|i| i.time_deleted);
    match found {
        Some(item) => trash::os_limited::restore_all([item])
            .map_err(|e| AppError::new("TRASH", format!("Не получилось вернуть «{}» из корзины ({e}). Открой корзину Windows и восстанови файл вручную.", file_label(from)))),
        None => Err(AppError::new(
            "FILE_NOT_THERE",
            format!("«{}» нет в корзине — возможно, корзину очистили.", file_label(from)),
        )),
    }
}

fn load_pile(conn: &Connection, pile_id: i64) -> AppResult<PileRow> {
    conn.query_row(
        "SELECT name, table_path, is_trash, exists_on_disk FROM pile WHERE id = ?1",
        params![pile_id],
        |r| Ok(PileRow { name: r.get(0)?, table_path: r.get(1)?, is_trash: r.get(2)?, exists: r.get(3)? }),
    )
    .optional()?
    .ok_or_else(|| AppError::new("PILE_GONE", "Этой стопки больше нет — её папку убрали со стола. Выбери другую стопку или создай её заново."))
}

fn set_step(conn: &Connection, item_id: i64, step: &str) -> AppResult<()> {
    conn.execute("UPDATE move_item SET step = ?2 WHERE id = ?1", params![item_id, step])?;
    Ok(())
}

fn set_state(conn: &Connection, move_id: i64, state: &str, error: Option<&str>) -> AppResult<()> {
    conn.execute("UPDATE move SET state = ?2, error = ?3 WHERE id = ?1", params![move_id, state, error])?;
    Ok(())
}

fn load_items(conn: &Connection, move_id: i64) -> AppResult<Vec<Item>> {
    let mut st = conn.prepare(
        "SELECT id, card_id, from_path, to_path, step FROM move_item WHERE move_id = ?1 ORDER BY id",
    )?;
    let rows = st.query_map(params![move_id], |r| {
        Ok(Item {
            id: r.get(0)?,
            card_id: r.get(1)?,
            from: PathBuf::from(r.get::<_, String>(2)?),
            to: r.get::<_, Option<String>>(3)?.map(PathBuf::from),
            step: r.get(4)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn load_move(conn: &Connection, move_id: i64) -> AppResult<MoveView> {
    let (at, method, pile_id, state, error, pile_name, is_trash) = conn.query_row(
        "SELECT m.at, m.method, m.pile_id, m.state, m.error, p.name, p.is_trash
         FROM move m JOIN pile p ON p.id = m.pile_id WHERE m.id = ?1",
        params![move_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)),
    )?;
    let items = load_items(conn, move_id)?
        .into_iter()
        .map(|i| MoveItemView {
            card_id: i.card_id,
            file_name: file_label(&i.from),
            final_name: i.to.as_deref().map(file_label),
            from_path: i.from.to_string_lossy().into_owned(),
            to_path: i.to.map(|p| p.to_string_lossy().into_owned()),
        })
        .collect();
    let pile_name = if is_trash { "Корзина".to_string() } else { pile_name };
    Ok(MoveView { id: move_id, at, method, pile_id, pile_name, is_trash, state, error, items })
}

fn roll_back_item(item: &Item, rename: RenameFn) -> AppResult<()> {
    match &item.to {
        None => {
            if item.from.exists() {
                Ok(())
            } else {
                trash_restore(&item.from)
            }
        }
        Some(to) => {
            let _ = fs::remove_file(part_path(to));
            match (item.from.exists(), to.exists()) {
                (true, true) => fs::remove_file(to).map_err(|e| io_error(&e, to)),
                (false, true) => transfer(to, &item.from, rename, || Ok(())),
                _ => Ok(()),
            }
        }
    }
}

pub fn place(conn: &mut Connection, card_ids: &[i64], pile_id: i64, method: &str) -> AppResult<MoveView> {
    place_with(conn, card_ids, pile_id, method, safe_rename)
}

pub fn place_with(
    conn: &mut Connection,
    card_ids: &[i64],
    pile_id: i64,
    method: &str,
    rename: RenameFn,
) -> AppResult<MoveView> {
    move_with(conn, card_ids, &[], pile_id, method, None, rename)
}

pub fn move_with(
    conn: &mut Connection,
    card_ids: &[i64],
    paths: &[PathBuf],
    pile_id: i64,
    method: &str,
    group: Option<i64>,
    rename: RenameFn,
) -> AppResult<MoveView> {
    move_into(conn, card_ids, paths, pile_id, Path::new(""), method, group, rename)
}

#[allow(clippy::too_many_arguments)]
fn move_into(
    conn: &mut Connection,
    card_ids: &[i64],
    paths: &[PathBuf],
    pile_id: i64,
    sub: &Path,
    method: &str,
    group: Option<i64>,
    rename: RenameFn,
) -> AppResult<MoveView> {
    if card_ids.is_empty() && paths.is_empty() {
        return Err(AppError::new("NOTHING", "Нечего раскладывать — отметь хотя бы одну карту."));
    }
    let pile = load_pile(conn, pile_id)?;
    if !pile.is_trash && !pile.exists {
        return Err(AppError::new("PILE_GONE", format!("Папки стопки «{}» больше нет — её удалили или переименовали вне Sorter. Создай стопку заново.", pile.name)));
    }
    let mut dir = Path::new(&pile.table_path).join(&pile.name);
    if !sub.as_os_str().is_empty() {
        dir.push(sub);
    }
    let mut items = Vec::with_capacity(card_ids.len());
    let mut taken: Vec<String> = Vec::new();
    for &card_id in card_ids {
        let (deck_path, file_name, status): (String, String, String) = conn
            .query_row(
                "SELECT deck_path, file_name, status FROM card WHERE id = ?1",
                params![card_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?
            .ok_or_else(|| AppError::new("NO_CARD", "Этой карты уже нет в колоде — её файл убрали. Колода обновится сама."))?;
        if status != "in_deck" && status != "deferred" {
            return Err(AppError::new("NOT_IN_DECK", format!("«{file_name}» уже не в колоде — его переместили вне Sorter. Колода обновится сама.")));
        }
        let to = if pile.is_trash {
            None
        } else {
            let name = free_name(&dir, &file_name, &taken);
            taken.push(name.clone());
            Some(dir.join(name))
        };
        items.push(Item { id: 0, card_id: Some(card_id), from: Path::new(&deck_path).join(&file_name), to, step: "planned".into() });
    }
    for path in paths {
        if !path.is_file() {
            return Err(AppError::new("FILE_NOT_THERE", format!("Файла «{}» уже нет на месте.", file_label(path))));
        }
        let to = if pile.is_trash {
            None
        } else {
            let name = free_name(&dir, &file_label(path), &taken);
            taken.push(name.clone());
            Some(dir.join(name))
        };
        items.push(Item { id: 0, card_id: None, from: path.clone(), to, step: "planned".into() });
    }

    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO move(at, method, pile_id, state, group_id) VALUES (?1, ?2, ?3, 'pending', ?4)",
        params![now_ms(), method, pile_id, group],
    )?;
    let move_id = tx.last_insert_rowid();
    for it in items.iter_mut() {
        let cross = it.to.as_deref().map(|to| !same_root(&it.from, to)).unwrap_or(false);
        tx.execute(
            "INSERT INTO move_item(move_id, card_id, from_path, to_path, cross_volume, step) VALUES (?1, ?2, ?3, ?4, ?5, 'planned')",
            params![
                move_id,
                it.card_id,
                it.from.to_string_lossy(),
                it.to.as_ref().map(|p| p.to_string_lossy().into_owned()),
                cross
            ],
        )?;
        it.id = tx.last_insert_rowid();
    }
    tx.commit()?;
    let conn: &Connection = conn;

    let mut failure = None;
    for idx in 0..items.len() {
        let (item_id, from, to) = (items[idx].id, items[idx].from.clone(), items[idx].to.clone());
        let result = match &to {
            Some(to) => transfer(&from, to, rename, || {
                set_step(conn, item_id, "copied")?;
                conn.execute("UPDATE move_item SET cross_volume = 1 WHERE id = ?1", params![item_id])?;
                Ok(())
            }),
            None => retry(|| trash::delete(&from).map_err(io::Error::other))
                .map_err(|e| AppError::new("TRASH", format!("Не получилось отправить «{}» в корзину ({e}). Проверь, что корзина включена для этого диска, и попробуй снова.", file_label(&from)))),
        };
        match result {
            Ok(()) => {
                set_step(conn, item_id, "done")?;
                items[idx].step = "done".into();
            }
            Err(e) => {
                failure = Some((idx, e));
                break;
            }
        }
    }

    if let Some((failed_at, err)) = failure {
        let mut note = err.message.clone();
        for item in items[..=failed_at].iter().rev() {
            if let Err(back) = roll_back_item(item, rename) {
                note.push_str(&format!(" Вернуть «{}» не вышло: {}", file_label(&item.from), back.message));
            }
        }
        set_state(conn, move_id, "failed", Some(&note))?;
        return Err(AppError::new(&err.code, note));
    }

    for it in &items {
        if let Some(card_id) = it.card_id {
            conn.execute(
                "UPDATE card SET status = 'placed', pile_id = ?2, current_path = ?3 WHERE id = ?1",
                params![card_id, pile_id, it.to.as_ref().map(|p| p.to_string_lossy().into_owned())],
            )?;
        }
        if let Some(to) = &it.to {
            crate::dupes::repath(conn, &it.from, to)?;
        }
    }
    set_state(conn, move_id, "done", None)?;
    load_move(conn, move_id)
}

fn table_path(conn: &Connection) -> AppResult<String> {
    Ok(crate::db::get_setting(conn, "table_path")?.unwrap_or_default())
}

fn trash_pile(conn: &Connection) -> AppResult<i64> {
    conn.query_row("SELECT id FROM pile WHERE table_path = ?1 AND is_trash = 1", params![table_path(conn)?], |r| r.get(0))
        .optional()?
        .ok_or_else(|| AppError::new("PILE_GONE", "Корзины нет — открой стол заново."))
}

fn deck_card(conn: &Connection, path: &Path) -> AppResult<Option<i64>> {
    let (Some(dir), Some(name)) = (path.parent(), path.file_name()) else { return Ok(None) };
    Ok(conn
        .query_row(
            "SELECT id FROM card WHERE deck_path = ?1 AND file_name = ?2 AND status IN ('in_deck','deferred')",
            params![dir.to_string_lossy(), name.to_string_lossy()],
            |r| r.get(0),
        )
        .optional()?)
}

pub fn trash_copies(conn: &mut Connection, paths: &[PathBuf]) -> AppResult<MoveView> {
    trash_copies_with(conn, paths, safe_rename)
}

pub fn trash_copies_with(conn: &mut Connection, paths: &[PathBuf], rename: RenameFn) -> AppResult<MoveView> {
    let trash = trash_pile(conn)?;
    let (mut cards, mut files) = (Vec::new(), Vec::new());
    for p in paths {
        match deck_card(conn, p)? {
            Some(id) => cards.push(id),
            None => files.push(p.clone()),
        }
    }
    move_with(conn, &cards, &files, trash, "key", None, rename)
}

pub fn replace_copy(conn: &mut Connection, card_id: i64, worse: &Path) -> AppResult<Vec<MoveView>> {
    replace_copy_with(conn, card_id, worse, safe_rename)
}

pub fn replace_copy_with(conn: &mut Connection, card_id: i64, worse: &Path, rename: RenameFn) -> AppResult<Vec<MoveView>> {
    let table = table_path(conn)?;
    let not_in_pile = || AppError::new("PILE_GONE", format!("«{}» лежит не в стопке — заменять нечего.", file_label(worse)));
    let (name, _) = crate::dupes::pile_place(worse, &table).ok_or_else(not_in_pile)?;
    let sub = worse.parent().and_then(|d| d.strip_prefix(Path::new(&table).join(&name)).ok()).map(Path::to_path_buf).unwrap_or_default();
    let pile: i64 = conn
        .query_row("SELECT id FROM pile WHERE table_path = ?1 AND name = ?2 AND is_trash = 0", params![table, name], |r| r.get(0))
        .optional()?
        .ok_or_else(not_in_pile)?;
    let trash = trash_pile(conn)?;
    let first = move_with(conn, &[], &[worse.to_path_buf()], trash, "key", None, rename)?;
    conn.execute("UPDATE move SET group_id = ?1 WHERE id = ?1", params![first.id])?;
    match move_into(conn, &[card_id], &[], pile, &sub, "key", Some(first.id), rename) {
        Ok(second) => Ok(vec![first, second]),
        Err(e) => {
            let _ = undo_with(conn, first.id, rename);
            Err(e)
        }
    }
}

pub fn place_series(conn: &mut Connection, members: &[i64], keep: &[i64], pile_id: i64, trash_rest: bool, method: &str) -> AppResult<MoveView> {
    place_series_with(conn, members, keep, pile_id, trash_rest, method, safe_rename)
}

pub fn place_series_with(
    conn: &mut Connection,
    members: &[i64],
    keep: &[i64],
    pile_id: i64,
    trash_rest: bool,
    method: &str,
    rename: RenameFn,
) -> AppResult<MoveView> {
    if members.is_empty() {
        return Err(AppError::new("NO_CARD", "Этой серии уже нет в колоде — колода обновится сама."));
    }
    let chosen: Vec<i64> = if keep.is_empty() { members.to_vec() } else { members.iter().copied().filter(|id| keep.contains(id)).collect() };
    let rest: Vec<i64> = members.iter().copied().filter(|id| !chosen.contains(id)).collect();
    let first = move_with(conn, &chosen, &[], pile_id, method, None, rename)?;
    if !trash_rest {
        for id in &rest {
            conn.execute("UPDATE card SET solo = 1, series_id = NULL WHERE id = ?1", params![id])?;
        }
    }
    if !trash_rest || rest.is_empty() {
        return Ok(first);
    }
    let trash = trash_pile(conn)?;
    conn.execute("UPDATE move SET group_id = ?1 WHERE id = ?1", params![first.id])?;
    match move_with(conn, &rest, &[], trash, method, Some(first.id), rename) {
        Ok(_) => load_move(conn, first.id),
        Err(e) => {
            let _ = undo_with(conn, first.id, rename);
            Err(e)
        }
    }
}

fn same_root(a: &Path, b: &Path) -> bool {
    let root = |p: &Path| p.components().next().map(|c| c.as_os_str().to_string_lossy().to_lowercase());
    root(a) == root(b)
}

fn return_cards(conn: &Connection, move_id: i64, items: &[Item]) -> AppResult<()> {
    for it in items {
        if let Some(to) = &it.to {
            crate::dupes::repath(conn, to, &it.from)?;
        }
    }
    let cards: Vec<i64> = items.iter().filter_map(|it| it.card_id).collect();
    if let Some(first) = cards.first() {
        let top: f64 = conn.query_row(
            "SELECT COALESCE(MIN(c.position), 0) FROM card c
             WHERE c.status IN ('in_deck','deferred') AND c.deck_path = (SELECT deck_path FROM card WHERE id = ?1)",
            params![first],
            |r| r.get(0),
        )?;
        for (i, id) in cards.iter().enumerate() {
            conn.execute(
                "UPDATE card SET status = 'gone' WHERE id != ?1 AND status IN ('in_deck','deferred')
                   AND deck_path = (SELECT deck_path FROM card WHERE id = ?1)
                   AND file_name = (SELECT file_name FROM card WHERE id = ?1) COLLATE NOCASE",
                params![id],
            )?;
            conn.execute(
                "UPDATE card SET status = 'in_deck', pile_id = NULL, current_path = NULL, position = ?2,
                 stage = CASE WHEN stage = 'meta' THEN 'new' ELSE stage END WHERE id = ?1",
                params![id, top - (cards.len() - i) as f64],
            )?;
            conn.execute("DELETE FROM example WHERE card_id = ?1", params![id])?;
        }
    }
    set_state(conn, move_id, "undone", None)
}

pub fn undo(conn: &Connection, move_id: i64) -> AppResult<MoveView> {
    undo_with(conn, move_id, safe_rename)
}

pub fn undo_with(conn: &Connection, move_id: i64, rename: RenameFn) -> AppResult<MoveView> {
    let state: String = conn.query_row("SELECT state FROM move WHERE id = ?1", params![move_id], |r| r.get(0))?;
    if state != "done" && state != "undoing" {
        return Err(AppError::new("NOT_UNDOABLE", "Этот ход уже забран или не состоялся."));
    }
    let items = load_items(conn, move_id)?;
    if state == "done" {
        for it in &items {
            if let Some(to) = &it.to {
                if !to.exists() {
                    return Err(AppError::new(
                        "FILE_NOT_THERE",
                        format!("Файла «{}» нет в стопке — его переместили или удалили, забрать ход нельзя.", file_label(to)),
                    ));
                }
            }
            if it.from.exists() {
                return Err(AppError::new(
                    "FILE_IN_THE_WAY",
                    format!("В колоде уже лежит файл «{}» — убери его и попробуй снова.", file_label(&it.from)),
                ));
            }
        }
        set_state(conn, move_id, "undoing", None)?;
    }
    for it in &items {
        if it.step == "restored" {
            continue;
        }
        set_step(conn, it.id, "restoring")?;
        match &it.to {
            None => {
                if !it.from.exists() {
                    trash_restore(&it.from)?;
                }
            }
            Some(to) => {
                if to.exists() && !it.from.exists() {
                    transfer(to, &it.from, rename, || Ok(()))?;
                }
            }
        }
        set_step(conn, it.id, "restored")?;
    }
    return_cards(conn, move_id, &items)?;
    load_move(conn, move_id)
}

pub fn undo_group(conn: &Connection, move_id: i64) -> AppResult<Vec<MoveView>> {
    let group: Option<i64> = conn.query_row("SELECT group_id FROM move WHERE id = ?1", params![move_id], |r| r.get(0))?;
    let ids: Vec<i64> = match group {
        Some(g) => conn
            .prepare("SELECT id FROM move WHERE group_id = ?1 AND state IN ('done','undoing') ORDER BY id DESC")?
            .query_map(params![g], |r| r.get(0))?
            .collect::<Result<_, _>>()?,
        None => vec![move_id],
    };
    ids.into_iter().map(|id| undo(conn, id)).collect()
}

pub fn undo_last(conn: &Connection) -> AppResult<Vec<MoveView>> {
    let last: Option<i64> = conn
        .query_row("SELECT id FROM move WHERE state = 'done' ORDER BY id DESC LIMIT 1", [], |r| r.get(0))
        .optional()?;
    Ok(match last {
        Some(id) => undo_group(conn, id)?,
        None => Vec::new(),
    })
}

pub fn undo_since(conn: &Connection, since: i64) -> AppResult<(usize, Vec<String>)> {
    let ids: Vec<i64> = conn
        .prepare("SELECT id FROM move WHERE state = 'done' AND at >= ?1 ORDER BY id DESC")?
        .query_map(params![since], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    let mut undone = 0;
    let mut failed = Vec::new();
    for id in ids {
        match undo(conn, id) {
            Ok(_) => undone += 1,
            Err(e) => failed.push(e.message),
        }
    }
    Ok((undone, failed))
}

pub fn recover(conn: &Connection) -> AppResult<usize> {
    recover_with(conn, safe_rename)
}

pub fn recover_with(conn: &Connection, rename: RenameFn) -> AppResult<usize> {
    let pending: Vec<(i64, String)> = conn
        .prepare("SELECT id, state FROM move WHERE state IN ('pending','undoing') ORDER BY id")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    for (id, state) in &pending {
        if state == "undoing" {
            undo_with(conn, *id, rename)?;
            continue;
        }
        let items = load_items(conn, *id)?;
        let mut note = String::from("Ход прервался — файлы возвращены в колоду.");
        for it in items.iter().rev() {
            if let Err(e) = roll_back_item(it, rename) {
                note.push_str(&format!(" «{}»: {}", file_label(&it.from), e.message));
            }
        }
        set_state(conn, *id, "failed", Some(&note))?;
    }
    Ok(pending.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use tempfile::TempDir;

    struct Env {
        _tmp: TempDir,
        deck: PathBuf,
        table: PathBuf,
        conn: Connection,
        pile: i64,
    }

    fn env() -> Env {
        let tmp = TempDir::new().unwrap();
        let deck = tmp.path().join("deck");
        let table = tmp.path().join("table");
        fs::create_dir_all(&deck).unwrap();
        fs::create_dir_all(table.join("Мемы")).unwrap();
        let conn = Connection::open_in_memory().unwrap();
        db::init(&conn).unwrap();
        conn.execute(
            "INSERT INTO pile(table_path, name, key, ord) VALUES (?1, 'Мемы', '1', 0)",
            params![table.to_string_lossy()],
        )
        .unwrap();
        let pile = conn.last_insert_rowid();
        Env { _tmp: tmp, deck, table, conn, pile }
    }

    fn card(env: &Env, name: &str, body: &[u8]) -> i64 {
        fs::write(env.deck.join(name), body).unwrap();
        env.conn
            .execute(
                "INSERT INTO card(deck_path, file_name, size, mtime, position) VALUES (?1, ?2, ?3, 0, (SELECT COUNT(*) FROM card))",
                params![env.deck.to_string_lossy(), name, body.len() as i64],
            )
            .unwrap();
        env.conn.last_insert_rowid()
    }

    fn status(env: &Env, id: i64) -> String {
        env.conn.query_row("SELECT status FROM card WHERE id = ?1", params![id], |r| r.get(0)).unwrap()
    }

    fn cross(_: &Path, _: &Path) -> io::Result<()> {
        Err(io::Error::from_raw_os_error(NOT_SAME_DEVICE))
    }

    #[test]
    fn places_and_undoes() {
        let mut e = env();
        let c = card(&e, "a.mp4", b"aaa");
        let m = place(&mut e.conn, &[c], e.pile, "key").unwrap();
        assert!(e.table.join("Мемы/a.mp4").exists());
        assert!(!e.deck.join("a.mp4").exists());
        assert_eq!(status(&e, c), "placed");
        undo(&e.conn, m.id).unwrap();
        assert!(e.deck.join("a.mp4").exists());
        assert!(!e.table.join("Мемы/a.mp4").exists());
        assert_eq!(status(&e, c), "in_deck");
    }

    fn with_trash(e: &Env) -> i64 {
        db::set_setting(&e.conn, "table_path", &e.table.to_string_lossy()).unwrap();
        e.conn
            .execute(
                "INSERT INTO pile(table_path, name, key, ord, is_trash) VALUES (?1, 'Корзина', NULL, 99, 1)",
                params![e.table.to_string_lossy()],
            )
            .unwrap();
        e.conn.last_insert_rowid()
    }

    #[test]
    fn trash_copies_takes_cards_and_pile_files() {
        let mut e = env();
        with_trash(&e);
        let c = card(&e, "a.mp4", b"aaa");
        let file = e.table.join("Мемы").join("b.mp4");
        fs::write(&file, b"bbb").unwrap();
        let m = trash_copies_with(&mut e.conn, &[e.deck.join("a.mp4"), file.clone()], cross_free).unwrap();
        assert_eq!(m.items.iter().map(|i| i.card_id).collect::<Vec<_>>(), vec![Some(c), None]);
        assert_eq!(status(&e, c), "placed");
        assert!(!file.exists());
    }

    #[test]
    fn trash_copy_from_outside_folder_comes_back_on_undo() {
        let mut e = env();
        with_trash(&e);
        let ext = e.table.parent().unwrap().join("ext");
        fs::create_dir_all(&ext).unwrap();
        let file = ext.join("1525005239349.mp4");
        fs::write(&file, b"copy").unwrap();
        let m = trash_copies_with(&mut e.conn, std::slice::from_ref(&file), cross_free).unwrap();
        assert_eq!((m.items.len(), m.items[0].card_id), (1, None));
        assert!(!file.exists());
        let logged: i64 = e.conn.query_row("SELECT COUNT(*) FROM move", [], |r| r.get(0)).unwrap();
        assert_eq!(logged, 1);
        let undone = undo_last(&e.conn).unwrap();
        assert_eq!(undone.len(), 1);
        assert_eq!(fs::read(&file).unwrap(), b"copy");
    }

    fn cross_free(from: &Path, to: &Path) -> io::Result<()> {
        fs::rename(from, to)
    }

    #[test]
    fn replace_puts_card_in_place_of_worse_and_undoes_both() {
        let mut e = env();
        with_trash(&e);
        let c = card(&e, "best.mp4", b"good");
        let worse = e.table.join("Мемы").join("worse.mp4");
        fs::write(&worse, b"bad").unwrap();
        let ms = replace_copy_with(&mut e.conn, c, &worse, cross_free).unwrap();
        assert_eq!(ms.len(), 2);
        assert!(e.table.join("Мемы").join("best.mp4").exists());
        assert!(!worse.exists());
        let groups: Vec<Option<i64>> =
            e.conn.prepare("SELECT group_id FROM move ORDER BY id").unwrap().query_map([], |r| r.get(0)).unwrap().map(|r| r.unwrap()).collect();
        assert_eq!(groups, vec![Some(ms[0].id), Some(ms[0].id)]);
        let undone = undo_last(&e.conn).unwrap();
        assert_eq!(undone.len(), 2);
        assert!(e.deck.join("best.mp4").exists());
        assert_eq!(status(&e, c), "in_deck");
        assert!(worse.exists());
    }

    #[test]
    fn replace_keeps_the_subfolder_of_worse() {
        let mut e = env();
        with_trash(&e);
        let c = card(&e, "best.mp4", b"good");
        let dir = e.table.join("Мемы").join("2019").join("лето");
        fs::create_dir_all(&dir).unwrap();
        let worse = dir.join("worse.mp4");
        fs::write(&worse, b"bad").unwrap();
        replace_copy_with(&mut e.conn, c, &worse, cross_free).unwrap();
        assert!(dir.join("best.mp4").exists());
        assert!(!worse.exists());
        undo_last(&e.conn).unwrap();
        assert!(e.deck.join("best.mp4").exists() && worse.exists());
    }

    #[test]
    fn series_places_marked_trashes_rest_and_undoes_as_one() {
        let mut e = env();
        with_trash(&e);
        let ids: Vec<i64> = (1..=5).map(|i| card(&e, &format!("s{i}.jpg"), b"x")).collect();
        let m = place_series_with(&mut e.conn, &ids, &[ids[1], ids[3]], e.pile, true, "key", cross_free).unwrap();
        assert_eq!(m.items.len(), 2);
        assert!(e.table.join("Мемы/s2.jpg").exists() && e.table.join("Мемы/s4.jpg").exists());
        assert!(ids.iter().all(|id| status(&e, *id) == "placed"));
        let undone = undo_last(&e.conn).unwrap();
        assert_eq!(undone.len(), 2);
        assert!(ids.iter().all(|id| status(&e, *id) == "in_deck"));
        assert!((1..=5).all(|i| e.deck.join(format!("s{i}.jpg")).exists()));

        let m = place_series_with(&mut e.conn, &ids, &[ids[0]], e.pile, false, "key", cross_free).unwrap();
        assert_eq!(m.items.len(), 1);
        assert_eq!((status(&e, ids[0]).as_str(), status(&e, ids[1]).as_str()), ("placed", "in_deck"));
        let solo: i64 = e.conn.query_row("SELECT solo FROM card WHERE id = ?1", params![ids[1]], |r| r.get(0)).unwrap();
        assert_eq!(solo, 1);
        undo_last(&e.conn).unwrap();
        let all = place_series_with(&mut e.conn, &ids, &[], e.pile, true, "key", cross_free).unwrap();
        assert_eq!(all.items.len(), 5);
    }

    #[test]
    fn undo_survives_twin_card_from_watcher() {
        let mut e = env();
        let c = card(&e, "a.jpg", b"a");
        let m = place(&mut e.conn, &[c], e.pile, "key").unwrap();
        e.conn
            .execute(
                "INSERT INTO card(deck_path, file_name, size, mtime, position) VALUES (?1, 'A.jpg', 1, 0, 99)",
                params![e.deck.to_string_lossy()],
            )
            .unwrap();
        undo(&e.conn, m.id).unwrap();
        assert_eq!(status(&e, c), "in_deck");
        let twins: i64 = e.conn.query_row("SELECT COUNT(*) FROM card WHERE status = 'in_deck'", [], |r| r.get(0)).unwrap();
        assert_eq!(twins, 1);
    }

    #[test]
    fn dupes_follow_the_file() {
        let mut e = env();
        let c = card(&e, "a.mp4", b"aaa");
        let (a, z) = (e.deck.join("a.mp4").to_string_lossy().into_owned(), e.deck.join("z.mp4").to_string_lossy().into_owned());
        e.conn.execute("INSERT INTO fingerprint(path, size, mtime, state) VALUES (?1, 3, 0, 'ok')", params![a]).unwrap();
        e.conn
            .execute("INSERT INTO dupe(a, b, kind, confidence, offset_ms) VALUES (?1, ?2, 'same', 90, 500)", params![a, z])
            .unwrap();
        let pair = |e: &Env| -> (String, String, i64) {
            e.conn.query_row("SELECT a, b, offset_ms FROM dupe", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap()
        };
        let m = place(&mut e.conn, &[c], e.pile, "key").unwrap();
        let moved = e.table.join("Мемы").join("a.mp4").to_string_lossy().into_owned();
        let (pa, pb, off) = pair(&e);
        assert_eq!((pa.as_str(), pb.as_str(), off), (z.as_str(), moved.as_str(), -500));
        let path: String = e.conn.query_row("SELECT path FROM fingerprint", [], |r| r.get(0)).unwrap();
        assert_eq!(path, moved);
        undo(&e.conn, m.id).unwrap();
        assert_eq!(pair(&e), (a, z, 500));
    }

    #[test]
    fn never_overwrites() {
        let mut e = env();
        fs::write(e.table.join("Мемы/a.mp4"), b"old").unwrap();
        let c = card(&e, "a.mp4", b"new");
        let m = place(&mut e.conn, &[c], e.pile, "key").unwrap();
        assert_eq!(m.items[0].final_name.as_deref(), Some("a (2).mp4"));
        assert_eq!(fs::read(e.table.join("Мемы/a.mp4")).unwrap(), b"old");
        assert_eq!(fs::read(e.table.join("Мемы/a (2).mp4")).unwrap(), b"new");
    }

    #[test]
    fn batch_names_do_not_collide() {
        assert_eq!(free_name(Path::new("Z:/nope"), "a.mp4", &["A.mp4".into()]), "a (2).mp4");
    }

    #[test]
    fn failed_batch_rolls_back() {
        let mut e = env();
        let a = card(&e, "a.mp4", b"a");
        let b = card(&e, "b.mp4", b"b");
        let c = card(&e, "c.mp4", b"c");
        fs::remove_file(e.deck.join("c.mp4")).unwrap();
        let err = place(&mut e.conn, &[a, b, c], e.pile, "table").unwrap_err();
        assert_eq!(err.code, "FILE_NOT_THERE");
        assert!(e.deck.join("a.mp4").exists() && e.deck.join("b.mp4").exists());
        assert!(!e.table.join("Мемы/a.mp4").exists());
        assert_eq!(status(&e, a), "in_deck");
        let state: String = e.conn.query_row("SELECT state FROM move", [], |r| r.get(0)).unwrap();
        assert_eq!(state, "failed");
    }

    #[test]
    fn cross_volume_copies_verifies_and_undoes() {
        let mut e = env();
        let c = card(&e, "a.mp4", b"payload");
        let m = place_with(&mut e.conn, &[c], e.pile, "key", cross).unwrap();
        assert_eq!(fs::read(e.table.join("Мемы/a.mp4")).unwrap(), b"payload");
        assert!(!e.deck.join("a.mp4").exists());
        assert!(!e.table.join("Мемы/.a.mp4.sorter-part").exists());
        undo_with(&e.conn, m.id, cross).unwrap();
        assert_eq!(fs::read(e.deck.join("a.mp4")).unwrap(), b"payload");
        assert!(!e.table.join("Мемы/a.mp4").exists());
    }

    #[test]
    fn undo_refuses_when_file_gone() {
        let mut e = env();
        let c = card(&e, "a.mp4", b"a");
        let m = place(&mut e.conn, &[c], e.pile, "key").unwrap();
        fs::remove_file(e.table.join("Мемы/a.mp4")).unwrap();
        assert_eq!(undo(&e.conn, m.id).unwrap_err().code, "FILE_NOT_THERE");
        assert_eq!(status(&e, c), "placed");
    }

    #[test]
    fn undo_last_returns_card_to_top() {
        let mut e = env();
        let a = card(&e, "a.mp4", b"a");
        let b = card(&e, "b.mp4", b"b");
        place(&mut e.conn, &[b], e.pile, "key").unwrap();
        undo_last(&e.conn).unwrap();
        let top: i64 = e
            .conn
            .query_row("SELECT id FROM card WHERE status = 'in_deck' ORDER BY position LIMIT 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(top, b);
        assert_ne!(top, a);
    }

    fn pending_move(e: &Env, c: i64, step: &str) -> i64 {
        e.conn
            .execute("INSERT INTO move(at, method, pile_id, state) VALUES (0, 'key', ?1, 'pending')", params![e.pile])
            .unwrap();
        let id = e.conn.last_insert_rowid();
        e.conn
            .execute(
                "INSERT INTO move_item(move_id, card_id, from_path, to_path, step) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    id,
                    c,
                    e.deck.join("a.mp4").to_string_lossy(),
                    e.table.join("Мемы/a.mp4").to_string_lossy(),
                    step
                ],
            )
            .unwrap();
        id
    }

    #[test]
    fn recover_planned_untouched() {
        let e = env();
        let c = card(&e, "a.mp4", b"a");
        pending_move(&e, c, "planned");
        assert_eq!(recover(&e.conn).unwrap(), 1);
        assert!(e.deck.join("a.mp4").exists());
        assert_eq!(status(&e, c), "in_deck");
    }

    #[test]
    fn recover_planned_after_rename() {
        let e = env();
        let c = card(&e, "a.mp4", b"a");
        pending_move(&e, c, "planned");
        fs::rename(e.deck.join("a.mp4"), e.table.join("Мемы/a.mp4")).unwrap();
        recover(&e.conn).unwrap();
        assert!(e.deck.join("a.mp4").exists());
        assert!(!e.table.join("Мемы/a.mp4").exists());
    }

    #[test]
    fn recover_copied_removes_copy() {
        let e = env();
        let c = card(&e, "a.mp4", b"a");
        pending_move(&e, c, "copied");
        fs::copy(e.deck.join("a.mp4"), e.table.join("Мемы/a.mp4")).unwrap();
        fs::write(e.table.join("Мемы/.a.mp4.sorter-part"), b"half").unwrap();
        recover(&e.conn).unwrap();
        assert!(e.deck.join("a.mp4").exists());
        assert!(!e.table.join("Мемы/a.mp4").exists());
        assert!(!e.table.join("Мемы/.a.mp4.sorter-part").exists());
    }

    #[test]
    fn recover_finishes_undoing() {
        let mut e = env();
        let c = card(&e, "a.mp4", b"a");
        let m = place(&mut e.conn, &[c], e.pile, "key").unwrap();
        e.conn.execute("UPDATE move SET state = 'undoing' WHERE id = ?1", params![m.id]).unwrap();
        recover(&e.conn).unwrap();
        assert!(e.deck.join("a.mp4").exists());
        assert_eq!(status(&e, c), "in_deck");
    }

    #[test]
    fn pile_names() {
        assert!(validate_pile_name("Кино и сериалы").is_ok());
        assert_eq!(validate_pile_name("a/b").unwrap_err().code, "BAD_NAME");
        assert_eq!(validate_pile_name("con").unwrap_err().code, "BAD_NAME");
        assert_eq!(validate_pile_name("x.").unwrap_err().code, "BAD_NAME");
    }
}
