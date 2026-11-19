use crate::error::Result;
use rusqlite::{Connection, params};
use std::path::PathBuf;
use std::sync::Mutex;

pub struct Db {
    pub conn: Mutex<Connection>,
}

impl Db {
    pub fn open(path: PathBuf) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| crate::error::AppError::Io(e.to_string()))?;
        }
        let conn = Connection::open(&path).map_err(|e| crate::error::AppError::Db(e.to_string()))?;
        conn.execute_batch(
            r#"
            PRAGMA journal_mode = WAL;
            CREATE TABLE IF NOT EXISTS kv (key TEXT PRIMARY KEY, value TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS videos (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                channel_id TEXT,
                channel_title TEXT,
                description TEXT,
                thumbnail TEXT,
                duration TEXT,
                published_at TEXT,
                view_count INTEGER,
                cached_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS search_cache (
                query TEXT PRIMARY KEY,
                video_ids TEXT NOT NULL,
                cached_at INTEGER NOT NULL
            );
            "#,
        )
        .map_err(|e| crate::error::AppError::Db(e.to_string()))?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    pub fn get_kv(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT value FROM kv WHERE key = ?1").map_err(|e| crate::error::AppError::Db(e.to_string()))?;
        let mut rows = stmt.query(params![key]).map_err(|e| crate::error::AppError::Db(e.to_string()))?;
        if let Some(row) = rows.next().map_err(|e| crate::error::AppError::Db(e.to_string()))? {
            Ok(Some(row.get(0).map_err(|e| crate::error::AppError::Db(e.to_string()))?))
        } else {
            Ok(None)
        }
    }

    pub fn set_kv(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO kv(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            params![key, value],
        )
        .map_err(|e| crate::error::AppError::Db(e.to_string()))?;
        Ok(())
    }
}