# allourthings-core

Rust storage library for [AllOurThings](https://allourthings.io) — an AI-powered inventory system.


## Storage format

Each item is a directory containing a single JSON file:

```
<data-dir>/items/<slug>-<id>/item.json
```

The directory name is `<slug>-<id>` where `<slug>` is a URL-safe lowercase version of the item name (max 50 chars) and `<id>` is an 8-character lowercase hex string. 

Required fields: `id`, `name`, `created_at`, `updated_at`. Well-known optional fields include `category`, `subcategory`, `tags`, and `attachments`. Any additional fields are preserved and round-trip without modification.

Attachment files (PDFs, images, etc.) are stored alongside `item.json` in the same directory.


## Query cache

`CatalogStore` supports an optional SQLite query cache for efficient filtering.

The cache is write-through — every mutation (`add_item`, `update_item`, `delete_item`, and the attachment operations) updates the cache immediately. Call `refresh()` to sync changes written externally (e.g. by another process or a mobile app), and `rebuild_cache()` to fully repopulate from disk if the cache becomes corrupt.

| Platform | Default location |
|---|---|
| macOS | `~/Library/Caches/allourthings/<hash>/` |
| Windows | `%LOCALAPPDATA%\allourthings\<hash>\` |
| Linux | `~/.cache/allourthings/<hash>/` |

The `<hash>` is a SHA-256 of the data directory path, so multiple vaults each get their own isolated cache.

The cache schema indexes `category`, `subcategory`, and `tags` for SQL filtering, and stores the full item as a JSON blob for everything else. All three are optional on any item — items missing them are stored and returned normally, they just won't appear when those specific filters are applied. Keeping the indexed columns to only those actually queried means new well-known fields can be added without any schema migration.

## Running tests

```bash
cargo test
```

This runs two test suites:

- **Unit tests** (in `src/`) — cover the item schema, slug generation, ID generation, storage CRUD, attachments, and cache write-through behaviour
- **Conformance tests** (`tests/conformance.rs`) — assert against the shared fixtures in `test-fixtures/catalog/`. These define the canonical storage format that all AllOurThings clients (MCP server, iOS app, etc.) must honour


## License

MIT
