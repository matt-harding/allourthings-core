# allourthings-core

Rust storage library for [All Our Things](https://allourthings.io) — an AI-powered household inventory system.

Handles all reading and writing of catalog data. Shared across clients via platform-specific bindings:

| Client | Binding |
|---|---|
| MCP server (TypeScript) | napi-rs native addon |
| iOS (SwiftUI) | UniFFI xcframework _(planned)_ |
| Android (Kotlin) | UniFFI AAR _(planned)_ |

## Usage

```bash
cargo test        # run all 25 tests
bun run build     # build the .node native addon for the MCP server
```

## Storage format

Each item is a directory containing a single JSON file:

```
<data-dir>/items/<slug>-<id>/item.json
```

Required fields: `id`, `name`, `created_at`, `updated_at`. All other fields are optional. Custom fields are preserved as-is and round-trip without modification.

See [STORAGE_SPEC.md](https://github.com/matt-harding/AllOurThings/blob/main/STORAGE_SPEC.md) for the full specification.

## License

MIT
