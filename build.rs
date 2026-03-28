extern crate napi_build;

fn main() {
    // Only set up napi linker flags when building the native addon
    if std::env::var("CARGO_FEATURE_NAPI").is_ok() {
        napi_build::setup();
    }
}
