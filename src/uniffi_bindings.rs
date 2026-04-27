use std::sync::Arc;

use crate::{
    error::Error,
    item::{Attachment as CoreAttachment, AttachmentType, Item as CoreItem, ItemUpdate, ListFilter, NewItem},
    storage::CatalogStore,
};

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum CatalogError {
    #[error("not found: {id}")]
    NotFound { id: String },

    #[error("io error: {message}")]
    Io { message: String },

    #[error("json error: {message}")]
    Json { message: String },

    #[error("invalid filename: {filename}")]
    InvalidFilename { filename: String },

    #[error("cache error: {message}")]
    Cache { message: String },
}

impl From<Error> for CatalogError {
    fn from(e: Error) -> Self {
        match e {
            Error::NotFound(id) => CatalogError::NotFound { id },
            Error::Io(e) => CatalogError::Io { message: e.to_string() },
            Error::Json(e) => CatalogError::Json { message: e.to_string() },
            Error::InvalidFilename(filename) => CatalogError::InvalidFilename { filename },
            Error::Cache(e) => CatalogError::Cache { message: e.to_string() },
        }
    }
}

// ---------------------------------------------------------------------------
// AttachmentKind
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, uniffi::Enum)]
pub enum AttachmentKind {
    Manual,
    Receipt,
    Photo,
    Warranty,
    Other,
}

impl From<AttachmentType> for AttachmentKind {
    fn from(t: AttachmentType) -> Self {
        match t {
            AttachmentType::Manual  => AttachmentKind::Manual,
            AttachmentType::Receipt => AttachmentKind::Receipt,
            AttachmentType::Photo   => AttachmentKind::Photo,
            AttachmentType::Warranty => AttachmentKind::Warranty,
            AttachmentType::Other   => AttachmentKind::Other,
        }
    }
}

impl From<AttachmentKind> for AttachmentType {
    fn from(k: AttachmentKind) -> Self {
        match k {
            AttachmentKind::Manual  => AttachmentType::Manual,
            AttachmentKind::Receipt => AttachmentType::Receipt,
            AttachmentKind::Photo   => AttachmentType::Photo,
            AttachmentKind::Warranty => AttachmentType::Warranty,
            AttachmentKind::Other   => AttachmentType::Other,
        }
    }
}

// ---------------------------------------------------------------------------
// UniAttachment
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, uniffi::Record)]
pub struct UniAttachment {
    pub filename: String,
    pub kind: AttachmentKind,
    pub label: Option<String>,
}

impl From<CoreAttachment> for UniAttachment {
    fn from(a: CoreAttachment) -> Self {
        UniAttachment { filename: a.filename, kind: a.kind.into(), label: a.label }
    }
}

impl From<UniAttachment> for CoreAttachment {
    fn from(a: UniAttachment) -> Self {
        CoreAttachment { filename: a.filename, kind: a.kind.into(), label: a.label }
    }
}

// ---------------------------------------------------------------------------
// UniItem
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, uniffi::Record)]
pub struct UniItem {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
    pub category: Option<String>,
    pub subcategory: Option<String>,
    pub brand: Option<String>,
    pub model: Option<String>,
    pub purchase_date: Option<String>,
    pub purchase_price: Option<f64>,
    pub currency: Option<String>,
    pub warranty_expires: Option<String>,
    pub retailer: Option<String>,
    pub location: Option<String>,
    pub features: Option<Vec<String>>,
    pub notes: Option<String>,
    pub tags: Option<Vec<String>>,
    pub attachments: Option<Vec<UniAttachment>>,
    /// JSON-encoded passthrough custom fields (e.g. `{}`).
    pub extra_json: String,
}

