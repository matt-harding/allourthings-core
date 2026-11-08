use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Mutex;

use crate::error::Error;
use crate::item::{Item, ListFilter, SearchHit};

const SCHEMA_VERSION: i64 = 3;

// English stemming, accent folding. Changing this requires a SCHEMA_VERSION bump.
const FTS_TOKENIZER: &str = "porter unicode61 remove_diacritics 2";

// bm25 weights in column order: id (unindexed), name, brand_model, categories, notes, attachments.
const FTS_RANK: &str = "bm25(items_fts, 0.0, 10.0, 5.0, 3.0, 1.0, 1.0)";

/// Quotes each word so user input is matched as text, never as FTS5 query syntax.
fn fts_terms(query: &str) -> Vec<String> {
    query
        .split_whitespace()
        .filter(|w| w.chars().any(char::is_alphanumeric))
        .map(|w| format!("\"{}\"", w.replace('"', "\"\"")))
        .collect()
}

fn row_to_item(json: &str) -> rusqlite::Result<Item> {
    serde_json::from_str(json).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    })
}

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

    /// A throwaway cache used to search stores that have no on-disk cache.
    pub(crate) fn open_in_memory() -> Result<Self, Error> {
        let db = Self { conn: Mutex::new(rusqlite::Connection::open_in_memory()?) };
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
        conn.execute_batch(&format!(
            "CREATE VIRTUAL TABLE IF NOT EXISTS items_fts USING fts5(
                id UNINDEXED, name, brand_model, categories, notes, attachments,
                tokenize = '{FTS_TOKENIZER}'
            );"
        ))?;

        // Schema migration: zero out mtimes so refresh() re-upserts all rows,
        // repopulating item_tags and items_fts for any pre-existing cache.
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
        let tags_json = item.tags.as_ref().map(serde_json::to_string).transpose()?;
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

        let text = item.search_text();
        conn.execute("DELETE FROM items_fts WHERE id = ?1", [&item.id])?;
        conn.execute(
            "INSERT INTO items_fts (id, name, brand_model, categories, notes, attachments)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![item.id, text.name, text.brand_model, text.categories, text.notes, text.attachments],
        )?;

        Ok(())
    }

    pub fn delete(&self, id: &str) -> Result<(), Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM item_tags WHERE item_id = ?1", [id])?;
        conn.execute("DELETE FROM items_fts WHERE id = ?1", [id])?;
        conn.execute("DELETE FROM items WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn delete_all(&self) -> Result<(), Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM item_tags", [])?;
        conn.execute("DELETE FROM items_fts", [])?;
        conn.execute("DELETE FROM items", [])?;
        Ok(())
    }

    /// Ranked full-text search: items containing any query word, best first.
    pub fn search(&self, query: &str, limit: Option<usize>) -> Result<Vec<SearchHit>, Error> {
        let terms = fts_terms(query);
        if terms.is_empty() {
            return Ok(vec![]);
        }
        let conn = self.conn.lock().unwrap();
        let limit = limit.map_or(-1, |l| l as i64);

        let mut stmt = conn.prepare(&format!(
            "SELECT i.data FROM items_fts f JOIN items i ON i.id = f.id
             WHERE items_fts MATCH ?1
             ORDER BY {FTS_RANK}, i.id
             LIMIT ?2"
        ))?;
        let items: Vec<Item> = stmt
            .query_map(rusqlite::params![terms.join(" OR "), limit], |row| row_to_item(&row.get::<_, String>(0)?))?
            .filter_map(|r| r.ok())
            .collect();

        let all_ids: HashSet<String> = if terms.len() == 1 {
            items.iter().map(|i| i.id.clone()).collect()
        } else {
            let mut stmt = conn.prepare("SELECT id FROM items_fts WHERE items_fts MATCH ?1")?;
            let ids = stmt
                .query_map([terms.join(" ")], |row| row.get::<_, String>(0))?
                .filter_map(|r| r.ok())
                .collect();
            ids
        };

        Ok(items
            .into_iter()
            .map(|item| SearchHit { matched_all: all_ids.contains(&item.id), item })
            .collect())
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
            .query_map(rusqlite::params_from_iter(params), |row| row_to_item(&row.get::<_, String>(0)?))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(items)
    }
}
