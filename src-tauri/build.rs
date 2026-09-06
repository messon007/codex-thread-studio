fn main() {
    verify_typescript_assets();
    tauri_build::build()
}

fn verify_typescript_assets() {
    use sha2::{Digest, Sha256};
    use std::{fs, path::Path};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let manifest = root.join("ui-src/generated.sha256");
    println!("cargo:rerun-if-changed={}", manifest.display());
    println!("cargo:rerun-if-changed={}", root.join("ui-src").display());
    let contents = fs::read_to_string(manifest).expect("Run npm run build:ui first");
    let mut paths = std::collections::HashSet::new();
    for line in contents.lines() {
        let (path, expected) = line.split_once('\t').expect("Invalid UI manifest");
        paths.insert(path);
        let file = root.join(path);
        println!("cargo:rerun-if-changed={}", file.display());
        let content = fs::read_to_string(&file).expect("Missing TypeScript input/output");
        let actual = format!(
            "{:x}",
            Sha256::digest(content.replace("\r\n", "\n").as_bytes())
        );
        assert_eq!(actual, expected, "Stale {path}; run npm run build:ui");
    }
    for entry in fs::read_dir(root.join("ui-src")).unwrap() {
        let name = entry.unwrap().file_name().to_string_lossy().into_owned();
        assert!(
            !name.ends_with(".mts") || paths.contains(format!("ui-src/{name}").as_str()),
            "New TypeScript source {name}; run npm run build:ui"
        );
    }
}
