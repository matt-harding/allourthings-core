use napi::bindgen_prelude::Buffer;
use napi::Result as NapiResult;
use napi_derive::napi;
use serde_json::Value;

use crate::item::{AttachmentType, ItemUpdate, NewItem};
use crate::storage::CatalogStore;

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
    pub fn refresh(&self) -> NapiResult<()> {
        self.inner.refresh().map_err(|e| napi::Error::from_reason(e.to_string()))
    }

    #[napi]
    pub fn rebuild_cache(&self) -> NapiResult<()> {
        self.inner.rebuild_cache().map_err(|e| napi::Error::from_reason(e.to_string()))
    }

    #[napi]
    pub fn add_item(&self, new_item: Value) -> NapiResult<Value> {
        let parsed: NewItem = serde_json::from_value(new_item)
            .map_err(|e| napi::Error::from_reason(e.to_string()))?;
        let item = self.inner.add_item(parsed)
            .map_err(|e| napi::Error::from_reason(e.to_string()))?;
        serde_json::to_value(item)
            .map_err(|e| napi::Error::from_reason(e.to_string()))
    }

    #[napi]
    pub fn get_item(&self, id_or_name: String) -> NapiResult<Option<Value>> {
        let item = self.inner.get_item(&id_or_name)
            .map_err(|e| napi::Error::from_reason(e.to_string()))?;
        match item {
            Some(i) => Ok(Some(
                serde_json::to_value(i)
                    .map_err(|e| napi::Error::from_reason(e.to_string()))?,
            )),
            None => Ok(None),
        }
    }

    #[napi]
    pub fn list_items(&self, filter: Option<JsListFilter>) -> NapiResult<Value> {
        let list_filter = filter.map(|f| crate::item::ListFilter {
            category: f.category,
            subcategory: f.subcategory,
            tags: f.tags,
        });
        let items = self.inner.list_items(list_filter)
            .map_err(|e| napi::Error::from_reason(e.to_string()))?;
        serde_json::to_value(items)
            .map_err(|e| napi::Error::from_reason(e.to_string()))
    }

    #[napi]
    pub fn update_item(&self, id: String, updates: Value) -> NapiResult<Option<Value>> {
        let parsed: ItemUpdate = serde_json::from_value(updates)
            .map_err(|e| napi::Error::from_reason(e.to_string()))?;
        let item = self.inner.update_item(&id, parsed)
            .map_err(|e| napi::Error::from_reason(e.to_string()))?;
        match item {
            Some(i) => Ok(Some(
                serde_json::to_value(i)
                    .map_err(|e| napi::Error::from_reason(e.to_string()))?,
            )),
            None => Ok(None),
        }
    }

    #[napi]
    pub fn delete_item(&self, id: String) -> NapiResult<bool> {
        self.inner.delete_item(&id)
            .map_err(|e| napi::Error::from_reason(e.to_string()))
    }

    #[napi]
    pub fn search_items(&self, query: String) -> NapiResult<Value> {
        let items = self.inner.search_items(&query)
            .map_err(|e| napi::Error::from_reason(e.to_string()))?;
        serde_json::to_value(items)
            .map_err(|e| napi::Error::from_reason(e.to_string()))
    }

    #[napi]
    pub fn get_item_fields(&self) -> NapiResult<Vec<String>> {
        self.inner.get_item_fields().map_err(|e| napi::Error::from_reason(e.to_string()))
    }

    #[napi]
    pub fn add_attachment(
        &self,
        item_id: String,
        filename: String,
        kind: String,
        data: Buffer,
        label: Option<String>,
    ) -> NapiResult<Value> {
        let attachment_type = match kind.as_str() {
            "manual"   => AttachmentType::Manual,
            "receipt"  => AttachmentType::Receipt,
            "photo"    => AttachmentType::Photo,
            "warranty" => AttachmentType::Warranty,
            _          => AttachmentType::Other,
        };
        let item = self.inner
            .add_attachment(&item_id, &filename, attachment_type, &data, label)
            .map_err(|e| napi::Error::from_reason(e.to_string()))?;
        serde_json::to_value(item)
            .map_err(|e| napi::Error::from_reason(e.to_string()))
    }

    #[napi]
    pub fn get_attachment(&self, item_id: String, filename: String) -> NapiResult<Buffer> {
        let bytes = self.inner
            .get_attachment(&item_id, &filename)
            .map_err(|e| napi::Error::from_reason(e.to_string()))?;
        Ok(Buffer::from(bytes))
    }

    #[napi]
    pub fn delete_attachment(&self, item_id: String, filename: String) -> NapiResult<Option<Value>> {
        let item = self.inner
            .delete_attachment(&item_id, &filename)
            .map_err(|e| napi::Error::from_reason(e.to_string()))?;
        match item {
            Some(i) => Ok(Some(
                serde_json::to_value(i)
                    .map_err(|e| napi::Error::from_reason(e.to_string()))?,
            )),
            None => Ok(None),
        }
    }
}
