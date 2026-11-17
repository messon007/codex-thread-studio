//! Temporary, fixed-asset derivation for MarkNoria #39. Remove on upstream adoption.
use super::reader_compat::{hash, REVISION};
pub const SOURCE_SHA256: &str = "69afa7035c008fbcb865f8712ae47a7f91c0ae8e2100eb5d210701e6249e9b8b";
pub const DERIVED_SHA256: &str = "5c6915f1e0c7da9b4622a3e5d3eb482d219c06cac899d9926fef45949a95ee83";
const HELPER: &str = include_str!("../compat/comment-click.mjs");
const ANCHOR: &str = "  function clearHover()";
const BEFORE: [&str; 2] = ["  function clearHover() { clearTimeout(hover); clearTimeout(hideTimer); popup.hidden=true }","    realm.addEventListener('pointerdown',event=>{if(!popup.contains(event.target))clearHover()},{signal:controller.signal})"];
const AFTER: [&str; 2] = ["  function clearHover() { clearTimeout(activation); pressed=null; clearTimeout(hover); clearTimeout(hideTimer); popup.hidden=true }","    realm.addEventListener('pointerdown',event=>pressMark(event,frame),{signal:controller.signal})\n    realm.addEventListener('click',event=>clickMark(event,frame),{signal:controller.signal})\n    realm.addEventListener('pointercancel',()=>clearHover(),{signal:controller.signal})\n    realm.addEventListener('selectionchange',()=>{if(hasSelection(realm))clearHover()},{signal:controller.signal})\n    realm.defaultView?.addEventListener('blur',()=>clearHover(),{signal:controller.signal})"];
fn replace_once(source: String, before: &str, after: &str) -> Result<String, String> {
    if source.matches(before).count() != 1 {
        return Err("MarkNoria #39 compatibility anchor changed".into());
    }
    Ok(source.replacen(before, after, 1))
}
pub fn derive(revision: &str, source: &[u8]) -> Result<Vec<u8>, String> {
    if revision != REVISION || hash(source) != SOURCE_SHA256 {
        return Err("MarkNoria #39 source changed: remove or review compatibility layer".into());
    }
    let source = std::str::from_utf8(source)
        .map_err(|e| e.to_string())?
        .to_owned();
    let helper = HELPER.replace("\r\n", "\n");
    let mut derived = replace_once(source, ANCHOR, &format!("{helper}{ANCHOR}"))?;
    for (before, after) in BEFORE.iter().zip(AFTER) {
        derived = replace_once(derived, before, after)?;
    }
    if hash(derived.as_bytes()) != DERIVED_SHA256 {
        return Err("MarkNoria #39 derived output changed".into());
    }
    Ok(derived.into_bytes())
}
#[cfg(test)]
pub fn restore(derived: &[u8]) -> Result<Vec<u8>, String> {
    let mut source = std::str::from_utf8(derived)
        .map_err(|e| e.to_string())?
        .to_owned();
    for (before, after) in BEFORE.iter().zip(AFTER) {
        source = replace_once(source, after, before)?;
    }
    let helper = HELPER.replace("\r\n", "\n");
    Ok(replace_once(source, &format!("{helper}{ANCHOR}"), ANCHOR)?.into_bytes())
}
#[allow(dead_code)]
pub fn generate() {
    use std::{fs, path::Path};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    for path in [
        "crates/marknoria/web/comment-marks.mjs",
        "src-tauri/compat/comment-click.mjs",
        "src-tauri/build/comment_compat.rs",
    ] {
        println!("cargo:rerun-if-changed={}", root.join(path).display());
    }
    // reader_compat::generate checks the shared immutable SDK pin before this call.
    let source = fs::read(root.join("crates/marknoria/web/comment-marks.mjs")).unwrap();
    let derived = derive(REVISION, &source).expect("Comment compatibility validation failed");
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    fs::write(out.join("comment-marks-compat.mjs"), derived).unwrap();
}
