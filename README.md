# allourthings-core

Shared Rust storage library for [All Our Things](https://allourthings.io) — an AI-powered household inventory system.

The core handles all reading and writing of catalog data. It is the single reference implementation of the [storage spec](https://github.com/matt-harding/AllOurThings/blob/main/STORAGE_SPEC.md), shared across all clients:

| Consumer | How it links |
|---|---|
| MCP server (TypeScript) | napi-rs `.node` native addon |
| iOS app (SwiftUI) | UniFFI-generated xcframework _(planned)_ |
| Android app (Kotlin) | UniFFI-generated AAR _(planned)_ |

## Repository layout

```
src/
  lib.rs            Public re-exports
  id.rs             generate_id() — 4 random bytes, hex-encoded (8 chars)
  slug.rs           to_slug() — canonical spec algorithm
  item.rs           Item, NewItem, ItemUpdate, Attachment structs
  error.rs          Error enum (Io, Json, NotFound)
  storage.rs        CatalogStore — full CRUD + search
  napi_bindings.rs  JsCatalogStore — napi-rs bindings (feature-gated)
tests/
  conformance.rs    17 conformance tests against test-fixtures/
test-fixtures/
  catalog/          Static catalog read by conformance tests
  MANIFEST.json     Expected results — shared with MCP server tests
build.rs            napi linker setup (only active with --features napi)
```

## Running tests

```bash
cargo test
```

25 tests: 8 unit tests across `id`, `slug`, and `item` modules, plus 17 conformance tests in `tests/conformance.rs` that read from `test-fixtures/catalog/`.

## Key design decisions

**Passthrough fields** — `Item` uses `#[serde(flatten)] extra: HashMap<String, Value>`. Any field not in the known schema round-trips transparently through JSON without modification. This is intentional: users can add custom fields and they are never lost.

**Atomic writes** — items are written to `item.json.tmp` then renamed into place. On the same filesystem, `rename()` is atomic, so a crash mid-write never leaves a corrupt file.

**No `chrono` dependency** — timestamps are formatted with a manual epoch-to-calendar calculation in `storage.rs`. Swap it for `chrono` if it becomes a problem.

**Directory-per-item layout** — each item lives at `<data-dir>/items/<slug>-<id>/item.json`. Malformed files are silently skipped on read. The slug and ID are both in the directory name so items are human-browsable in Finder/Explorer.

## Building the native addon (for the MCP server)

Requires `@napi-rs/cli`:

```bash
bun install          # installs @napi-rs/cli
bun run build        # release build → allourthings-core.darwin-arm64.node + index.js
bun run build:debug  # faster, unoptimised
```

The generated `.node` file, `index.js`, and `index.d.ts` are gitignored — they are build artifacts. Run `bun run build` after any Rust changes before testing the MCP server.

## Using in the MCP server (local development)

The MCP server references this package via a `file:` path dependency:

```json
"allourthings-core": "file:../../../allourthings-core"
```

After running `bun run build` here, run `bun install` in `AllOurThings/` to pick up changes.

## Storage format

Each item is a directory:

```
<data-dir>/items/<slug>-<id>/item.json
```

Example:
```
~/Documents/AllOurThings/items/bosch-washing-machine-3fa2c1b4/item.json
```

`item.json` is pretty-printed JSON. Required fields: `id`, `name`, `created_at`, `updated_at`. All other fields are optional. Unknown fields are preserved as-is.

## License

MIT
