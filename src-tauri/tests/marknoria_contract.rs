// Execute the exact public SDK fixtures in Studio's normal platform test matrix.
// Keep the reviewed vendor sources byte-for-byte; they own their formatting.
#[rustfmt::skip]
#[path = "../../crates/marknoria/tests/reader_api.rs"]
mod public_api;
#[rustfmt::skip]
#[path = "../../crates/marknoria/tests/markdown_profile.rs"]
mod markdown_profile;