impl From<CoreItem> for UniItem {
    fn from(i: CoreItem) -> Self {
        UniItem {
            id: i.id,
            name: i.name,
            created_at: i.created_at,
            updated_at: i.updated_at,
            category: i.category,
            subcategory: i.subcategory,
            brand: i.brand,
            model: i.model,
            purchase_date: i.purchase_date,
            purchase_price: i.purchase_price,
            currency: i.currency,
            warranty_expires: i.warranty_expires,
            retailer: i.retailer,
            location: i.location,
            features: i.features,
            notes: i.notes,
            tags: i.tags,
            attachments: i.attachments.map(|v| v.into_iter().map(Into::into).collect()),
            extra_json: serde_json::to_string(&i.extra).unwrap_or_else(|_| "{}".to_string()),
        }
    }
}

// ---------------------------------------------------------------------------
// UniNewItem
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, uniffi::Record)]
pub struct UniNewItem {
    /// Optional caller-supplied ID. If empty string, one is generated by the store.
    pub id: Option<String>,
    pub name: String,
    pub category: Option<String>,
    pub subcategory: Option<String>,
    pub brand: Option<String>,
    pub model: Option<String>,
    pub purchase_date: Option<String>,
    pub purchase_price: Option<f64>,
    pub currency: Option<String>,
    pub warranty_expires: Option<String>,
    pub retailer: Option<String>,
    pub location: Option<String>,
    pub features: Option<Vec<String>>,
    pub notes: Option<String>,
    pub tags: Option<Vec<String>>,
    pub attachments: Option<Vec<UniAttachment>>,
    /// JSON-encoded extra fields (pass `"{}"` if none).
    pub extra_json: String,
}

impl TryFrom<UniNewItem> for NewItem {
    type Error = CatalogError;

    fn try_from(u: UniNewItem) -> Result<Self, Self::Error> {
        let extra = serde_json::from_str(&u.extra_json)
            .map_err(|e| CatalogError::Json { message: e.to_string() })?;
        Ok(NewItem {
            id: u.id,
            name: u.name,
            category: u.category,
            subcategory: u.subcategory,
            brand: u.brand,
            model: u.model,
            purchase_date: u.purchase_date,
            purchase_price: u.purchase_price,
            currency: u.currency,
            warranty_expires: u.warranty_expires,
            retailer: u.retailer,
            location: u.location,
            features: u.features,
            notes: u.notes,
            tags: u.tags,
            attachments: u.attachments.map(|v| v.into_iter().map(Into::into).collect()),
            extra: extra,
        })
    }
}

// ---------------------------------------------------------------------------
// UniItemUpdate
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, uniffi::Record)]
pub struct UniItemUpdate {
    pub name: Option<String>,
    pub category: Option<String>,
    pub subcategory: Option<String>,
    pub brand: Option<String>,
    pub model: Option<String>,
    pub purchase_date: Option<String>,
    pub purchase_price: Option<f64>,
    pub currency: Option<String>,
    pub warranty_expires: Option<String>,
    pub retailer: Option<String>,
    pub location: Option<String>,
    pub features: Option<Vec<String>>,
    pub notes: Option<String>,
    pub tags: Option<Vec<String>>,
    pub attachments: Option<Vec<UniAttachment>>,
    /// JSON-encoded extra fields to merge in (pass `"{}"` to change nothing).
    pub extra_json: String,
}

impl TryFrom<UniItemUpdate> for ItemUpdate {
    type Error = CatalogError;

    fn try_from(u: UniItemUpdate) -> Result<Self, Self::Error> {
        let extra = serde_json::from_str(&u.extra_json)
            .map_err(|e| CatalogError::Json { message: e.to_string() })?;
        Ok(ItemUpdate {
            name: u.name,
            category: u.category,
            subcategory: u.subcategory,
            brand: u.brand,
            model: u.model,
            purchase_date: u.purchase_date,
            purchase_price: u.purchase_price,
            currency: u.currency,
            warranty_expires: u.warranty_expires,
            retailer: u.retailer,
            location: u.location,
            features: u.features,
            notes: u.notes,
            tags: u.tags,
            attachments: u.attachments.map(|v| v.into_iter().map(Into::into).collect()),
            extra: extra,
        })
    }
}

