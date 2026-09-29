use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

const SCHEMA_VERSION: i64 = 1;

pub fn open(path: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;
    init(&conn)?;
    Ok(conn)
}

pub fn init(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = FULL;
         PRAGMA foreign_keys = ON;
         PRAGMA busy_timeout = 5000;
         CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
         CREATE TABLE IF NOT EXISTS card (
           id INTEGER PRIMARY KEY,
           deck_path TEXT NOT NULL,
           file_name TEXT NOT NULL,
           size INTEGER NOT NULL,
           mtime INTEGER NOT NULL,
           taken_at INTEGER,
           duration_ms INTEGER,
           width INTEGER,
           height INTEGER,
           orientation TEXT CHECK (orientation IN ('portrait','landscape','square')),
           frames INTEGER NOT NULL DEFAULT 0,
           stage TEXT NOT NULL DEFAULT 'new' CHECK (stage IN ('new','meta','frames','embedded','broken')),
           error TEXT,
           status TEXT NOT NULL DEFAULT 'in_deck' CHECK (status IN ('in_deck','deferred','placed','gone')),
           position REAL NOT NULL,
           pile_id INTEGER REFERENCES pile(id),
           current_path TEXT
         );
         CREATE UNIQUE INDEX IF NOT EXISTS card_in_deck ON card(deck_path, file_name COLLATE NOCASE)
           WHERE status IN ('in_deck','deferred');
         CREATE INDEX IF NOT EXISTS card_order ON card(deck_path, status, position);
         CREATE TABLE IF NOT EXISTS pile (
           id INTEGER PRIMARY KEY,
           table_path TEXT NOT NULL,
           name TEXT NOT NULL,
           key TEXT,
           is_trash INTEGER NOT NULL DEFAULT 0,
           ord INTEGER NOT NULL,
           exists_on_disk INTEGER NOT NULL DEFAULT 1
         );
         CREATE UNIQUE INDEX IF NOT EXISTS pile_name ON pile(table_path, name COLLATE NOCASE);
         CREATE UNIQUE INDEX IF NOT EXISTS pile_key ON pile(table_path, key) WHERE key IS NOT NULL;
         CREATE UNIQUE INDEX IF NOT EXISTS pile_trash ON pile(table_path) WHERE is_trash = 1;
         CREATE TABLE IF NOT EXISTS move (
           id INTEGER PRIMARY KEY,
           at INTEGER NOT NULL,
           method TEXT NOT NULL CHECK (method IN ('key','hint','search','table','drag','new_pile')),
           pile_id INTEGER NOT NULL REFERENCES pile(id),
           state TEXT NOT NULL CHECK (state IN ('pending','done','undoing','undone','failed')),
           error TEXT
         );
         CREATE TABLE IF NOT EXISTS move_item (
           move_id INTEGER NOT NULL REFERENCES move(id),
           card_id INTEGER NOT NULL REFERENCES card(id),
           from_path TEXT NOT NULL,
           to_path TEXT,
           cross_volume INTEGER NOT NULL DEFAULT 0,
           step TEXT NOT NULL CHECK (step IN ('planned','copied','done','restoring','restored')),
           PRIMARY KEY (move_id, card_id)
         );
         CREATE TABLE IF NOT EXISTS example (
           id INTEGER PRIMARY KEY,
           card_id INTEGER REFERENCES card(id),
           pile_id INTEGER NOT NULL REFERENCES pile(id),
           path TEXT NOT NULL,
           vector BLOB NOT NULL
         );
         CREATE INDEX IF NOT EXISTS example_pile ON example(pile_id);
         CREATE TABLE IF NOT EXISTS fingerprint (
           path TEXT PRIMARY KEY,
           size INTEGER NOT NULL,
           mtime INTEGER NOT NULL,
           sha256 BLOB,
           duration_ms INTEGER,
           width INTEGER,
           height INTEGER,
           bitrate INTEGER,
           box TEXT,
           frames BLOB,
           audio BLOB,
           semantic BLOB,
           mean BLOB,
           state TEXT NOT NULL DEFAULT 'new' CHECK (state IN ('new','ok','failed'))
         );
         CREATE INDEX IF NOT EXISTS fingerprint_sha ON fingerprint(sha256);
         CREATE TABLE IF NOT EXISTS dupe (
           a TEXT NOT NULL,
           b TEXT NOT NULL,
           kind TEXT NOT NULL CHECK (kind IN ('exact','same','trim','crop')),
           confidence INTEGER NOT NULL,
           offset_ms INTEGER,
           visual REAL,
           audio REAL,
           semantic REAL,
           PRIMARY KEY (a, b)
         );
         CREATE INDEX IF NOT EXISTS dupe_b ON dupe(b);
         CREATE TABLE IF NOT EXISTS dupe_dismissed (
           a TEXT NOT NULL,
           b TEXT NOT NULL,
           PRIMARY KEY (a, b)
         );
         CREATE TABLE IF NOT EXISTS embedding (
           card_id INTEGER PRIMARY KEY REFERENCES card(id),
           vector BLOB NOT NULL
         );",
    )?;
    let has_group: bool = conn
        .prepare("SELECT 1 FROM pragma_table_info('move') WHERE name = 'group_id'")?
        .exists([])?;
    if !has_group {
        conn.execute("ALTER TABLE move ADD COLUMN group_id INTEGER", [])?;
    }
    conn.execute(
        "INSERT OR IGNORE INTO settings(key, value) VALUES ('schema_version', ?1)",
        params![SCHEMA_VERSION.to_string()],
    )?;
    Ok(())
}

pub fn get_setting(conn: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row("SELECT value FROM settings WHERE key = ?1", params![key], |r| r.get(0))
        .optional()
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO settings(key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
