//! Bounded inline image validation; no URL fetching or filesystem access.
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};

pub const MAX_BODY: usize = 32 * 1024 * 1024;
const MAX_FILE: usize = 10 * 1024 * 1024;
const MAX_TOTAL: usize = 20 * 1024 * 1024;

pub fn mime(url: &str) -> Result<&str, String> {
    let (prefix, _) = url.split_once(',').ok_or("Invalid inline image")?;
    prefix
        .strip_prefix("data:")
        .and_then(|prefix| prefix.strip_suffix(";base64"))
        .filter(|mime| {
            matches!(
                *mime,
                "image/png" | "image/jpeg" | "image/webp" | "image/gif"
            )
        })
        .ok_or_else(|| "Only inline PNG, JPEG, WebP and GIF images can be routed".into())
}
pub fn validate(images: &[String]) -> Result<(), String> {
    if images.len() > 4 {
        return Err("A routed message supports at most four images".into());
    }
    let mut total = 0;
    for url in images {
        let declared = mime(url)?;
        let encoded = url.split_once(',').unwrap().1;
        if encoded.len() > (MAX_FILE + 2) / 3 * 4 {
            return Err("Each routed image must be 10 MiB or smaller".into());
        }
        let bytes = STANDARD
            .decode(encoded)
            .map_err(|_| "Invalid inline image encoding")?;
        total += bytes.len();
        if bytes.is_empty() || bytes.len() > MAX_FILE || total > MAX_TOTAL {
            return Err("Routed images exceed the 20 MiB message budget".into());
        }
        let detected = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            "image/png"
        } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
            "image/jpeg"
        } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
            "image/gif"
        } else if bytes.get(..4) == Some(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
            "image/webp"
        } else {
            return Err("Unsupported image signature".into());
        };
        if detected != declared {
            return Err("Inline image type does not match its contents".into());
        }
    }
    Ok(())
}
pub fn append(params: &mut Value, images: &[String]) {
    if let Some(input) = params["input"].as_array_mut() {
        input.extend(images.iter().map(|url| json!({"type":"image","url":url})));
    }
}
pub fn opencode_part(input: &Value) -> Result<Value, String> {
    match input["type"].as_str() {
        Some("text") => Ok(json!({"type":"text","text":input["text"]})),
        Some("image") => {
            let url = input["url"].as_str().ok_or("Missing image URL")?;
            let mime = mime(url)?;
            Ok(json!({"type":"file","mime":mime,"url":url}))
        }
        Some("file")
            if input["url"]
                .as_str()
                .is_some_and(|url| url.starts_with("file://"))
                && input["mime"] == "text/plain" =>
        {
            Ok(input.clone())
        }
        _ => Err("Unsupported Router input".into()),
    }
}
pub fn same_part(a: &Value, b: &Value) -> bool {
    a["type"] == b["type"]
        && match b["type"].as_str() {
            Some("text") => a["text"] == b["text"],
            Some("file") => a["url"] == b["url"] && a["mime"] == b["mime"],
            _ => false,
        }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inline_images_are_bounded_typed_and_echo_identity_ignores_only_server_metadata() {
        let url = format!("data:image/gif;base64,{}", STANDARD.encode(b"GIF89atest"));
        validate(&[url.clone()]).unwrap();
        assert!(validate(&vec![url.clone(); 5]).is_err());
        for bad in [
            "https://example.test/a.png".to_owned(),
            "file:///tmp/a.png".into(),
            url.replace("image/gif", "image/png"),
            "data:image/gif;base64,%%%".into(),
        ] {
            assert!(validate(&[bad]).is_err());
        }
        assert!(validate(&[format!(
            "data:image/gif;base64,{}",
            "a".repeat((MAX_FILE + 2) / 3 * 4 + 4)
        )])
        .is_err());
        let part = opencode_part(&json!({"type":"image","url":url})).unwrap();
        let mut echo = part.clone();
        echo["id"] = "server-owned".into();
        assert!(same_part(&echo, &part));
        echo["url"] = "data:image/gif;base64,R0lGODlh".into();
        assert!(!same_part(&echo, &part));
    }
}
