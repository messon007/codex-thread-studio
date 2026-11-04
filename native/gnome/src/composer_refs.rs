//! Bounded reference discovery and per-session input state, independent of GTK.
use crate::model::text;
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Command,
    File,
    Skill,
    Plugin,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Trigger {
    pub kind: Kind,
    pub start: i32,
    pub end: i32,
    pub query: String,
}
// Offsets are Unicode scalars, matching GtkTextBuffer, rather than UTF-8 bytes.
pub fn trigger(before: &str, end: i32) -> Option<Trigger> {
    if before.chars().count() as i32 == end {
        if let Some(query) = before.trim_start().strip_prefix('/') {
            if query
                .chars()
                .all(|ch| ch.is_alphanumeric() || ch == '_' || ch == '-')
            {
                return Some(Trigger {
                    kind: Kind::Command,
                    start: end - query.chars().count() as i32 - 1,
                    end,
                    query: query.into(),
                });
            }
        }
    }
    let token = before.rsplit(char::is_whitespace).next()?;
    let mut chars = token.chars();
    let kind = match chars.next()? {
        '@' => Kind::File,
        '$' => Kind::Skill,
        _ => return None,
    };
    let query = chars.as_str();
    if query.contains(['@', '$', '"']) || query.chars().count() > 256 {
        return None;
    }
    Some(Trigger {
        kind,
        start: end - token.chars().count() as i32,
        end,
        query: query.into(),
    })
}
#[derive(Clone, Debug)]
pub struct Suggestion {
    pub kind: Kind,
    pub name: String,
    pub path: String,
    pub title: String,
    pub detail: String,
}
fn clean(raw: &str, max: usize) -> String {
    raw.chars()
        .filter(|ch| !ch.is_control())
        .take(max)
        .collect()
}
fn field(raw: &Value, name: &str, max: usize) -> String {
    let value = text(raw, name);
    if value.len() > max || value.chars().any(char::is_control) {
        String::new()
    } else {
        value.into()
    }
}
pub fn catalog(result: &Value) -> Option<Arc<Vec<Arc<Suggestion>>>> {
    let mut output = Vec::new();
    if let Some(files) = result["files"].as_array() {
        for file in files.iter().take(200) {
            let path = if file["path"].is_string() {
                field(file, "path", 4096)
            } else {
                field(file, "file_name", 4096)
            };
            if path.is_empty() || path.len() > 4096 {
                continue;
            }
            output.push(Arc::new(Suggestion {
                kind: Kind::File,
                name: path.clone(),
                title: path.clone(),
                path,
                detail: String::new(),
            }));
        }
    } else if result["data"]
        .as_array()
        .is_some_and(|rows| rows.iter().any(|row| row.get("skills").is_some()))
    {
        for skill in result["data"]
            .as_array()
            .into_iter()
            .flatten()
            .take(64)
            .flat_map(|row| row["skills"].as_array().into_iter().flatten())
            .take(512)
        {
            if skill["enabled"] == false {
                continue;
            }
            let name = field(skill, "name", 128);
            let path = field(skill, "path", 4096);
            if name.is_empty() || path.is_empty() {
                continue;
            }
            let title = skill
                .pointer("/interface/displayName")
                .and_then(Value::as_str)
                .filter(|name| !name.is_empty())
                .unwrap_or(&name);
            output.push(Arc::new(Suggestion {
                kind: Kind::Skill,
                title: clean(title, 128),
                name,
                path,
                detail: clean(
                    skill["shortDescription"]
                        .as_str()
                        .unwrap_or(text(skill, "description")),
                    400,
                ),
            }));
        }
    } else if let Some(markets) = result["marketplaces"].as_array() {
        for plugin in markets
            .iter()
            .take(64)
            .flat_map(|market| market["plugins"].as_array().into_iter().flatten())
            .take(512)
        {
            if plugin["installed"] != true || plugin["enabled"] != true {
                continue;
            }
            let id = field(plugin, "id", 256);
            let name = field(plugin, "name", 128);
            if id.is_empty() || name.is_empty() {
                continue;
            }
            output.push(Arc::new(Suggestion {
                kind: Kind::Plugin,
                title: clean(
                    plugin
                        .pointer("/interface/displayName")
                        .and_then(Value::as_str)
                        .unwrap_or(&name),
                    128,
                ),
                name,
                path: format!("plugin://{id}"),
                detail: clean(
                    plugin
                        .pointer("/interface/shortDescription")
                        .and_then(Value::as_str)
                        .unwrap_or("Installed plugin"),
                    400,
                ),
            }));
        }
    } else {
        return None;
    }
    Some(Arc::new(output))
}
pub fn filter(sources: Vec<Arc<Vec<Arc<Suggestion>>>>, query: &str) -> Vec<Arc<Suggestion>> {
    let query = query.to_lowercase();
    let mut seen = HashSet::new();
    let mut matches = sources
        .iter()
        .flat_map(|source| source.iter())
        .filter(|entry| {
            seen.insert(entry.path.as_str())
                && (entry.kind == Kind::File
                    || query.is_empty()
                    || [&entry.name, &entry.title, &entry.detail]
                        .iter()
                        .any(|field| field.to_lowercase().contains(&query)))
        })
        .cloned()
        .collect::<Vec<_>>();
    // File results already carry the backend's fuzzy ranking. Skill/plugin
    // sources arrive independently, so give their merged menu a stable order.
    if matches.iter().all(|entry| entry.kind != Kind::File) {
        matches.sort_by_cached_key(|entry| {
            (
                if entry.kind == Kind::Plugin { 0 } else { 1 },
                entry.name.to_lowercase(),
                entry.path.clone(),
            )
        });
    }
    matches.truncate(30);
    matches
}
pub fn replacement(suggestion: &Suggestion) -> String {
    if suggestion.kind == Kind::File {
        let path = &suggestion.path;
        if path.chars().any(char::is_whitespace) || path.contains('"') {
            format!("{} ", serde_json::to_string(path).unwrap_or_default())
        } else {
            format!("{path} ")
        }
    } else {
        format!("${} ", suggestion.name)
    }
}
#[derive(Clone, Debug)]
pub struct Reference {
    pub id: String,
    pub label: String,
    pub input: Value,
    pub bytes: usize,
}
#[derive(Default)]
pub struct Book {
    pub revision: u64,
    sessions: HashMap<String, Vec<Arc<Reference>>>,
}
impl Book {
    pub fn add_prepared(&mut self, key: &str, prepared: Vec<Arc<Reference>>) -> Result<(), String> {
        let current = self.get(key);
        let additions = prepared
            .into_iter()
            .filter(|reference| {
                !current
                    .iter()
                    .any(|old| old.input["path"] == reference.input["path"])
            })
            .collect::<Vec<_>>();
        let bytes = self
            .sessions
            .values()
            .flatten()
            .map(|reference| reference.bytes)
            .sum::<usize>()
            + additions
                .iter()
                .map(|reference| reference.bytes)
                .sum::<usize>();
        if current.len() + additions.len() > 32
            || (!self.sessions.contains_key(key) && self.sessions.len() >= 128)
            || bytes > 2 * 1024 * 1024
        {
            return Err("Pending reference budget is full; remove some references first".into());
        }
        self.sessions
            .entry(key.into())
            .or_default()
            .extend(additions);
        self.revision += 1;
        Ok(())
    }
    pub fn get(&self, key: &str) -> Vec<Arc<Reference>> {
        self.sessions.get(key).cloned().unwrap_or_default()
    }
    pub fn add(
        &mut self,
        key: &str,
        id: String,
        source: &Suggestion,
        root: &str,
        opencode: bool,
    ) -> Result<(), String> {
        if source.kind == Kind::Command {
            return Err("Commands cannot be attached to messages".into());
        }
        if source.kind == Kind::File && !opencode {
            return Ok(());
        }
        let refs = self.get(key);
        if refs.iter().any(|item| item.input["path"] == source.path) {
            return Ok(());
        }
        if refs.len() >= 32 || (!self.sessions.contains_key(key) && self.sessions.len() >= 128) {
            return Err("Remove some pending references first".into());
        }
        let input = if source.kind == Kind::File {
            json!({"type":"file","path":source.path,"root":root})
        } else {
            json!({"type":if source.kind==Kind::Plugin{"mention"}else{"skill"},"name":source.name,"path":source.path})
        };
        let bytes = source.path.len() + source.name.len() + source.title.len() + root.len() + 256;
        if self
            .sessions
            .values()
            .flatten()
            .map(|item| item.bytes)
            .sum::<usize>()
            + bytes
            > 2 * 1024 * 1024
        {
            return Err("Pending reference memory budget is full".into());
        }
        self.sessions
            .entry(key.into())
            .or_default()
            .push(Arc::new(Reference {
                id,
                label: source.title.clone(),
                input,
                bytes,
            }));
        self.revision += 1;
        Ok(())
    }
    pub fn remove(&mut self, key: &str, ids: &[String]) {
        if let Some(items) = self.sessions.get_mut(key) {
            items.retain(|item| !ids.contains(&item.id));
            if items.is_empty() {
                self.sessions.remove(key);
            }
        }
        self.revision += 1;
    }
    pub fn forget(&mut self, key: &str) {
        self.sessions.remove(key);
        self.revision += 1;
    }
}
/// Prepare file names, never their potentially large contents, outside GTK.
pub fn prepare_files(
    paths: Vec<std::path::PathBuf>,
    tag: u64,
) -> Result<Vec<Arc<Reference>>, String> {
    if paths.is_empty() || paths.len() > 32 {
        return Err("Choose between one and 32 reference files".into());
    }
    let mut output = Vec::new();
    let mut seen = HashSet::new();
    for path in paths {
        let path = std::fs::canonicalize(path).map_err(|_| "The selected file is unavailable")?;
        if !path.is_file() {
            return Err("Only regular files can be attached as references".into());
        }
        let path = path
            .to_str()
            .filter(|path| path.len() <= 4096 && !path.chars().any(char::is_control))
            .ok_or("Unsupported reference path")?
            .to_owned();
        if !seen.insert(path.clone()) {
            continue;
        }
        let root = std::path::Path::new(&path)
            .parent()
            .and_then(std::path::Path::to_str)
            .ok_or("The reference has no parent directory")?;
        let input = json!({"type":"file","path":path,"root":root});
        let bytes = path.len() * 2 + 256;
        output.push(Arc::new(Reference {
            id: format!("router-file-{tag}-{}", output.len()),
            label: path,
            input,
            bytes,
        }));
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn file_preparation_keeps_only_metadata_deduplicates_and_preserves_other_drafts() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("文档 with spaces.md");
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(30 * 1024 * 1024).unwrap();
        let prepared = prepare_files(vec![path.clone(), path.clone()], 7).unwrap();
        assert_eq!(prepared.len(), 1);
        assert!(serde_json::to_vec(&prepared[0].input).unwrap().len() < 9000);
        assert_eq!(prepared[0].input["path"], path.to_str().unwrap());
        let mut book = Book::default();
        book.add_prepared("codex:router", prepared.clone()).unwrap();
        book.add_prepared("codex:router", prepared.clone()).unwrap();
        assert_eq!(book.get("codex:router").len(), 1);
        book.add_prepared("opencode:router", prepared.clone())
            .unwrap();
        book.remove("codex:router", &[prepared[0].id.clone()]);
        assert_eq!(book.get("opencode:router").len(), 1);
        assert!(prepare_files(vec![dir.path().to_path_buf()], 8).is_err());
        #[cfg(unix)]
        assert!(prepare_files(vec!["/dev/zero".into()], 9).is_err());
    }
    #[test]
    fn scalar_ranges_quotes_enabled_catalogs_and_bounded_matching() {
        let trigger = trigger("中文😀 $build", 10).unwrap();
        assert_eq!((trigger.start, trigger.end), (4, 10));
        assert!(super::trigger("email@example.org", 17).is_none());
        let suggestions=catalog(&json!({"data":[{"skills":[{"name":"disabled","path":"/no","enabled":false},{"name":"build","path":"/skills/构建/SKILL.md","description":"中文😀 build helper","enabled":true}]}]})).unwrap();
        assert_eq!(filter(vec![suggestions.clone()], "中文").len(), 1);
        let plugin=catalog(&json!({"marketplaces":[{"plugins":[{"id":"build@market","name":"build","installed":true,"enabled":true}]}]})).unwrap();
        assert_eq!(filter(vec![suggestions, plugin], "build").len(), 2);
        let file = Suggestion {
            kind: Kind::File,
            name: String::new(),
            path: "docs/中文 \"a\".md".into(),
            title: String::new(),
            detail: String::new(),
        };
        assert_eq!(replacement(&file), "\"docs/中文 \\\"a\\\".md\" ");
    }
    #[test]
    fn reference_acknowledgements_preserve_newer_items_and_other_sessions() {
        let source = Suggestion {
            kind: Kind::Skill,
            name: "build".into(),
            path: "/s/SKILL.md".into(),
            title: "Build".into(),
            detail: String::new(),
        };
        let mut book = Book::default();
        book.add("c:one", "old".into(), &source, "/project", false)
            .unwrap();
        book.add("o:two", "other".into(), &source, "/project", true)
            .unwrap();
        let mut newer = source.clone();
        newer.path = "/new/SKILL.md".into();
        book.add("c:one", "new".into(), &newer, "/project", false)
            .unwrap();
        book.remove("c:one", &["old".into()]);
        assert_eq!(book.get("c:one")[0].id, "new");
        assert_eq!(book.get("o:two")[0].id, "other");
        assert_eq!(book.get("c:one")[0].input["type"], "skill");
    }
    #[test]
    fn backend_fuzzy_file_ranking_is_preserved_without_requiring_a_literal_match() {
        let source=catalog(&json!({"files":[{"file_name":"src/markdown.rs"},{"path":"second.rs"},{"path":"bad\nname.rs"}]})).unwrap();
        let results = filter(vec![source], "smr");
        assert_eq!(
            results
                .iter()
                .map(|item| item.path.as_str())
                .collect::<Vec<_>>(),
            vec!["src/markdown.rs", "second.rs"]
        );
    }
}
