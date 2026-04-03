# allourthings-core

Rust storage library for [All Our Things](https://allourthings.io) — an AI-powered household inventory system.

Handles all reading and writing of catalog data. Shared across clients via platform-specific bindings:

- **MCP server** — consumed as an npm package (`@allourthings/core`) via [napi-rs](https://napi.rs/) pre-built native addons
- **iOS app** — consumed as a Swift Package Manager binary target via [UniFFI](https://mozilla.github.io/uniffi-rs/) XCFramework

## Usage

```bash
cargo test        # run all tests
bun run build     # build the .node native addon for the MCP server
```

## iOS distribution

### Local development

```bash
./scripts/build-ios.sh          # release build (default)
./scripts/build-ios.sh --debug  # debug build
```

This compiles the crate for all iOS targets, generates Swift bindings via UniFFI, and assembles a static XCFramework at `build/allourthings_core.xcframework`. The generated Swift bindings are committed to `Sources/AllourthingsCore/allourthings_core.swift`.

In the iOS project, add `../allourthings-core` as a local Swift Package Manager dependency — Xcode resolves the XCFramework automatically.

### Published releases

Tagging a version (e.g. `git tag v0.1.8 && git push origin v0.1.8`) triggers `.github/workflows/release-ios.yml`, which:

1. Builds the XCFramework for all iOS targets
2. Zips it and computes the SPM checksum
3. Patches `Package.swift` with the remote URL and checksum
4. Commits and force-moves the tag, then creates a GitHub release with the zip attached

iOS consumers can then depend on this repo via SPM without needing a Rust toolchain.

## Storage format

Each item is a directory containing a single JSON file:

```
<data-dir>/items/<slug>-<id>/item.json
```

Required fields: `id`, `name`, `created_at`, `updated_at`. All other fields are optional. Custom fields are preserved as-is and round-trip without modification.

## License

MIT
