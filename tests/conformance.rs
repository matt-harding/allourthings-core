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

// ---------------------------------------------------------------------------
// Write validation (validation-report F2, F3, F11, F12, F14, F20)
// ---------------------------------------------------------------------------

mod write_rules {
    use super::*;
    use allourthings_core::item::{AttachmentType, NewItem};
    use serde_json::Value;
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn store() -> (CatalogStore, TempDir) {
        let tmp = TempDir::new().unwrap();
        (CatalogStore::new(tmp.path()), tmp)
    }

    fn named(name: &str) -> NewItem {
        NewItem { name: name.into(), ..Default::default() }
    }

    #[test]
    fn caller_supplied_existing_id_is_rejected() {
        let (store, _t) = store();
        let a = store.add_item(named("Original")).unwrap();
        let result = store.add_item(NewItem { id: Some(a.id.clone()), ..named("Impostor") });
        assert!(matches!(result, Err(Error::DuplicateId(_))));
        assert_eq!(store.list_items(None).unwrap().len(), 1);
    }

    #[test]
    fn caller_supplied_malformed_id_is_rejected() {
        let (store, _t) = store();
        let result = store.add_item(NewItem { id: Some("../x".into()), ..named("Bad") });
        assert!(matches!(result, Err(Error::Validation(_))));
    }

    #[test]
    fn system_fields_in_custom_fields_are_rejected() {
        let (store, tmp) = store();
        let item = store.add_item(named("Kettle")).unwrap();
        for key in ["id", "created_at", "updated_at"] {
            let mut extra = HashMap::new();
            extra.insert(key.to_string(), Value::String("x".into()));
            let result = store.update_item(&item.id, ItemUpdate { extra, ..Default::default() });
            assert!(matches!(result, Err(Error::Validation(_))), "{key} must be rejected");
        }
        let raw = std::fs::read_to_string(
            std::fs::read_dir(tmp.path().join("items")).unwrap().next().unwrap().unwrap().path().join("item.json"),
        )
        .unwrap();
        assert_eq!(raw.matches("\"id\"").count(), 1, "item.json must never contain duplicate keys");
    }

    #[test]
    fn empty_name_is_rejected_and_names_are_trimmed() {
        let (store, _t) = store();
        assert!(matches!(store.add_item(named("   ")), Err(Error::Validation(_))));
        assert_eq!(store.add_item(named("  Lamp  ")).unwrap().name, "Lamp");
    }

    #[test]
    fn invalid_dates_and_prices_are_rejected() {
        let (store, _t) = store();
        let bad_date = NewItem { purchase_date: Some("yesterday".into()), ..named("A") };
        assert!(matches!(store.add_item(bad_date), Err(Error::Validation(_))));
        let bad_warranty = NewItem { warranty_expires: Some("2026-13-01".into()), ..named("B") };
        assert!(matches!(store.add_item(bad_warranty), Err(Error::Validation(_))));
        let bad_price = NewItem { purchase_price: Some(f64::INFINITY), ..named("C") };
        assert!(matches!(store.add_item(bad_price), Err(Error::Validation(_))));
        let negative = NewItem { purchase_price: Some(-1.0), ..named("D") };
        assert!(matches!(store.add_item(negative), Err(Error::Validation(_))));

        let ok = NewItem {
            purchase_date: Some("2025-02-10".into()),
            warranty_expires: Some("2027-02-10T00:00:00Z".into()),
            purchase_price: Some(0.0),
            ..named("E")
        };
        assert!(store.add_item(ok).is_ok());
    }

    #[test]
    fn duplicate_names_are_rejected_on_add_and_rename() {
        let (store, _t) = store();
        let a = store.add_item(named("Dishwasher")).unwrap();
        assert!(matches!(store.add_item(named(" dishwasher ")), Err(Error::DuplicateName { .. })));

        let b = store.add_item(named("Dryer")).unwrap();
        let rename = ItemUpdate { name: Some("DISHWASHER".into()), ..Default::default() };
        assert!(matches!(store.update_item(&b.id, rename), Err(Error::DuplicateName { .. })));

        // Re-saving an item under its own name (as the iOS app does) is fine.
        let same = ItemUpdate { name: Some("Dishwasher".into()), ..Default::default() };
        assert!(store.update_item(&a.id, same).unwrap().is_some());
    }

    #[test]
    fn ambiguous_name_lookup_is_an_error() {
        let (store, _t) = store();
        store.add_item(named("Bosch Dishwasher")).unwrap();
        store.add_item(named("Bosch Drill")).unwrap();
        assert!(matches!(store.get_item("bosch"), Err(Error::Ambiguous { ref ids, .. }) if ids.len() == 2));
        assert_eq!(store.get_item("bosch drill").unwrap().unwrap().name, "Bosch Drill");
    }

