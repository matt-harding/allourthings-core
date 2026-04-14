use std::fs;
use std::path::{Component, Path, PathBuf};

use chrono::Utc;

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
        Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
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

    // -------------------------------------------------------------------------
    // Attachment file I/O
    // -------------------------------------------------------------------------

    /// Reject filenames that contain path separators or special components
    /// (e.g. `../escape`, `/etc/passwd`, `.`). Only a single `Normal` component
    /// is accepted.
    fn validate_filename(filename: &str) -> Result<(), Error> {
        let path = Path::new(filename);
        let mut components = path.components();
        match (components.next(), components.next()) {
            (Some(Component::Normal(_)), None) => Ok(()),
            _ => Err(Error::InvalidFilename(filename.to_string())),
        }
    }

    /// Write `data` as `filename` inside the item's directory, and record it in
    /// `item.json`. If an attachment with the same filename already exists its
    /// record is replaced (useful for re-uploading an updated photo).
    pub fn add_attachment(
        &self,
        item_id: &str,
        filename: &str,
        kind: crate::item::AttachmentType,
        data: &[u8],
        label: Option<String>,
    ) -> Result<Item, Error> {
        Self::validate_filename(filename)?;
        let dir = self.find_dir_by_id(item_id)?
            .ok_or_else(|| Error::NotFound(item_id.to_string()))?;
        let mut item = Self::read_item_from_dir(&dir)?
            .ok_or_else(|| Error::NotFound(item_id.to_string()))?;

        fs::write(dir.join(filename), data)?;

        let new_attachment = crate::item::Attachment { filename: filename.to_string(), kind, label };
        let mut attachments = item.attachments.unwrap_or_default();
        if let Some(pos) = attachments.iter().position(|a| a.filename == filename) {
            attachments[pos] = new_attachment;
        } else {
            attachments.push(new_attachment);
        }
        item.attachments = Some(attachments);
        item.updated_at = Self::now_iso8601();

        Self::write_item_to_dir(&dir, &item)?;
        Ok(item)
    }

    /// Read raw bytes for a named attachment file.
    pub fn get_attachment(&self, item_id: &str, filename: &str) -> Result<Vec<u8>, Error> {
        Self::validate_filename(filename)?;
        let dir = self.find_dir_by_id(item_id)?
            .ok_or_else(|| Error::NotFound(item_id.to_string()))?;
        Ok(fs::read(dir.join(filename))?)
    }

    /// Delete an attachment file and remove its record from `item.json`.
    /// Returns `None` if the item does not exist.
    pub fn delete_attachment(&self, item_id: &str, filename: &str) -> Result<Option<Item>, Error> {
        Self::validate_filename(filename)?;
        let dir = match self.find_dir_by_id(item_id)? {
            Some(d) => d,
            None => return Ok(None),
        };
        let mut item = match Self::read_item_from_dir(&dir)? {
            Some(i) => i,
            None => return Ok(None),
        };

        let file_path = dir.join(filename);
        if file_path.exists() {
            fs::remove_file(&file_path)?;
        }

        if let Some(ref mut attachments) = item.attachments {
            attachments.retain(|a| a.filename != filename);
            if attachments.is_empty() {
                item.attachments = None;
            }
        }
        item.updated_at = Self::now_iso8601();

        Self::write_item_to_dir(&dir, &item)?;
        Ok(Some(item))
    }
}

#[cfg(test)]
mod attachment_tests {
    use super::*;
    use crate::item::{AttachmentType, NewItem};

    fn make_store() -> (CatalogStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let store = CatalogStore::new(dir.path());
        (store, dir)
    }

    fn add_test_item(store: &CatalogStore) -> Item {
        store.add_item(NewItem {
            name: "Test Item".to_string(),
            ..Default::default()
        }).unwrap()
    }

    #[test]
    fn add_and_get_attachment() {
        let (store, _dir) = make_store();
        let item = add_test_item(&store);
        let data = b"fake image bytes";

        let updated = store.add_attachment(&item.id, "photo.jpg", AttachmentType::Photo, data, None).unwrap();
        assert_eq!(updated.attachments.as_ref().unwrap().len(), 1);
        assert_eq!(updated.attachments.as_ref().unwrap()[0].filename, "photo.jpg");

        let retrieved = store.get_attachment(&item.id, "photo.jpg").unwrap();
        assert_eq!(retrieved, data);
    }

    #[test]
    fn add_attachment_with_label() {
        let (store, _dir) = make_store();
        let item = add_test_item(&store);

        let updated = store.add_attachment(
            &item.id, "receipt.jpg", AttachmentType::Receipt, b"data", Some("Amazon receipt".to_string()),
        ).unwrap();
        let att = &updated.attachments.as_ref().unwrap()[0];
        assert_eq!(att.label, Some("Amazon receipt".to_string()));
    }

    #[test]
    fn replace_existing_attachment() {
        let (store, _dir) = make_store();
        let item = add_test_item(&store);

        store.add_attachment(&item.id, "photo.jpg", AttachmentType::Photo, b"v1", None).unwrap();
        let updated = store.add_attachment(&item.id, "photo.jpg", AttachmentType::Photo, b"v2", Some("updated".to_string())).unwrap();

        // Still only one attachment record
        assert_eq!(updated.attachments.as_ref().unwrap().len(), 1);
        // File bytes replaced
        assert_eq!(store.get_attachment(&item.id, "photo.jpg").unwrap(), b"v2");
    }

    #[test]
    fn delete_attachment_removes_file_and_record() {
        let (store, _dir) = make_store();
        let item = add_test_item(&store);

        store.add_attachment(&item.id, "photo.jpg", AttachmentType::Photo, b"data", None).unwrap();
        let updated = store.delete_attachment(&item.id, "photo.jpg").unwrap().unwrap();

        assert!(updated.attachments.is_none());
        assert!(store.get_attachment(&item.id, "photo.jpg").is_err());
    }

    #[test]
    fn delete_attachment_on_missing_item_returns_none() {
        let (store, _dir) = make_store();
        let result = store.delete_attachment("nonexistent", "photo.jpg").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn get_attachment_on_missing_item_returns_error() {
        let (store, _dir) = make_store();
        assert!(store.get_attachment("nonexistent", "photo.jpg").is_err());
    }

    #[test]
    fn multiple_attachments_coexist() {
        let (store, _dir) = make_store();
        let item = add_test_item(&store);

        store.add_attachment(&item.id, "photo1.jpg", AttachmentType::Photo, b"img1", None).unwrap();
        let updated = store.add_attachment(&item.id, "photo2.jpg", AttachmentType::Photo, b"img2", None).unwrap();

        assert_eq!(updated.attachments.as_ref().unwrap().len(), 2);
        assert_eq!(store.get_attachment(&item.id, "photo1.jpg").unwrap(), b"img1");
        assert_eq!(store.get_attachment(&item.id, "photo2.jpg").unwrap(), b"img2");
    }
}
