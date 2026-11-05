//! Shared, embedded diagram-engine and sanitization assets.
pub const ENGINE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/diagrams/engine.mjs"
));
pub const RENDER_PAGE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/diagrams/render.html"
));
pub const PURIFY: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/vendor/purify.mjs"
));
pub const MERMAID: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/vendor/mermaid.min.js"
));
pub const PLANTUML: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/vendor/plantuml.js"
));
pub const PLANTUML_VIZ: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/vendor/plantuml-viz-global.js"
));
pub const D2: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/vendor/d2.js"));
