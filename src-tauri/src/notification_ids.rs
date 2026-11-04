//! Opaque, stable notification IDs; never pass provider identifiers to GIO as C strings.
pub const MAX_IDS: usize = 32;
#[allow(dead_code)] // The gateway validates IDs; the native shell also constructs them.
pub fn tag(backend: &str, thread: &str) -> Option<String> {
    if backend.is_empty()
        || backend.len() > 64
        || thread.is_empty()
        || thread.len() > 256
        || backend.chars().chain(thread.chars()).any(char::is_control)
    {
        return None;
    }
    let mut result = "studio-".to_owned();
    for b in backend.bytes().chain([b':']).chain(thread.bytes()) {
        use std::fmt::Write;
        let _ = write!(result, "{b:02x}");
    }
    Some(result)
}
pub fn valid(value: &str) -> bool {
    value.strip_prefix("studio-").is_some_and(|s| {
        !s.is_empty()
            && s.len() <= 642
            && s.len() % 2 == 0
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}
