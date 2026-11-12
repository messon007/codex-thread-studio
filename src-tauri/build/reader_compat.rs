//! Single, removable compatibility derivation for MarkNoria issue #38.
//! This is deliberately not a general vendor patch loader.
use sha2::{Digest, Sha256};

pub const REVISION: &str = "0a60f6951250e4ee50036c20e10e4283c4cd9079";
pub const SOURCE_SHA256: &str = "5a3ce895a3d7e0b01ded2d0883b686b602c39bc33ce742b3b20de4978c099d86";
pub const DERIVED_SHA256: &str = "4f35a6ea75c7d0bf13c8805d1f42ac75cc872edc0638a5f91504f27d03b3392a";
const HELPER: &str = include_str!("../compat/remeasure-mounted-pages.mjs");
const ANCHOR: &str = "async function refreshTypography(anchor) {";
const BEFORE: [&str; 2] = [
    "    virtualizer?.measure()\n    await previewLine(anchor.preview)",
    "    virtualizer.measure()\n    if (item) jump(anchor, null, row)",
];
const AFTER: [&str; 2] = [
    "    remeasureMountedPages()\n    await previewLine(anchor.preview)",
    "    remeasureMountedPages()\n    if (item) jump(anchor, null, row)",
];

pub fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn replace_once(source: String, before: &str, after: &str) -> Result<String, String> {
    if source.matches(before).count() != 1 {
        return Err(
            "MarkNoria #38 compatibility anchor changed; review or remove the compatibility layer"
                .into(),
        );
    }
    Ok(source.replacen(before, after, 1))
}

pub fn derive(revision: &str, source: &[u8]) -> Result<Vec<u8>, String> {
    if revision != REVISION || hash(source) != SOURCE_SHA256 {
        return Err("MarkNoria #38 compatibility source changed; adopt the upstream fix and remove this layer, or explicitly review its new scope".into());
    }
    let source = std::str::from_utf8(source)
        .map_err(|error| error.to_string())?
        .to_owned();
    let helper = HELPER.replace("\r\n", "\n");
    let mut derived = replace_once(source, ANCHOR, &format!("{helper}{ANCHOR}"))?;
    for (before, after) in BEFORE.iter().zip(AFTER) {
        derived = replace_once(derived, before, after)?;
    }
    if hash(derived.as_bytes()) != DERIVED_SHA256 {
        return Err("MarkNoria #38 compatibility output changed; review its diff and tests".into());
    }
    Ok(derived.into_bytes())
}

#[cfg(test)]
pub fn restore(derived: &[u8]) -> Result<Vec<u8>, String> {
    let mut source = std::str::from_utf8(derived)
        .map_err(|error| error.to_string())?
        .to_owned();
    for (before, after) in BEFORE.iter().zip(AFTER) {
        source = replace_once(source, after, before)?;
    }
    let helper = HELPER.replace("\r\n", "\n");
    source = replace_once(source, &format!("{helper}{ANCHOR}"), ANCHOR)?;
    Ok(source.into_bytes())
}

#[allow(dead_code)] // Also compiled into unit tests to exercise the exact derivation.
pub fn generate() {
    use std::{fs, path::Path};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let source = root.join("crates/marknoria/web/reader.mjs");
    let pin = root.join("scripts/marknoria-release.json");
    for path in [
        &source,
        &pin,
        &root.join("src-tauri/compat/remeasure-mounted-pages.mjs"),
        &root.join("src-tauri/build/reader_compat.rs"),
    ] {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    let pin = fs::read_to_string(pin).expect("Missing fixed reader pin");
    assert_eq!(
        pin.matches(&format!("\"revision\": \"{REVISION}\""))
            .count(),
        1,
        "Fixed SDK changed: remove/review the MarkNoria #38 compatibility layer"
    );
    let derived = derive(
        REVISION,
        &fs::read(source).expect("Missing fixed reader source"),
    )
    .expect("Reader compatibility validation failed");
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    fs::write(out.join("reader-compat.mjs"), derived).unwrap();
    fs::write(out.join("reader-compat.json"), format!("{{\"issue\":\"https://github.com/messon007/marknoria/issues/38\",\"revision\":\"{REVISION}\",\"sourceSha256\":\"{SOURCE_SHA256}\",\"derivedSha256\":\"{DERIVED_SHA256}\"}}\n")).unwrap();
}
