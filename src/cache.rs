use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use crate::error::Error;
use crate::item::{Item, ListFilter};

pub struct CacheDb {
    conn: Mutex<rusqlite::Connection>,
}

impl CacheDb {
    pub fn open(cache_dir: &Path) -> Result<Self, Error> {
        std::fs::create_dir_all(cache_dir)?;
        let conn = rusqlite::Connection::open(cache_dir.join("catalog.db"))?;
        let db = Self { conn: Mutex::new(conn) };
        db.init_schema()?;
        Ok(db)
    }

    fn init_schema(&self) -> Result<(), Error> {
        self.conn.lock().unwrap().execute_batch(
            "CREATE TABLE IF NOT EXISTS items (
                id          TEXT PRIMARY KEY,
                category    TEXT,
                subcategory TEXT,
                tags        TEXT,
                file_mtime  INTEGER NOT NULL DEFAULT 0,
                data        TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_category    ON items(category);
            CREATE INDEX IF NOT EXISTS idx_subcategory ON items(subcategory);
            CREATE TABLE IF NOT EXISTS _meta (
                key   TEXT PRIMARY KEY,
                value TEXT
            );",
        )?;
        Ok(())
    }

    pub fn upsert(&self, item: &Item, file_mtime: i64) -> Result<(), Error> {
        let tags = item.tags.as_ref().map(|t| serde_json::to_string(t)).transpose()?;
        let data = serde_json::to_string(item)?;

        self.conn.lock().unwrap().execute(
            "INSERT OR REPLACE INTO items (id, category, subcategory, tags, file_mtime, data)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![item.id, item.category, item.subcategory, tags, file_mtime, data],
        )?;
        Ok(())
    }

    pub fn delete(&self, id: &str) -> Result<(), Error> {
        self.conn.lock().unwrap().execute("DELETE FROM items WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn delete_all(&self) -> Result<(), Error> {
        self.conn.lock().unwrap().execute("DELETE FROM items", [])?;
        Ok(())
    }

    pub fn get_id_mtime_map(&self) -> Result<HashMap<String, i64>, Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, file_mtime FROM items")?;
        let map: HashMap<String, i64> = stmt
            .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(map)
    }

    pub fn get_fields(&self) -> Result<Vec<String>, Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT DISTINCT key FROM items, json_each(items.data) ORDER BY key",
        )?;
        let fields: Vec<String> = stmt
            .query_map([], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(fields)
    }

    pub fn list_filtered(&self, filter: Option<&ListFilter>) -> Result<Vec<Item>, Error> {
        let (category, subcategory) = filter
            .map(|f| (f.category.as_deref(), f.subcategory.as_deref()))
            .unwrap_or((None, None));

        let items: Vec<Item> = {
            let conn = self.conn.lock().unwrap();
            let mut stmt = conn.prepare(
                "SELECT data FROM items
                 WHERE (?1 IS NULL OR category = ?1)
                   AND (?2 IS NULL OR subcategory = ?2)",
            )?;
            let rows: Vec<Item> = stmt
                .query_map(rusqlite::params![category, subcategory], |row| {
                    let json: String = row.get(0)?;
                    serde_json::from_str(&json).map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            0,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        )
                    })
                })?
                .filter_map(|r| r.ok())
                .collect();
            rows
        };

        let mut items = items;
        if let Some(tags) = filter.and_then(|f| f.tags.as_ref()) {
            if !tags.is_empty() {
                items.retain(|i| {
                    let item_tags = i.tags.as_deref().unwrap_or(&[]);
                    tags.iter().all(|t| item_tags.contains(t))
                });
            }
        }

        Ok(items)
    }
}
