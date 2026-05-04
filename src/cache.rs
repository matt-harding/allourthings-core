use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use crate::error::Error;
use crate::item::{Item, ListFilter};

const SCHEMA_VERSION: i64 = 2;

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
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
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
            CREATE TABLE IF NOT EXISTS item_tags (
                item_id TEXT NOT NULL,
                tag     TEXT NOT NULL,
                PRIMARY KEY (item_id, tag)
            );
            CREATE INDEX IF NOT EXISTS idx_item_tags ON item_tags(item_id);
            CREATE TABLE IF NOT EXISTS _meta (
                key   TEXT PRIMARY KEY,
                value TEXT
            );",
        )?;

        // Schema migration: zero out mtimes so refresh() re-upserts all rows
        // and populates item_tags for any pre-existing cache.
        let version: i64 = conn
            .query_row(
                "SELECT CAST(value AS INTEGER) FROM _meta WHERE key = 'schema_version'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);

        if version < SCHEMA_VERSION {
            conn.execute("UPDATE items SET file_mtime = 0", [])?;
            conn.execute(
                "INSERT OR REPLACE INTO _meta (key, value) VALUES ('schema_version', ?1)",
                [SCHEMA_VERSION.to_string()],
            )?;
        }

        Ok(())
    }

    pub fn upsert(&self, item: &Item, file_mtime: i64) -> Result<(), Error> {
        let tags_json = item.tags.as_ref().map(|t| serde_json::to_string(t)).transpose()?;
        let data = serde_json::to_string(item)?;
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "INSERT OR REPLACE INTO items (id, category, subcategory, tags, file_mtime, data)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![item.id, item.category, item.subcategory, tags_json, file_mtime, data],
        )?;

        conn.execute("DELETE FROM item_tags WHERE item_id = ?1", [&item.id])?;
        if let Some(tags) = &item.tags {
            for tag in tags {
                conn.execute(
                    "INSERT OR IGNORE INTO item_tags (item_id, tag) VALUES (?1, ?2)",
                    rusqlite::params![item.id, tag],
                )?;
            }
        }

        Ok(())
    }

    pub fn delete(&self, id: &str) -> Result<(), Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM item_tags WHERE item_id = ?1", [id])?;
        conn.execute("DELETE FROM items WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn delete_all(&self) -> Result<(), Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM item_tags", [])?;
        conn.execute("DELETE FROM items", [])?;
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
        let category = filter.and_then(|f| f.category.as_deref());
        let subcategory = filter.and_then(|f| f.subcategory.as_deref());
        let tags: &[String] = filter
            .and_then(|f| f.tags.as_deref())
            .unwrap_or(&[]);

        use rusqlite::types::Value;

        let mut params: Vec<Value> = vec![
            category.map(|s| Value::Text(s.to_owned())).unwrap_or(Value::Null),
            subcategory.map(|s| Value::Text(s.to_owned())).unwrap_or(Value::Null),
        ];

        let sql = if tags.is_empty() {
            "SELECT data FROM items \
             WHERE (?1 IS NULL OR category = ?1) \
               AND (?2 IS NULL OR subcategory = ?2)"
                .to_owned()
        } else {
            let placeholders = (3..3 + tags.len())
                .map(|i| format!("?{i}"))
                .collect::<Vec<_>>()
                .join(", ");
            for tag in tags {
                params.push(Value::Text(tag.clone()));
            }
            format!(
                "SELECT data FROM items \
                 WHERE (?1 IS NULL OR category = ?1) \
                   AND (?2 IS NULL OR subcategory = ?2) \
                   AND (SELECT COUNT(*) FROM item_tags \
                        WHERE item_id = id AND tag IN ({placeholders})) = {}",
                tags.len()
            )
        };

        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(&sql)?;
        let items: Vec<Item> = stmt
            .query_map(rusqlite::params_from_iter(params), |row| {
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

        Ok(items)
    }

    /// Full-text search over serialised item JSON, equivalent to the no-cache
    /// path but using SQLite's LIKE operator against the stored data column.
    pub fn search(&self, query: &str) -> Result<Vec<Item>, Error> {
        let pattern = format!("%{}%", query.to_lowercase());
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT data FROM items WHERE LOWER(data) LIKE ?1",
        )?;
        let items: Vec<Item> = stmt
            .query_map([pattern], |row| {
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
        Ok(items)
    }
}
