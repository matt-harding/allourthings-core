/// Conformance tests against the shared test-fixtures/catalog directory.
///
/// These are the canonical assertions from MANIFEST.json. Any compliant
/// implementation (MCP server, iOS app, Android app) must pass the equivalent
/// of these checks.
use allourthings_core::{CatalogStore, item::ItemUpdate, error::Error};
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test-fixtures/catalog")
}

// ---------------------------------------------------------------------------
// Reading rules
// ---------------------------------------------------------------------------

#[test]
fn reads_exactly_three_valid_items() {
    let store = CatalogStore::new(fixtures_dir());
    let items = store.list_items(None).unwrap();
    assert_eq!(items.len(), 3, "expected 3 readable items (1 malformed must be skipped)");
}

#[test]
fn minimal_item_id() {
    let store = CatalogStore::new(fixtures_dir());
    let item = store.get_item("00000001").unwrap().expect("minimal item should be found");
    assert_eq!(item.id, "00000001");
    assert_eq!(item.name, "Minimal Item");
}

#[test]
fn full_item_fields() {
    let store = CatalogStore::new(fixtures_dir());
    let item = store.get_item("00000002").unwrap().expect("full item should be found");
    assert_eq!(item.id, "00000002");
    assert_eq!(item.name, "Full Item");
    assert_eq!(item.category.as_deref(), Some("electronics"));
    assert_eq!(item.subcategory.as_deref(), Some("large appliance"));
    // purchase_price must be a number, not a string
    assert_eq!(item.purchase_price, Some(649.0));
    // date-only fields must NOT be full datetimes
    assert_eq!(item.purchase_date.as_deref(), Some("2025-02-10"));
    assert_eq!(item.warranty_expires.as_deref(), Some("2027-02-10"));
    let attachments = item.attachments.as_ref().expect("attachments should be present");
    assert_eq!(attachments.len(), 2);
}

#[test]
fn custom_fields_passthrough() {
    let store = CatalogStore::new(fixtures_dir());
    let item = store.get_item("00000003").unwrap().expect("custom fields item should be found");
    assert_eq!(item.id, "00000003");
    assert_eq!(
        item.extra.get("serial_number").and_then(|v| v.as_str()),
        Some("ABC123XYZ")
    );
    assert_eq!(
        item.extra.get("rack_unit").and_then(|v| v.as_i64()),
        Some(2)
    );
    assert_eq!(
        item.extra.get("custom_bool").and_then(|v| v.as_bool()),
        Some(true)
    );
}

#[test]
fn malformed_item_is_silently_skipped() {
    // If the malformed item were included we'd get 4; we assert 3 above.
    // This test documents the intent explicitly.
    let store = CatalogStore::new(fixtures_dir());
    let items = store.list_items(None).unwrap();
    let has_malformed = items.iter().any(|i| i.id == "00000004");
    assert!(!has_malformed, "malformed item must be silently skipped, not surfaced");
}

// ---------------------------------------------------------------------------
// Slug algorithm (STORAGE_SPEC.md §4)
// ---------------------------------------------------------------------------

mod slug_tests {
    use allourthings_core::slug::to_slug;

    macro_rules! slug_case {
        ($name:ident, $input:expr, $expected:expr) => {
            #[test]
            fn $name() {
                assert_eq!(to_slug($input), $expected);
            }
        };
    }

    slug_case!(bosch, "Bosch Washing Machine", "bosch-washing-machine");
    slug_case!(playstation, "PlayStation 5", "playstation-5");
    slug_case!(wool, "100% Wool Blanket", "100-wool-blanket");
    slug_case!(spaces, "  Spaces  ", "spaces");
    slug_case!(ampersand, "A & B -- C", "a-b-c");
    slug_case!(macbook, "MacBook Pro 14\"", "macbook-pro-14");
    slug_case!(
        truncation,
        "A very long name that exceeds fifty characters totally",
        "a-very-long-name-that-exceeds-fifty-characters-tot"
    );
}

// ---------------------------------------------------------------------------
// Write rules
// ---------------------------------------------------------------------------

#[test]
fn add_item_sets_required_fields() {
    use allourthings_core::item::NewItem;
    use tempfile::TempDir;

    let tmp = TempDir::new().unwrap();
    let store = CatalogStore::new(tmp.path());
    let item = store
        .add_item(NewItem { name: "Test Widget".into(), ..Default::default() })
        .unwrap();

    assert_eq!(item.id.len(), 8);
    assert!(item.id.chars().all(|c| c.is_ascii_hexdigit()));
    assert_eq!(item.name, "Test Widget");
    assert!(!item.created_at.is_empty());
    assert!(!item.updated_at.is_empty());
    assert_eq!(item.created_at, item.updated_at, "created_at and updated_at must match on creation");
}