// ---------------------------------------------------------------------------
// UniListFilter
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, uniffi::Record)]
pub struct UniListFilter {
    pub category: Option<String>,
    pub subcategory: Option<String>,
    pub tags: Option<Vec<String>>,
}

impl From<UniListFilter> for ListFilter {
    fn from(f: UniListFilter) -> Self {
        ListFilter { category: f.category, subcategory: f.subcategory, tags: f.tags }
    }
}

// ---------------------------------------------------------------------------
// UniCatalogStore
// ---------------------------------------------------------------------------

#[derive(uniffi::Object)]
pub struct UniCatalogStore {
    inner: CatalogStore,
}

#[uniffi::export]
impl UniCatalogStore {
    #[uniffi::constructor]
    pub fn new(data_dir: String) -> Arc<Self> {
        Arc::new(Self { inner: CatalogStore::new(data_dir) })
    }

    #[uniffi::constructor]
    pub fn new_with_cache(data_dir: String, cache_dir: String) -> Result<Arc<Self>, CatalogError> {
        let inner = CatalogStore::new_with_cache(data_dir, cache_dir)?;
        Ok(Arc::new(Self { inner }))
    }

    pub fn refresh(&self) -> Result<(), CatalogError> {
        self.inner.refresh().map_err(Into::into)
    }

    pub fn rebuild_cache(&self) -> Result<(), CatalogError> {
        self.inner.rebuild_cache().map_err(Into::into)
    }

    pub fn add_item(&self, item: UniNewItem) -> Result<UniItem, CatalogError> {
        NewItem::try_from(item)
            .and_then(|ni| self.inner.add_item(ni).map(UniItem::from).map_err(Into::into))
    }

    pub fn get_item(&self, id_or_name: String) -> Result<Option<UniItem>, CatalogError> {
        self.inner.get_item(&id_or_name)
            .map(|opt| opt.map(UniItem::from))
            .map_err(Into::into)
    }

    pub fn list_items(&self, filter: Option<UniListFilter>) -> Result<Vec<UniItem>, CatalogError> {
        self.inner
            .list_items(filter.map(ListFilter::from))
            .map(|v| v.into_iter().map(UniItem::from).collect())
            .map_err(Into::into)
    }

    pub fn update_item(&self, id: String, updates: UniItemUpdate) -> Result<Option<UniItem>, CatalogError> {
        ItemUpdate::try_from(updates)
            .and_then(|upd| self.inner.update_item(&id, upd).map(|opt| opt.map(UniItem::from)).map_err(Into::into))
    }

    pub fn delete_item(&self, id: String) -> Result<bool, CatalogError> {
        self.inner.delete_item(&id).map_err(Into::into)
    }

    pub fn get_item_fields(&self) -> Result<Vec<String>, CatalogError> {
        self.inner.get_item_fields().map_err(Into::into)
    }

    pub fn search_items(&self, query: String) -> Result<Vec<UniItem>, CatalogError> {
        self.inner
            .search_items(&query)
            .map(|v| v.into_iter().map(UniItem::from).collect())
            .map_err(Into::into)
    }

    pub fn add_attachment(
        &self,
        item_id: String,
        filename: String,
        kind: AttachmentKind,
        data: Vec<u8>,
        label: Option<String>,
    ) -> Result<UniItem, CatalogError> {
        self.inner
            .add_attachment(&item_id, &filename, kind.into(), &data, label)
            .map(UniItem::from)
            .map_err(Into::into)
    }

    pub fn get_attachment(&self, item_id: String, filename: String) -> Result<Vec<u8>, CatalogError> {
        self.inner.get_attachment(&item_id, &filename).map_err(Into::into)
    }

    pub fn delete_attachment(
        &self,
        item_id: String,
        filename: String,
    ) -> Result<Option<UniItem>, CatalogError> {
        self.inner
            .delete_attachment(&item_id, &filename)
            .map(|opt| opt.map(UniItem::from))
            .map_err(Into::into)
    }
}
