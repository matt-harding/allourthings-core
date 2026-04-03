use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::error::Error;
use crate::id::generate_id;
use crate::item::{Item, ItemUpdate, NewItem};
use crate::slug::to_slug;

/// Filtering options for list_items.
#[derive(Debug, Default)]
pub struct ListFilter {
    pub category: Option<String>,
    pub location: Option<String>,
    pub tags: Option<Vec<String>>,
}

/// The main entry point for reading and writing catalog data.
///
/// One `CatalogStore` per catalog directory. Thread-safety is the caller's
/// responsibility — this is intentionally simple and sync.
pub struct CatalogStore {
    data_dir: PathBuf,
}

impl CatalogStore {
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self { data_dir: data_dir.into() }
    }

    fn items_dir(&self) -> PathBuf {
        self.data_dir.join("items")
    }

    fn item_dir_path(&self, name: &str, id: &str) -> PathBuf {
        self.items_dir().join(format!("{}-{}", to_slug(name), id))
    }

    /// Scan items/ for the directory whose name ends with `-<id>`.
    fn find_dir_by_id(&self, id: &str) -> Result<Option<PathBuf>, Error> {
        let items_dir = self.items_dir();
        if !items_dir.exists() {
            return Ok(None);
        }
        let suffix = format!("-{}", id);
        for entry in fs::read_dir(&items_dir)? {
            let entry = entry?;
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.ends_with(&suffix) && entry.file_type()?.is_dir() {
                return Ok(Some(entry.path()));
            }
        }
        Ok(None)
    }

    fn read_item_from_dir(dir: &Path) -> Result<Option<Item>, Error> {
        let json_path = dir.join("item.json");
        if !json_path.exists() {
            return Ok(None);
        }
        let raw = fs::read_to_string(&json_path)?;
        Ok(serde_json::from_str(&raw).ok()) // silently return None if malformed
    }

    fn write_item_to_dir(dir: &Path, item: &Item) -> Result<(), Error> {
        fs::create_dir_all(dir)?;
        let tmp = dir.join("item.json.tmp");
        let json = serde_json::to_string_pretty(item)?;
        fs::write(&tmp, json)?;
        fs::rename(&tmp, dir.join("item.json"))?; // atomic on same filesystem
        Ok(())
    }

    fn load_all(&self) -> Result<Vec<Item>, Error> {
        let items_dir = self.items_dir();
        if !items_dir.exists() {
            return Ok(vec![]);
        }
        let mut items = Vec::new();
        for entry in fs::read_dir(&items_dir)? {
            let entry = entry?;
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            // Skip hidden entries (STORAGE_SPEC.md §2)
            if name_str.starts_with('.') {
                continue;
            }
            if !entry.file_type()?.is_dir() {
                continue;
            }
            if let Some(item) = Self::read_item_from_dir(&entry.path())? {
                items.push(item);
            }
            // Directories without item.json or with malformed JSON are silently skipped
        }
        Ok(items)
    }

    fn now_iso8601() -> String {
        // Format current UTC time as ISO 8601 with milliseconds
        let secs = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let s = secs / 1000;
        let ms = secs % 1000;
        // Manual UTC formatting — avoids a chrono/time dependency for now
        let (y, mo, d, h, min, sec) = epoch_to_ymd_hms(s as u64);
        format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z", y, mo, d, h, min, sec, ms)
    }

    // -------------------------------------------------------------------------
    // Public CRUD API
    // -------------------------------------------------------------------------

    pub fn add_item(&self, new_item: NewItem) -> Result<Item, Error> {
        let id = new_item.id.clone().unwrap_or_else(generate_id);
        let now = Self::now_iso8601();
        let item = Item {
            id: id.clone(),
            name: new_item.name.clone(),
            created_at: now.clone(),
            updated_at: now,
            category: new_item.category,
            brand: new_item.brand,
            model: new_item.model,
            purchase_date: new_item.purchase_date,
            purchase_price: new_item.purchase_price,
            currency: new_item.currency,
            warranty_expires: new_item.warranty_expires,
            retailer: new_item.retailer,
            location: new_item.location,
            features: new_item.features,
            notes: new_item.notes,
            tags: new_item.tags,
            attachments: new_item.attachments,
            extra: new_item.extra,
        };
        let dir = self.item_dir_path(&item.name, &id);
        Self::write_item_to_dir(&dir, &item)?;
        Ok(item)
    }

    pub fn get_item(&self, id_or_name: &str) -> Result<Option<Item>, Error> {
        // Try exact ID match first
        if let Some(dir) = self.find_dir_by_id(id_or_name)? {
            return Ok(Self::read_item_from_dir(&dir)?);
        }
        // Fall back to name search (exact, then substring)
        let items = self.load_all()?;
        let lower = id_or_name.to_lowercase();
        let found = items
            .iter()
            .find(|i| i.name.to_lowercase() == lower)
            .or_else(|| items.iter().find(|i| i.name.to_lowercase().contains(&lower)))
            .cloned();
        Ok(found)
    }

    pub fn list_items(&self, filter: Option<ListFilter>) -> Result<Vec<Item>, Error> {
        let mut items = self.load_all()?;
        if let Some(f) = filter {
            if let Some(cat) = f.category {
                items.retain(|i| i.category.as_deref() == Some(cat.as_str()));
            }
            if let Some(loc) = f.location {
                items.retain(|i| {
                    i.location.as_deref().map(|l| l.eq_ignore_ascii_case(&loc)).unwrap_or(false)
                });
            }
            if let Some(tags) = f.tags {
                items.retain(|i| {
                    let item_tags = i.tags.as_deref().unwrap_or(&[]);
                    tags.iter().all(|t| item_tags.contains(t))
                });
            }
        }
        Ok(items)
    }

    pub fn update_item(&self, id: &str, updates: ItemUpdate) -> Result<Option<Item>, Error> {
        let old_dir = match self.find_dir_by_id(id)? {
            Some(d) => d,
            None => return Ok(None),
        };
        let existing = match Self::read_item_from_dir(&old_dir)? {
            Some(i) => i,
            None => return Ok(None),
        };

        let new_name = updates.name.unwrap_or(existing.name.clone());
        let mut merged_extra = existing.extra.clone();
        merged_extra.extend(updates.extra);

        let updated = Item {
            id: existing.id.clone(),
            name: new_name.clone(),
            created_at: existing.created_at.clone(),
            updated_at: Self::now_iso8601(),
            category: updates.category.or(existing.category),
            brand: updates.brand.or(existing.brand),
            model: updates.model.or(existing.model),
            purchase_date: updates.purchase_date.or(existing.purchase_date),
            purchase_price: updates.purchase_price.or(existing.purchase_price),
            currency: updates.currency.or(existing.currency),
            warranty_expires: updates.warranty_expires.or(existing.warranty_expires),
            retailer: updates.retailer.or(existing.retailer),
            location: updates.location.or(existing.location),
            features: updates.features.or(existing.features),
            notes: updates.notes.or(existing.notes),
            tags: updates.tags.or(existing.tags),
            attachments: updates.attachments.or(existing.attachments),
            extra: merged_extra,
        };

        let new_dir = self.item_dir_path(&new_name, id);
        if old_dir != new_dir {
            fs::rename(&old_dir, &new_dir)?;
        }
        Self::write_item_to_dir(&new_dir, &updated)?;
        Ok(Some(updated))
    }

    pub fn delete_item(&self, id: &str) -> Result<bool, Error> {
        match self.find_dir_by_id(id)? {
            Some(dir) => {
                fs::remove_dir_all(dir)?;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    pub fn search_items(&self, query: &str) -> Result<Vec<Item>, Error> {
        let lower = query.to_lowercase();
        let items = self.load_all()?;
        Ok(items
            .into_iter()
            .filter(|i| serde_json::to_string(i).unwrap_or_default().to_lowercase().contains(&lower))
            .collect())
    }
}

// ---------------------------------------------------------------------------
// Minimal UTC epoch → (year, month, day, hour, min, sec) without dependencies
// ---------------------------------------------------------------------------

fn epoch_to_ymd_hms(epoch_secs: u64) -> (u32, u32, u32, u32, u32, u32) {
    let sec = (epoch_secs % 60) as u32;
    let mins = epoch_secs / 60;
    let min = (mins % 60) as u32;
    let hours = mins / 60;
    let hour = (hours % 24) as u32;
    let days = (hours / 24) as u32;

    // Gregorian calendar calculation
    let mut y = 1970u32;
    let mut d = days;
    loop {
        let days_in_year = if is_leap(y) { 366 } else { 365 };
        if d < days_in_year {
            break;
        }
        d -= days_in_year;
        y += 1;
    }
    let months = [31u32, if is_leap(y) { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut mo = 0u32;
    for (i, &days_in_month) in months.iter().enumerate() {
        if d < days_in_month {
            mo = i as u32 + 1;
            break;
        }
        d -= days_in_month;
    }
    (y, mo, d + 1, hour, min, sec)
}

fn is_leap(y: u32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}