#[test]
fn update_does_not_change_created_at() {
    use allourthings_core::item::NewItem;
    use tempfile::TempDir;

    let tmp = TempDir::new().unwrap();
    let store = CatalogStore::new(tmp.path());
    let original = store
        .add_item(NewItem { name: "My Item".into(), ..Default::default() })
        .unwrap();

    // Sleep >1ms so the update timestamp is guaranteed to differ from created_at
    std::thread::sleep(std::time::Duration::from_millis(2));

    let updated = store
        .update_item(&original.id, ItemUpdate { notes: Some("updated".into()), ..Default::default() })
        .unwrap()
        .expect("item should exist");

    assert_eq!(updated.created_at, original.created_at, "created_at must never change after creation");
    assert_ne!(updated.updated_at, original.updated_at, "updated_at must change on write");
}

#[test]
fn update_renames_directory_on_name_change() {
    use allourthings_core::item::NewItem;
    use tempfile::TempDir;

    let tmp = TempDir::new().unwrap();
    let store = CatalogStore::new(tmp.path());
    let item = store
        .add_item(NewItem { name: "Old Name".into(), ..Default::default() })
        .unwrap();

    store
        .update_item(&item.id, ItemUpdate { name: Some("New Name".into()), ..Default::default() })
        .unwrap();

    // Old directory must be gone, new one must exist with the item still readable
    let refetched = store.get_item(&item.id).unwrap().expect("item must be findable by id after rename");
    assert_eq!(refetched.name, "New Name");
    assert_eq!(refetched.id, item.id, "id must be stable across renames");
}

#[test]
fn get_item_empty_string_returns_none() {
    use allourthings_core::item::NewItem;
    use tempfile::TempDir;

    let tmp = TempDir::new().unwrap();
    let store = CatalogStore::new(tmp.path());
    store.add_item(NewItem { name: "Kettle".into(), ..Default::default() }).unwrap();

    // Empty string must not substring-match every item
    assert!(store.get_item("").unwrap().is_none());
}

#[test]
fn search_items_empty_string_returns_nothing() {
    use allourthings_core::item::NewItem;
    use tempfile::TempDir;

    let tmp = TempDir::new().unwrap();
    let store = CatalogStore::new(tmp.path());
    store.add_item(NewItem { name: "Kettle".into(), ..Default::default() }).unwrap();

    // Empty query must not match every item
    assert!(store.search_items("").unwrap().is_empty());
}

#[test]
fn delete_removes_item() {
    use allourthings_core::item::NewItem;
    use tempfile::TempDir;

    let tmp = TempDir::new().unwrap();
    let store = CatalogStore::new(tmp.path());
    let item = store
        .add_item(NewItem { name: "To Delete".into(), ..Default::default() })
        .unwrap();

    assert!(store.delete_item(&item.id).unwrap());
    assert!(store.get_item(&item.id).unwrap().is_none());
    assert!(!store.delete_item(&item.id).unwrap(), "second delete should return false");
}

#[test]
fn custom_fields_survive_update_roundtrip() {
    use allourthings_core::item::NewItem;
    use serde_json::Value;
    use std::collections::HashMap;
    use tempfile::TempDir;

    let tmp = TempDir::new().unwrap();
    let store = CatalogStore::new(tmp.path());

    let mut extra = HashMap::new();
    extra.insert("serial_number".to_string(), Value::String("XYZ-987".into()));

    let item = store
        .add_item(NewItem { name: "Custom".into(), extra, ..Default::default() })
        .unwrap();

    // Update an unrelated field
    let updated = store
        .update_item(&item.id, ItemUpdate { notes: Some("note".into()), ..Default::default() })
        .unwrap()
        .unwrap();

    assert_eq!(
        updated.extra.get("serial_number").and_then(|v| v.as_str()),
        Some("XYZ-987"),
        "passthrough fields must survive update round-trips"
    );
}

// ---------------------------------------------------------------------------
// list_items filtering
// ---------------------------------------------------------------------------

#[test]
fn list_items_filter_by_category() {
    use allourthings_core::item::{ListFilter, NewItem};
    use tempfile::TempDir;

    let tmp = TempDir::new().unwrap();
    let store = CatalogStore::new(tmp.path());
    store.add_item(NewItem { name: "Laptop".into(), category: Some("Electronics".into()), ..Default::default() }).unwrap();
    store.add_item(NewItem { name: "Hammer".into(), category: Some("Tools".into()), ..Default::default() }).unwrap();
    store.add_item(NewItem { name: "Phone".into(), category: Some("Electronics".into()), ..Default::default() }).unwrap();

    let results = store.list_items(Some(ListFilter { category: Some("Electronics".into()), ..Default::default() })).unwrap();
    assert_eq!(results.len(), 2);
    assert!(results.iter().all(|i| i.category.as_deref() == Some("Electronics")));
}

