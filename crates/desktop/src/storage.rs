use core::storage::StorageProvider;
use rusqlite::Connection;
use std::path::Path;
use std::sync::Mutex;

pub struct SqliteStorage {
    conn: Mutex<Connection>,
}

impl SqliteStorage {
    pub fn new(db_path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(db_path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS plugin_kv (
                plugin_name TEXT NOT NULL,
                key TEXT NOT NULL,
                value TEXT NOT NULL DEFAULT '',
                PRIMARY KEY (plugin_name, key)
            )"
        )?;
        Ok(SqliteStorage { conn: Mutex::new(conn) })
    }
}

impl StorageProvider for SqliteStorage {
    fn get(&self, plugin: &str, key: &str) -> Option<String> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT value FROM plugin_kv WHERE plugin_name = ?1 AND key = ?2",
            [plugin, key],
            |row| row.get(0),
        ).ok()
    }
    fn set(&self, plugin: &str, key: &str, value: &str) {
        let conn = self.conn.lock().unwrap();
        let _ = conn.execute(
            "INSERT OR REPLACE INTO plugin_kv (plugin_name, key, value) VALUES (?1, ?2, ?3)",
            [plugin, key, value],
        );
    }
}
