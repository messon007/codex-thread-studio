//! Shared bounded session-directory rules for native lists and Router catalogs.
pub fn validate(hidden: &[String], ignore: &[String]) -> Result<(), String> {
    if hidden.len() > 256
        || ignore.len() > 512
        || hidden.iter().any(|p| p.trim().is_empty())
        || hidden
            .iter()
            .chain(ignore)
            .any(|p| p.len() > 4096 || p.chars().any(char::is_control))
    {
        return Err("Use at most 256 hidden folders and 512 ignore rules, each at most 4096 bytes without control characters".into());
    }
    Ok(())
}
fn web_space(c: char) -> bool {
    matches!(c,'\u{0009}'..='\u{000d}'|'\u{0020}'|'\u{00a0}'|'\u{1680}'|'\u{2000}'..='\u{200a}'|'\u{2028}'|'\u{2029}'|'\u{202f}'|'\u{205f}'|'\u{3000}'|'\u{feff}')
}
fn path(value: &str) -> String {
    let raw = value.trim_matches(web_space).replace('\\', "/");
    let prefix = if raw.starts_with("//") { 2 } else { 0 };
    let mut result = raw[..prefix].to_owned();
    let mut slash = false;
    for c in raw[prefix..].chars() {
        if c != '/' || !slash {
            result.push(c);
        }
        slash = c == '/';
    }
    if result.len() > 1 {
        result.truncate(result.trim_end_matches('/').len());
    }
    result
}
fn windows(value: &str) -> bool {
    value.starts_with("//")
        || (value
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphabetic)
            && value.as_bytes().get(1) == Some(&b':')
            && value.as_bytes().get(2) == Some(&b'/'))
}
// Translate the browser's simple glob classes, escaping Rust's additional set
// operators. Invalid ranges are ignored as in the browser, never reinterpreted
// as regex set intersection/subtraction.
fn class(content: &[char]) -> Option<String> {
    let negated = content.first().is_some_and(|c| matches!(c, '!' | '^'));
    let content = if negated { &content[1..] } else { content };
    if content.is_empty() {
        return negated.then(|| "(?s:.)".into());
    }
    let mut output = if negated { "[^" } else { "[" }.to_owned();
    let mut i = 0;
    while i < content.len() {
        if i + 2 < content.len() && content[i + 1] == '-' {
            if content[i] > content[i + 2] {
                return None;
            }
            output.push_str(&regex::escape(&content[i].to_string()));
            output.push('-');
            output.push_str(&regex::escape(&content[i + 2].to_string()));
            i += 3;
        } else {
            output.push_str(&regex::escape(&content[i].to_string()));
            i += 1;
        }
    }
    output.push(']');
    Some(output)
}
#[derive(Clone, Debug)]
pub struct Filter {
    hidden: Vec<String>,
    rules: regex::RegexSet,
    negated: Vec<bool>,
}
impl Filter {
    pub fn new(hidden: &[String], ignore: &[String]) -> Result<Self, String> {
        validate(hidden, ignore)?;
        let mut expressions = Vec::new();
        let mut source_bytes = 0usize;
        let mut negated = Vec::new();
        for raw in ignore {
            let raw = raw.trim_end_matches(web_space);
            if raw.is_empty() || raw.starts_with('#') {
                continue;
            }
            let (allow, raw) = if let Some(raw) = raw.strip_prefix('!') {
                (true, raw)
            } else {
                (
                    false,
                    raw.strip_prefix('\\')
                        .filter(|_| raw.starts_with("\\!") || raw.starts_with("\\#"))
                        .unwrap_or(raw),
                )
            };
            let raw = raw.replace('\\', "/");
            let directory = raw.ends_with('/');
            let raw = raw.trim_end_matches('/');
            if raw.is_empty() {
                continue;
            }
            let mut body = String::new();
            let chars = raw.chars().collect::<Vec<_>>();
            // Resolve class endings once. Repeated unmatched '[' characters must
            // not turn validation of a bounded configuration into quadratic work.
            let mut ends = vec![chars.len(); chars.len() + 1];
            let mut end = chars.len();
            for i in (0..chars.len()).rev() {
                if chars[i] == ']' {
                    end = i;
                }
                ends[i] = end;
            }
            let mut index = 0;
            let mut valid = true;
            while index < chars.len() {
                match chars[index] {
                    '*' if chars.get(index + 1) == Some(&'*') => {
                        while chars.get(index + 1) == Some(&'*') {
                            index += 1;
                        }
                        if chars.get(index + 1) == Some(&'/') {
                            body.push_str("(?:.*/)?");
                            index += 1;
                        } else {
                            body.push_str(".*");
                        }
                    }
                    '*' => body.push_str("[^/]*"),
                    '?' => body.push_str("[^/]"),
                    '[' => {
                        let end = ends[index + 1];
                        if end < chars.len() && end > index + 1 {
                            if let Some(fragment) = class(&chars[index + 1..end]) {
                                body.push_str(&fragment);
                                index = end;
                            } else {
                                valid = false;
                                break;
                            }
                        } else {
                            body.push_str("\\[");
                        }
                    }
                    ']' => {
                        valid = false;
                        break;
                    }
                    c => body.push_str(&regex::escape(&c.to_string())),
                }
                index += 1;
            }
            if !valid {
                continue;
            }
            let anchored = raw.starts_with('/') || windows(raw);
            let slash = raw.contains('/');
            let expression = format!(
                "{}{}{}",
                if anchored {
                    "^"
                } else if slash {
                    "(?:^|.*/)"
                } else {
                    "(?:^|/)"
                },
                body,
                if directory || slash {
                    "(?:/.*)?$"
                } else {
                    "(?:/|$)"
                }
            );
            let expression = if windows(raw) {
                format!("(?i:{expression})")
            } else {
                expression
            };
            source_bytes = source_bytes.saturating_add(expression.len());
            if source_bytes > 4 * 1024 * 1024 {
                return Err("Directory ignore rules exceed the filter budget".into());
            }
            expressions.push(expression);
            negated.push(allow);
        }
        let rules = regex::RegexSetBuilder::new(expressions)
            .size_limit(4 * 1024 * 1024)
            .build()
            .map_err(|_| "Directory ignore rules exceed the filter budget")?;
        Ok(Self {
            hidden: hidden
                .iter()
                .map(|value| path(value))
                .filter(|value| !value.is_empty())
                .collect(),
            rules,
            negated,
        })
    }
    pub fn hidden(&self, value: &str) -> bool {
        let value = path(value);
        if value.is_empty() {
            return false;
        }
        let mut hidden = self.hidden.iter().any(|prefix| {
            if windows(prefix) || windows(&value) {
                let prefix = prefix.to_lowercase();
                let value = value.to_lowercase();
                value == prefix || value.starts_with(&format!("{prefix}/"))
            } else {
                value == *prefix || value.starts_with(&format!("{prefix}/"))
            }
        });
        if let Some(index) = self.rules.matches(&value).iter().last() {
            hidden = !self.negated[index];
        }
        hidden
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_browser_native_router_conformance_fixture() {
        let cases: Vec<serde_json::Value> = serde_json::from_str(include_str!(
            "../tests/fixtures/session-directory-rules.json"
        ))
        .unwrap();
        for case in cases {
            let hidden = serde_json::from_value::<Vec<String>>(case["hidden"].clone()).unwrap();
            let rules = serde_json::from_value::<Vec<String>>(case["rules"].clone()).unwrap();
            let filter = Filter::new(&hidden, &rules).unwrap();
            assert_eq!(
                filter.hidden(case["path"].as_str().unwrap()),
                case["expected"].as_bool().unwrap(),
                "{case}"
            );
        }
    }
    #[test]
    fn native_and_router_share_ordered_glob_and_directory_boundary_semantics() {
        let filter = Filter::new(
            &[" /work//private/ ".into(), "C:\\Private\\".into()],
            &[
                "# comment".into(),
                "**/build/".into(),
                "!/public/build/keep/".into(),
                "\\#secret".into(),
                "cache?".into(),
            ],
        )
        .unwrap();
        for path in [
            "/work/private",
            "/work/private/sub",
            " c:\\PRIVATE\\sub ",
            "/public/build/drop",
            "/work/cache1/x",
            "/work/#secret",
        ] {
            assert!(filter.hidden(path), "{path}");
        }
        for path in [
            "",
            "/work/private-other",
            "/public/build/keep/doc",
            "/work/cache-long",
            "/visible",
        ] {
            assert!(!filter.hidden(path), "{path}");
        }
        let root = Filter::new(&["/".into()], &[]).unwrap();
        assert!(root.hidden("/"));
        assert!(!root.hidden("/visible"));
    }
    #[test]
    fn bracket_ranges_negation_unicode_and_set_characters_follow_web_globs() {
        for (pattern, yes, no) in [
            ("job-[0-9]", "/work/job-7", "/work/job-a"),
            ("job-[!0-9]", "/work/job-a", "/work/job-7"),
            ("[中文]", "/work/中", "/work/英"),
            ("[a&&b]", "/work/&", "/work/z"),
            ("[a~~b]", "/work/~", "/work/z"),
            ("[a-z-]", "/work/-", "/work/0"),
            ("[--z]", "/work/a", "/work/~"),
            ("[^]", "/work/中", "/work/"),
        ] {
            let filter = Filter::new(&[], &[pattern.into()]).unwrap();
            assert!(filter.hidden(yes), "{pattern}: {yes}");
            assert!(!filter.hidden(no), "{pattern}: {no}");
        }
        for invalid in ["[z-a]", "[a--z]", "[]", "]"] {
            let filter = Filter::new(&[], &[invalid.into()]).unwrap();
            assert!(!filter.hidden(&format!("/work/{invalid}")));
        }
    }
}