#[test]
fn list_items_filter_by_subcategory() {
    use allourthings_core::item::{ListFilter, NewItem};
    use tempfile::TempDir;

    let tmp = TempDir::new().unwrap();
    let store = CatalogStore::new(tmp.path());
    store.add_item(NewItem { name: "MacBook".into(), category: Some("Electronics".into()), subcategory: Some("Laptop".into()), ..Default::default() }).unwrap();
    store.add_item(NewItem { name: "iPhone".into(), category: Some("Electronics".into()), subcategory: Some("Phone".into()), ..Default::default() }).unwrap();
    store.add_item(NewItem { name: "Blender".into(), category: Some("Appliances".into()), subcategory: Some("Kitchen".into()), ..Default::default() }).unwrap();

    let results = store.list_items(Some(ListFilter { subcategory: Some("Laptop".into()), ..Default::default() })).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "MacBook");
}

#[test]
fn list_items_filter_by_category_and_subcategory() {
    use allourthings_core::item::{ListFilter, NewItem};
    use tempfile::TempDir;

    let tmp = TempDir::new().unwrap();
    let store = CatalogStore::new(tmp.path());
    store.add_item(NewItem { name: "Drill".into(), category: Some("Tools".into()), subcategory: Some("Power".into()), ..Default::default() }).unwrap();
    store.add_item(NewItem { name: "Hammer".into(), category: Some("Tools".into()), subcategory: Some("Hand".into()), ..Default::default() }).unwrap();
    store.add_item(NewItem { name: "Blender".into(), category: Some("Appliances".into()), subcategory: Some("Power".into()), ..Default::default() }).unwrap();

    let results = store.list_items(Some(ListFilter { category: Some("Tools".into()), subcategory: Some("Power".into()), ..Default::default() })).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "Drill");
}

#[test]
fn list_items_filter_by_tags_and_logic() {
    use allourthings_core::item::{ListFilter, NewItem};
    use tempfile::TempDir;

    let tmp = TempDir::new().unwrap();
    let store = CatalogStore::new(tmp.path());
    store.add_item(NewItem { name: "Item A".into(), tags: Some(vec!["red".into(), "fragile".into()]), ..Default::default() }).unwrap();
    store.add_item(NewItem { name: "Item B".into(), tags: Some(vec!["red".into()]), ..Default::default() }).unwrap();
    store.add_item(NewItem { name: "Item C".into(), tags: Some(vec!["fragile".into()]), ..Default::default() }).unwrap();

    let results = store.list_items(Some(ListFilter { tags: Some(vec!["red".into(), "fragile".into()]), ..Default::default() })).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "Item A");
}

#[test]
fn list_items_filter_via_cache() {
    use allourthings_core::item::{ListFilter, NewItem};
    use tempfile::TempDir;

    let tmp = TempDir::new().unwrap();
    let cache_dir = tmp.path().join("cache");
    let store = CatalogStore::new_with_cache(tmp.path(), &cache_dir).unwrap();

    store.add_item(NewItem { name: "MacBook".into(), category: Some("Electronics".into()), subcategory: Some("Laptop".into()), tags: Some(vec!["portable".into()]), ..Default::default() }).unwrap();
    store.add_item(NewItem { name: "Drill".into(), category: Some("Tools".into()), subcategory: Some("Power".into()), ..Default::default() }).unwrap();

    let by_cat = store.list_items(Some(ListFilter { category: Some("Electronics".into()), ..Default::default() })).unwrap();
    assert_eq!(by_cat.len(), 1);
    assert_eq!(by_cat[0].name, "MacBook");

    let by_sub = store.list_items(Some(ListFilter { subcategory: Some("Power".into()), ..Default::default() })).unwrap();
    assert_eq!(by_sub.len(), 1);
    assert_eq!(by_sub[0].name, "Drill");

    let by_tag = store.list_items(Some(ListFilter { tags: Some(vec!["portable".into()]), ..Default::default() })).unwrap();
    assert_eq!(by_tag.len(), 1);
    assert_eq!(by_tag[0].name, "MacBook");
}

// ---------------------------------------------------------------------------
// Attachment filename validation (path traversal prevention)
// ---------------------------------------------------------------------------

#[test]
fn path_traversal_filename_is_rejected() {
    use allourthings_core::item::{AttachmentType, NewItem};
    use tempfile::TempDir;

    let tmp = TempDir::new().unwrap();
    let store = CatalogStore::new(tmp.path());
    let item = store
        .add_item(NewItem { name: "Attachment Test".into(), ..Default::default() })
        .unwrap();

    let result = store.add_attachment(
        &item.id,
        "../escape",
        AttachmentType::Photo,
        b"malicious",
        None,
    );
    assert!(
        matches!(result, Err(Error::InvalidFilename(_))),
        "filenames with path traversal components must be rejected"
    );
}
