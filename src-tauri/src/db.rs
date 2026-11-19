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
            CREATE TABLE IF NOT EXISTS history (
                video_id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                channel_title TEXT,
                thumbnail TEXT,
                watched_at INTEGER NOT NULL,
                progress REAL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS thumb_cache (
                url TEXT PRIMARY KEY,
                path TEXT NOT NULL,
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

    pub fn record_history(&self, video_id: &str, title: &str, channel: Option<&str>, thumb: Option<&str>, progress: f64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO history(video_id,title,channel_title,thumbnail,watched_at,progress) VALUES(?1,?2,?3,?4,?5,?6)
             ON CONFLICT(video_id) DO UPDATE SET title=excluded.title, channel_title=excluded.channel_title, thumbnail=excluded.thumbnail, watched_at=excluded.watched_at, progress=excluded.progress",
            params![video_id, title, channel, thumb, chrono_now(), progress],
        )
        .map_err(|e| crate::error::AppError::Db(e.to_string()))?;
        Ok(())
    }

    pub fn get_history(&self, limit: i64) -> Result<Vec<(String,String,Option<String>,Option<String>,i64,f64)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT video_id,title,channel_title,thumbnail,watched_at,progress FROM history ORDER BY watched_at DESC LIMIT ?1").map_err(|e| crate::error::AppError::Db(e.to_string()))?;
        let rows = stmt.query_map(params![limit], |r| Ok((
            r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?
        ))).map_err(|e| crate::error::AppError::Db(e.to_string()))?;
        let mut out = Vec::new();
        for r in rows { out.push(r.map_err(|e| crate::error::AppError::Db(e.to_string()))?); }
        Ok(out)
    }

    pub fn clear_history(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM history", []).map_err(|e| crate::error::AppError::Db(e.to_string()))?;
        Ok(())
    }
}

fn chrono_now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}