use napi::bindgen_prelude::Buffer;
// Return types must be spelled `Result` so napi-derive throws on Err instead of returning an Error object.
use napi::{Result, Status};
use napi_derive::napi;
use serde_json::{Map, Value};

use crate::error::Error;
use crate::item::{AttachmentType, ItemUpdate, NewItem, SYSTEM_FIELDS};
use crate::storage::CatalogStore;

fn to_napi(e: Error) -> napi::Error {
    let status = match e {
        Error::Io(_) | Error::Cache(_) => Status::GenericFailure,
        _ => Status::InvalidArg,
    };
    napi::Error::new(status, e.to_string())
}

fn invalid(msg: impl Into<String>) -> napi::Error {
    napi::Error::new(Status::InvalidArg, msg.into())
}

/// Validates a JS object for add/update and splits out explicit nulls (field clears).
fn split_input(value: Value) -> Result<(Value, Vec<String>)> {
    let Value::Object(map) = value else {
        return Err(invalid("item fields must be an object"));
    };
    let mut kept = Map::new();
    let mut cleared = Vec::new();
    for (key, v) in map {
        if SYSTEM_FIELDS.contains(&key.as_str()) {
            return Err(invalid(format!("{key} is managed by AllOurThings and cannot be set")));
        }
        if key == "attachments" {
            return Err(invalid("attachments cannot be set directly; use the attachment tools"));
        }
        if v.is_null() {
            cleared.push(key);
        } else {
            kept.insert(key, v);
        }
    }
    Ok((Value::Object(kept), cleared))
}

/// Filter options for listItems, exposed as a plain JS object.
#[napi(object)]
#[derive(Default)]
pub struct JsListFilter {
    pub category: Option<String>,
    pub subcategory: Option<String>,
    pub tags: Option<Vec<String>>,
}

/// The main storage class exposed to JavaScript.
///
/// Methods are synchronous on the Rust side — the JS layer wraps them in
/// Promise.resolve() so they satisfy the async Backend interface without
/// blocking the event loop (operations are fast local filesystem I/O).
#[napi]
pub struct JsCatalogStore {
    inner: CatalogStore,
}

#[napi]
impl JsCatalogStore {
    #[napi(constructor)]
    pub fn new(data_dir: String, cache_dir: Option<String>) -> Self {
        let inner = match cache_dir {
            Some(cd) => CatalogStore::new_with_cache(data_dir.clone(), cd)
                .unwrap_or_else(|e| {
                    eprintln!("[allourthings] cache disabled: {e}");
                    CatalogStore::new(data_dir)
                }),
            None => CatalogStore::new(data_dir),
        };
        Self { inner }
    }

    #[napi]
    pub fn refresh(&self) -> Result<()> {
        self.inner.refresh().map_err(to_napi)
    }

    #[napi]
    pub fn rebuild_cache(&self) -> Result<()> {
        self.inner.rebuild_cache().map_err(to_napi)
    }

    #[napi]
    pub fn add_item(&self, new_item: Value) -> Result<Value> {
        let (fields, _nulls) = split_input(new_item)?;
        let parsed: NewItem = serde_json::from_value(fields)
            .map_err(|e| invalid(e.to_string()))?;
        let item = self.inner.add_item(parsed).map_err(to_napi)?;
        serde_json::to_value(item)
            .map_err(|e| napi::Error::from_reason(e.to_string()))
    }

    #[napi]
    pub fn get_item(&self, id_or_name: String) -> Result<Option<Value>> {
        let item = self.inner.get_item(&id_or_name).map_err(to_napi)?;
        match item {
            Some(i) => Ok(Some(
                serde_json::to_value(i)
                    .map_err(|e| napi::Error::from_reason(e.to_string()))?,
            )),
            None => Ok(None),
        }
    }

    #[napi]
    pub fn list_items(&self, filter: Option<JsListFilter>) -> Result<Value> {
        let list_filter = filter.map(|f| crate::item::ListFilter {
            category: f.category,
            subcategory: f.subcategory,
            tags: f.tags,
        });
        let items = self.inner.list_items(list_filter).map_err(to_napi)?;
        serde_json::to_value(items)
            .map_err(|e| napi::Error::from_reason(e.to_string()))
    }

    #[napi]
    pub fn update_item(&self, id: String, updates: Value) -> Result<Option<Value>> {
        let (fields, cleared) = split_input(updates)?;
        let mut parsed: ItemUpdate = serde_json::from_value(fields)
            .map_err(|e| invalid(e.to_string()))?;
        parsed.clear = cleared;
        let item = self.inner.update_item(&id, parsed).map_err(to_napi)?;
        match item {
            Some(i) => Ok(Some(
                serde_json::to_value(i)
                    .map_err(|e| napi::Error::from_reason(e.to_string()))?,
            )),
            None => Ok(None),
        }
    }

    #[napi]
    pub fn delete_item(&self, id: String) -> Result<bool> {
        self.inner.delete_item(&id).map_err(to_napi)
    }

    #[napi]
    pub fn search_items(&self, query: String) -> Result<Value> {
        let items = self.inner.search_items(&query).map_err(to_napi)?;
        serde_json::to_value(items)
            .map_err(|e| napi::Error::from_reason(e.to_string()))
    }

    #[napi]
    pub fn get_item_fields(&self) -> Result<Vec<String>> {
        self.inner.get_item_fields().map_err(to_napi)
    }

    #[napi]
    pub fn add_attachment(
        &self,
        item_id: String,
        filename: String,
        kind: String,
        data: Buffer,
        label: Option<String>,
    ) -> Result<Value> {
        let attachment_type = match kind.as_str() {
            "manual"   => AttachmentType::Manual,
            "receipt"  => AttachmentType::Receipt,
            "photo"    => AttachmentType::Photo,
            "warranty" => AttachmentType::Warranty,
            "other"    => AttachmentType::Other,
            other => return Err(invalid(format!(
                "unknown attachment kind \"{other}\"; expected manual, receipt, photo, warranty or other"
            ))),
        };
        let item = self.inner
            .add_attachment(&item_id, &filename, attachment_type, &data, label)
            .map_err(to_napi)?;
        serde_json::to_value(item)
            .map_err(|e| napi::Error::from_reason(e.to_string()))
    }

    #[napi]
    pub fn get_attachment(&self, item_id: String, filename: String) -> Result<Buffer> {
        let bytes = self.inner
            .get_attachment(&item_id, &filename)
            .map_err(to_napi)?;
        Ok(Buffer::from(bytes))
    }

    #[napi]
    pub fn delete_attachment(&self, item_id: String, filename: String) -> Result<Option<Value>> {
        let item = self.inner
            .delete_attachment(&item_id, &filename)
            .map_err(to_napi)?;
        match item {
            Some(i) => Ok(Some(
                serde_json::to_value(i)
                    .map_err(|e| napi::Error::from_reason(e.to_string()))?,
            )),
            None => Ok(None),
        }
    }
}