    #[test]
    fn fields_can_be_cleared() {
        let (store, _t) = store();
        let mut extra = HashMap::new();
        extra.insert("serial_number".to_string(), Value::String("X1".into()));
        let item = store
            .add_item(NewItem { notes: Some("n".into()), extra, ..named("Router") })
            .unwrap();
        let updated = store
            .update_item(
                &item.id,
                ItemUpdate { clear: vec!["notes".into(), "serial_number".into()], ..Default::default() },
            )
            .unwrap()
            .unwrap();
        assert!(updated.notes.is_none());
        assert!(!updated.extra.contains_key("serial_number"));

        let clear_name = ItemUpdate { clear: vec!["name".into()], ..Default::default() };
        assert!(matches!(store.update_item(&item.id, clear_name), Err(Error::Validation(_))));
    }

    #[test]
    fn deleting_a_missing_attachment_is_an_error() {
        let (store, _t) = store();
        let item = store.add_item(named("Camera")).unwrap();
        let result = store.delete_attachment(&item.id, "nope.pdf");
        assert!(matches!(result, Err(Error::AttachmentNotFound { .. })));
        assert!(matches!(store.get_attachment(&item.id, "nope.pdf"), Err(Error::AttachmentNotFound { .. })));
    }

    #[test]
    fn item_metadata_file_cannot_be_used_as_an_attachment() {
        let (store, _t) = store();
        let item = store.add_item(named("Fridge")).unwrap();
        for name in ["item.json", "ITEM.JSON", "item.json.tmp"] {
            let added = store.add_attachment(&item.id, name, AttachmentType::Other, b"x", None);
            assert!(matches!(added, Err(Error::InvalidFilename(_))), "{name} must be rejected");
            assert!(matches!(store.get_attachment(&item.id, name), Err(Error::InvalidFilename(_))));
            assert!(matches!(store.delete_attachment(&item.id, name), Err(Error::InvalidFilename(_))));
        }
        assert_eq!(store.get_item(&item.id).unwrap().unwrap().name, "Fridge");
    }

    #[test]
    fn replacing_an_attachment_keeps_its_label() {
        let (store, _t) = store();
        let item = store.add_item(named("Oven")).unwrap();
        store.add_attachment(&item.id, "m.pdf", AttachmentType::Manual, b"v1", Some("Manual".into())).unwrap();
        let updated = store.add_attachment(&item.id, "m.pdf", AttachmentType::Manual, b"v2", None).unwrap();
        assert_eq!(updated.attachments.unwrap()[0].label.as_deref(), Some("Manual"));
    }
}

// ---------------------------------------------------------------------------
// Search (validation-report F9)
// ---------------------------------------------------------------------------

mod search {
    use super::*;
    use allourthings_core::item::NewItem;
    use tempfile::TempDir;

    fn seeded(with_cache: bool) -> (CatalogStore, TempDir) {
        let tmp = TempDir::new().unwrap();
        let store = if with_cache {
            CatalogStore::new_with_cache(tmp.path(), tmp.path().join("cache")).unwrap()
        } else {
            CatalogStore::new(tmp.path())
        };
        store
            .add_item(NewItem {
                name: "Dishwasher".into(),
                brand: Some("Bosch".into()),
                notes: Some("50% off in sale".into()),
                tags: Some(vec!["kitchen".into()]),
                ..Default::default()
            })
            .unwrap();
        store
            .add_item(NewItem { name: "Kettle".into(), brand: Some("Smeg".into()), ..Default::default() })
            .unwrap();
        (store, tmp)
    }

    fn names(store: &CatalogStore, q: &str) -> Vec<String> {
        let mut n: Vec<String> = store.search_items(q).unwrap().into_iter().map(|i| i.name).collect();
        n.sort();
        n
    }

    #[test]
    fn search_matches_values_not_keys_or_wildcards() {
        for with_cache in [false, true] {
            let (store, _t) = seeded(with_cache);
            assert!(names(&store, "name").is_empty(), "field names must not match");
            assert!(names(&store, "created").is_empty());
            assert!(names(&store, "\"").is_empty(), "JSON punctuation must not match");
            assert!(names(&store, "_").is_empty(), "SQL wildcard must not match everything");
            assert_eq!(names(&store, "%"), vec!["Dishwasher"], "% is literal");
            assert_eq!(names(&store, "bosch kitchen"), vec!["Dishwasher"], "all words must match");
            assert_eq!(names(&store, "SMEG"), vec!["Kettle"]);
            assert!(names(&store, "   ").is_empty());
        }
    }
}
