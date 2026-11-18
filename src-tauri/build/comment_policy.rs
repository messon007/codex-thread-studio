//! Studio's direct-edit policy; fixed SDK bytes remain immutable.
use sha2::{Digest, Sha256};
pub const REVISION: &str = "18d122e5ea37cfc754b2d79b58d91334176c87e3";
pub const SOURCE_SHA256: &str = "a58d7527296da985c9ed589f3c7a7338b9d0728e690183655e4e7a8c8a2ff546";
pub const DERIVED_SHA256: &str = "662d3c703bbcd80a114efb72a745ac9c61a91ad7c3a57cf4133f14364fe52733";
fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
pub fn derive(revision: &str, source: &[u8]) -> Result<Vec<u8>, String> {
    if revision != REVISION || hash(source) != SOURCE_SHA256 {
        return Err("Review Studio comment policy for this SDK version".into());
    }
    let mut result = std::str::from_utf8(source)
        .map_err(|e| e.to_string())?
        .to_owned();
    for (before, after) in [
        ("    hover = setTimeout(() => showChoices(hits(atPointer(event,frame))),400)", "    // Studio: direct editing only; no hover chooser."),
        ("      if(ids.length===1){clearHover();openNote(ids[0])}else showChoices(current)", "      clearHover();openNote(ids.length===1 ? ids[0] : null) // Legacy overlaps remain accessible in the host list."),
    ] {
        if result.matches(before).count() != 1 { return Err("Comment policy anchor changed".into()); }
        result = result.replacen(before, after, 1);
    }
    if hash(result.as_bytes()) != DERIVED_SHA256 {
        return Err("Comment policy output changed".into());
    }
    Ok(result.into_bytes())
}
#[allow(dead_code)]
pub fn generate() {
    use std::{fs, path::Path};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    for name in [
        "scripts/marknoria-release.json",
        "crates/marknoria/web/comment-marks.mjs",
        "src-tauri/build/comment_policy.rs",
    ] {
        println!("cargo:rerun-if-changed={}", root.join(name).display());
    }
    let pin = fs::read_to_string(root.join("scripts/marknoria-release.json")).unwrap();
    assert!(
        pin.contains(&format!("\"revision\": \"{REVISION}\"")),
        "Review comment policy after SDK update"
    );
    let source = fs::read(root.join("crates/marknoria/web/comment-marks.mjs")).unwrap();
    let derived = derive(REVISION, &source).expect("Invalid Studio comment policy");
    fs::write(
        Path::new(&std::env::var_os("OUT_DIR").unwrap()).join("studio-comment-marks.mjs"),
        derived,
    )
    .unwrap();
}
