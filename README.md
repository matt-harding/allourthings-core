# allourthings-core

Rust storage library for [AllOurThings](https://allourthings.io) — an AI-powered household inventory system.

Shared across clients via platform-specific bindings:

| Consumer | Distribution |
|---|---|
| MCP server (Node.js) | npm — `@allourthings/core` |
| iOS app | Swift Package Manager — binary XCFramework |
| Android app *(planned)* | crates.io — `allourthings-core` |

## Storage format

Each item is a directory containing a single JSON file:

```
<data-dir>/items/<slug>-<id>/item.json
```

Required fields: `id`, `name`, `created_at`, `updated_at`. All other fields are optional and round-trip without modification.

## License

MIT
