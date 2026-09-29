use crate::db::{get_setting, set_setting};
use crate::deck::{self, CardView};
use crate::dupes::{self, DupeView};
use crate::error::{AppError, AppResult};
use crate::hints;
use crate::moves::{self, MoveView};
use crate::piles::{self, PileView};
use rusqlite::{params, Connection};
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use tauri::State;

pub struct AppState {
    pub db: Mutex<Connection>,
    pub data_dir: PathBuf,
    pub paused: Arc<AtomicBool>,
    pub wake: Arc<AtomicBool>,
    pub incoming: Mutex<Option<String>>,
}

const SETTING_KEYS: [&str; 8] = ["deck_path", "table_path", "hints_enabled", "theme", "muted", "mode", "volume", "updates"];

impl AppState {
    pub fn conn(&self) -> MutexGuard<'_, Connection> {
        self.db.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn roots(&self) -> Vec<PathBuf> {
        let conn = self.conn();
        let mut roots = vec![self.data_dir.join("cache")];
        for key in ["deck_path", "table_path"] {
            if let Ok(Some(p)) = get_setting(&conn, key) {
                roots.push(PathBuf::from(p));
            }
        }
        roots
    }
}

fn paths(conn: &Connection) -> AppResult<(String, String)> {
    let deck = get_setting(conn, "deck_path")?;
    let table = get_setting(conn, "table_path")?;
    match (deck, table) {
        (Some(d), Some(t)) => Ok((d, t)),
        _ => Err(AppError::new("NOT_READY", "Сначала выбери колоду и стол.")),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeckCounts {
    total: i64,
    left: i64,
    placed: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StateView {
    cache_dir: String,
    developing: DevelopView,
    settings: HashMap<String, String>,
    deck: DeckCounts,
    piles: Vec<PileView>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DevelopView {
    done: i64,
    total: i64,
    paused: bool,
}

fn snapshot(conn: &Connection, state: &AppState) -> AppResult<StateView> {
    let mut settings = HashMap::new();
    for key in SETTING_KEYS {
        if let Some(v) = get_setting(conn, key)? {
            settings.insert(key.to_string(), v);
        }
    }
    let (deck, piles) = match paths(conn) {
        Ok((d, t)) => {
            let (left, placed) = deck::counts(conn, &d)?;
            (DeckCounts { total: left + placed, left, placed }, piles::list(conn, &t)?)
        }
        Err(_) => (DeckCounts { total: 0, left: 0, placed: 0 }, Vec::new()),
    };
    let (done, total) = match get_setting(conn, "deck_path")? {
        Some(d) => conn.query_row(
            "SELECT COALESCE(SUM(stage != 'new'), 0), COUNT(*) FROM card WHERE deck_path = ?1 AND status IN ('in_deck','deferred')",
            params![d],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?,
        None => (0, 0),
    };
    Ok(StateView {
        cache_dir: state.data_dir.join("cache").to_string_lossy().into_owned(),
        developing: DevelopView { done, total, paused: state.paused.load(Ordering::Relaxed) },
        settings,
        deck,
        piles,
    })
}

#[tauri::command]
pub fn get_state(state: State<AppState>) -> AppResult<StateView> {
    let conn = state.conn();
    if let Ok((d, t)) = paths(&conn) {
        if Path::new(&d).is_dir() {
            deck::sync(&conn, &d)?;
        }
        if Path::new(&t).is_dir() {
            piles::sync(&conn, &t)?;
        }
    }
    state.wake.store(true, Ordering::Relaxed);
    snapshot(&conn, &state)
}

fn nested(a: &Path, b: &Path) -> bool {
    let a = format!("{}\\", a.to_string_lossy().to_lowercase().trim_end_matches('\\'));
    let b = format!("{}\\", b.to_string_lossy().to_lowercase().trim_end_matches('\\'));
    a.starts_with(&b) || b.starts_with(&a)
}

fn choose(conn: &Connection, key: &str, other: &str, path: &str) -> AppResult<()> {
    let p = Path::new(path);
    if !p.is_dir() {
        return Err(AppError::new("NO_FOLDER", "Такой папки нет — выбери другую."));
    }
    if let Some(o) = get_setting(conn, other)? {
        if nested(p, Path::new(&o)) {
            return Err(AppError::new(
                "NESTED",
                "Колода и стол не могут совпадать или лежать друг в друге — иначе Sorter начнёт раскладывать уже разложенное.",
            ));
        }
    }
    Ok(set_setting(conn, key, path)?)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Chosen {
    count: i64,
    bytes: i64,
}

#[tauri::command]
pub fn choose_deck(state: State<AppState>, path: String) -> AppResult<Chosen> {
    let conn = state.conn();
    choose(&conn, "deck_path", "table_path", &path)?;
    deck::sync(&conn, &path)?;
    Ok(conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(size), 0) FROM card WHERE deck_path = ?1 AND status IN ('in_deck','deferred')",
        params![path],
        |r| Ok(Chosen { count: r.get(0)?, bytes: r.get(1)? }),
    )?)
}

#[tauri::command]
pub fn choose_table(state: State<AppState>, path: String) -> AppResult<Vec<PileView>> {
    let conn = state.conn();
    choose(&conn, "table_path", "deck_path", &path)?;
    piles::sync(&conn, &path)?;
    piles::list(&conn, &path)
}

#[tauri::command]
pub fn set_setting_cmd(app: tauri::AppHandle, state: State<AppState>, key: String, value: String) -> AppResult<()> {
    if !["hints_enabled", "theme", "muted", "mode", "volume", "updates"].contains(&key.as_str()) {
        return Err(AppError::new("BAD_SETTING", "Такой настройки нет."));
    }
    set_setting(&state.conn(), &key, &value)?;
    if key == "updates" {
        crate::updates::toggled(&app);
    }
    Ok(())
}

#[tauri::command]
pub fn take_incoming(state: State<AppState>) -> Option<String> {
    state.incoming.lock().unwrap_or_else(|e| e.into_inner()).take()
}

pub fn folder_arg<I: IntoIterator<Item = String>>(args: I) -> Option<String> {
    let arg = args.into_iter().nth(1)?;
    let path = arg.trim().trim_matches('"').trim_end_matches(['\\', '/']);
    let path = if path.ends_with(':') { format!("{path}\\") } else { path.to_string() };
    Path::new(&path).is_dir().then_some(path)
}

#[tauri::command]
pub fn deck_window(state: State<AppState>, from: i64, count: i64) -> AppResult<Vec<CardView>> {
    let conn = state.conn();
    let (d, _) = paths(&conn)?;
    deck::window(&conn, &d, from, count)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Placed {
    #[serde(rename = "move")]
    mv: MoveView,
    piles: Vec<PileView>,
}

#[tauri::command]
pub fn place(state: State<AppState>, card_ids: Vec<i64>, pile_id: i64, method: String) -> AppResult<Placed> {
    let mut conn = state.conn();
    let mv = moves::place(&mut conn, &card_ids, pile_id, &method)?;
    hints::learn(&conn)?;
    let (_, t) = paths(&conn)?;
    Ok(Placed { mv, piles: piles::list(&conn, &t)? })
}

#[tauri::command]
pub fn defer(state: State<AppState>, card_id: i64) -> AppResult<()> {
    deck::defer(&state.conn(), card_id)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Undone {
    #[serde(rename = "move")]
    mv: Option<MoveView>,
    cards: Vec<CardView>,
    piles: Vec<PileView>,
}

fn undone(conn: &Connection, moves: Vec<MoveView>) -> AppResult<Undone> {
    hints::prune(conn)?;
    let ids: Vec<i64> = moves.iter().flat_map(|m| m.items.iter().filter_map(|i| i.card_id)).collect();
    let (_, t) = paths(conn)?;
    let mv = moves.iter().position(|m| m.items.iter().any(|i| i.card_id.is_some())).or((!moves.is_empty()).then_some(0));
    let mv = mv.and_then(|i| moves.into_iter().nth(i));
    Ok(Undone { cards: deck::cards(conn, &ids)?, piles: piles::list(conn, &t)?, mv })
}

#[tauri::command]
pub fn undo_last(state: State<AppState>) -> AppResult<Undone> {
    let conn = state.conn();
    let mv = moves::undo_last(&conn)?;
    undone(&conn, mv)
}

#[tauri::command]
pub fn undo_move(state: State<AppState>, move_id: i64) -> AppResult<Undone> {
    let conn = state.conn();
    let moves = moves::undo_group(&conn, move_id)?;
    undone(&conn, moves)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoneMany {
    undone: usize,
    failed: Vec<String>,
}

#[tauri::command]
pub fn undo_since(state: State<AppState>, since: i64) -> AppResult<UndoneMany> {
    let conn = state.conn();
    let (undone, failed) = moves::undo_since(&conn, since)?;
    hints::prune(&conn)?;
    Ok(UndoneMany { undone, failed })
}

#[tauri::command]
pub fn hints(state: State<AppState>, card_ids: Vec<i64>) -> AppResult<hints::HintsView> {
    Ok(hints::suggest(&state.conn(), &card_ids)?)
}

#[tauri::command]
pub fn journal(state: State<AppState>, before: Option<i64>, limit: i64) -> AppResult<Vec<MoveView>> {
    let conn = state.conn();
    let ids: Vec<i64> = conn
        .prepare("SELECT id FROM move WHERE id < ?1 ORDER BY id DESC LIMIT ?2")?
        .query_map(params![before.unwrap_or(i64::MAX), limit], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    ids.into_iter().map(|id| moves::load_move(&conn, id)).collect()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JournalStats {
    moves: i64,
    cards: i64,
    hinted: i64,
}

#[tauri::command]
pub fn journal_stats(state: State<AppState>) -> AppResult<JournalStats> {
    let conn = state.conn();
    let (moves, cards, hinted) = conn.query_row(
        "SELECT COUNT(DISTINCT m.id),
                COUNT(i.card_id),
                COUNT(CASE WHEN m.method = 'hint' THEN i.card_id END)
         FROM move m LEFT JOIN move_item i ON i.move_id = m.id
         WHERE m.state = 'done'",
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    Ok(JournalStats { moves, cards, hinted })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheInfo {
    bytes: u64,
    cards: usize,
}

#[tauri::command]
pub fn cache_info(state: State<AppState>) -> CacheInfo {
    let (bytes, cards) = crate::develop::cache_size(&state.data_dir.join("cache"));
    CacheInfo { bytes, cards }
}

#[tauri::command]
pub fn clear_cache(state: State<AppState>) -> AppResult<u64> {
    let freed = crate::develop::clear_cache(&state.conn(), &state.data_dir.join("cache"))?;
    state.wake.store(true, Ordering::Relaxed);
    Ok(freed)
}

#[tauri::command]
pub fn create_pile(state: State<AppState>, name: String) -> AppResult<Vec<PileView>> {
    let conn = state.conn();
    let (_, t) = paths(&conn)?;
    piles::create(&conn, &t, &name)?;
    piles::list(&conn, &t)
}

#[tauri::command]
pub fn rename_pile(state: State<AppState>, pile_id: i64, name: String) -> AppResult<Vec<PileView>> {
    let conn = state.conn();
    let (_, t) = paths(&conn)?;
    piles::rename(&conn, pile_id, &name)?;
    piles::list(&conn, &t)
}

#[tauri::command]
pub fn set_pile_key(state: State<AppState>, pile_id: i64, key: Option<String>) -> AppResult<Vec<PileView>> {
    let conn = state.conn();
    let (_, t) = paths(&conn)?;
    piles::set_key(&conn, pile_id, key.as_deref())?;
    piles::list(&conn, &t)
}

#[tauri::command]
pub fn develop_control(state: State<AppState>, pause: bool) -> AppResult<()> {
    state.paused.store(pause, Ordering::Relaxed);
    state.wake.store(true, Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
pub fn remove_pile(state: State<AppState>, pile_id: i64) -> AppResult<Vec<PileView>> {
    let conn = state.conn();
    let (_, t) = paths(&conn)?;
    piles::remove(&conn, pile_id)?;
    piles::list(&conn, &t)
}

#[tauri::command]
pub fn dupes_for(state: State<AppState>, card_id: i64) -> AppResult<Vec<DupeView>> {
    dupes::dupes_for(&state.conn(), card_id)
}

#[tauri::command]
pub fn dismiss_dupe(state: State<AppState>, a: String, b: String) -> AppResult<()> {
    dupes::dismiss(&state.conn(), &a, &b)
}

#[tauri::command]
pub fn trash_copies(state: State<AppState>, paths: Vec<String>) -> AppResult<Placed> {
    let mut conn = state.conn();
    let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    let mv = moves::trash_copies(&mut conn, &paths)?;
    let (_, t) = self::paths(&conn)?;
    Ok(Placed { mv, piles: piles::list(&conn, &t)? })
}

#[tauri::command]
pub fn replace_copy(state: State<AppState>, card_id: i64, worse_path: String) -> AppResult<Placed> {
    let mut conn = state.conn();
    let mv = moves::replace_copy(&mut conn, card_id, Path::new(&worse_path))?.pop().expect("два хода");
    hints::learn(&conn)?;
    let (_, t) = paths(&conn)?;
    Ok(Placed { mv, piles: piles::list(&conn, &t)? })
}

#[tauri::command]
pub fn dupe_groups(state: State<AppState>) -> AppResult<dupes::Groups> {
    dupes::groups(&state.conn())
}

#[tauri::command]
pub fn playable(state: State<AppState>, card_id: i64) -> AppResult<String> {
    let path = {
        let conn = state.conn();
        deck::cards(&conn, &[card_id])?.into_iter().next().map(|c| c.path)
    }
    .ok_or_else(|| AppError::new("NO_CARD", "Этой карты уже нет в колоде — её файл убрали. Колода обновится сама."))?;
    let dir = state.data_dir.join("cache").join(card_id.to_string());
    let dst = dir.join("play.mp4");
    if !dst.exists() {
        std::fs::create_dir_all(&dir).map_err(|e| AppError::new("CACHE", format!("Не получилось создать папку для копии видео ({e}). Проверь место на диске.")))?;
        crate::playable::remux(Path::new(&path), &dst).map_err(|e| AppError::new("NOT_PLAYABLE", format!("Не получилось подготовить видео: {e}")))?;
    }
    Ok(dst.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arg(a: &str) -> Option<String> {
        folder_arg(["sorter.exe".to_string(), a.to_string()])
    }

    #[test]
    fn folder_arg_takes_existing_folder_only() {
        let dir = std::env::temp_dir().join("sorter тест папка");
        std::fs::create_dir_all(&dir).unwrap();
        let plain = dir.to_string_lossy().into_owned();
        assert_eq!(arg(&plain).as_deref(), Some(plain.as_str()));
        assert_eq!(arg(&format!("\"{plain}\\\"")).as_deref(), Some(plain.as_str()));
        assert!(arg(&dir.join("нет такой").to_string_lossy()).is_none());
        assert!(folder_arg(["sorter.exe".to_string()]).is_none());
        let root = std::env::temp_dir().to_string_lossy()[..2].to_string();
        assert_eq!(arg(&format!("{root}\\")), Some(format!("{root}\\")));
    }
}
