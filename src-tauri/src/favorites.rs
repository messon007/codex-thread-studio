use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

pub const MAX_FAVORITE_BODY_BYTES: usize = 384 * 1024;
const MAX_FAVORITES: usize = 2_000;
const MAX_CONTENT_BYTES: usize = 192 * 1024;
const MAX_NOTE_BYTES: usize = 16 * 1024;
const MAX_RESULTS: usize = 2_000;
const MAX_STORE_BYTES: u64 = 400 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Favorite {
    pub id: String,
    #[serde(default = "default_scope")]
    pub scope: String,
    pub backend: String,
    pub thread_id: String,
    pub thread_title: String,
    pub project_path: String,
    pub turn_id: String,
    pub item_id: String,
    pub title: String,
    pub question: String,
    pub content: String,
    pub note: String,
    pub tags: Vec<String>,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FavoriteSummary {
    pub id: String,
    pub scope: String,
    pub backend: String,
    pub thread_id: String,
    pub thread_title: String,
    pub project_path: String,
    pub turn_id: String,
    pub item_id: String,
    pub title: String,
    pub question_snippet: String,
    pub snippet: String,
    pub tags: Vec<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FavoriteList {
    pub items: Vec<FavoriteSummary>,
    pub total: usize,
    pub all_total: usize,
}

#[derive(Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct FavoriteStore {
    #[serde(default = "store_version")]
    version: u8,
    #[serde(default)]
    items: Vec<Favorite>,
}

fn store_version() -> u8 {
    1
}

fn default_scope() -> String {
    "message".to_string()
}

pub fn load(path: &Path) -> Result<Vec<Favorite>, String> {
    if path
        .metadata()
        .is_ok_and(|metadata| metadata.len() > MAX_STORE_BYTES)
    {
        return Err("favorites file is too large".to_string());
    }
    match fs::read(path) {
        Ok(data) => {
            let store = serde_json::from_slice::<FavoriteStore>(&data)
                .map_err(|error| format!("invalid favorites file: {error}"))?;
            if store.items.len() > MAX_FAVORITES {
                return Err("favorites file contains too many items".to_string());
            }
            for favorite in &store.items {
                validate(favorite)
                    .map_err(|error| format!("invalid favorite {}: {error}", favorite.id))?;
            }
            Ok(store.items)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error.to_string()),
    }
}

pub fn list(items: &[Favorite], query: &str, limit: usize) -> FavoriteList {
    let all_total = items.len();
    let query = query.trim().to_lowercase();
    let mut matches = items
        .iter()
        .filter(|favorite| {
            let tags = favorite.tags.join(" ");
            query.is_empty()
                || [
                    favorite.title.as_str(),
                    favorite.question.as_str(),
                    favorite.content.as_str(),
                    favorite.note.as_str(),
                    favorite.thread_title.as_str(),
                    favorite.project_path.as_str(),
                    favorite.backend.as_str(),
                    tags.as_str(),
                ]
                .iter()
                .any(|value| value.to_lowercase().contains(&query))
        })
        .collect::<Vec<_>>();
    matches.sort_by(|left, right| right.created_at.cmp(&left.created_at));
    let total = matches.len();
    let items = matches
        .into_iter()
        .take(limit.clamp(1, MAX_RESULTS))
        .map(summary)
        .collect();
    FavoriteList {
        items,
        total,
        all_total,
    }
}

pub fn insert(
    path: &Path,
    mut items: Vec<Favorite>,
    favorite: Favorite,
) -> Result<Favorite, String> {
    validate(&favorite)?;
    if items.len() >= MAX_FAVORITES {
        return Err(format!("at most {MAX_FAVORITES} favorites can be stored"));
    }
    if items.iter().any(|item| item.id == favorite.id) {
        return Err("favorite id already exists".to_string());
    }
    if favorite.scope == "message"
        && items.iter().any(|item| {
            item.scope == "message"
                && item.backend == favorite.backend
                && item.thread_id == favorite.thread_id
                && item.turn_id == favorite.turn_id
                && item.item_id == favorite.item_id
        })
    {
        return Err("this message is already a favorite".to_string());
    }
    items.push(favorite.clone());
    save(path, &items)?;
    Ok(favorite)
}

pub fn update(
    path: &Path,
    mut items: Vec<Favorite>,
    id: &str,
    favorite: Favorite,
) -> Result<Option<Favorite>, String> {
    validate(&favorite)?;
    if favorite.id != id {
        return Err("favorite id cannot be changed".to_string());
    }
    let Some(index) = items.iter().position(|candidate| candidate.id == id) else {
        return Ok(None);
    };
    items[index] = favorite.clone();
    save(path, &items)?;
    Ok(Some(favorite))
}

pub fn remove(path: &Path, mut items: Vec<Favorite>, id: &str) -> Result<Option<Favorite>, String> {
    let Some(index) = items.iter().position(|favorite| favorite.id == id) else {
        return Ok(None);
    };
    let removed = items.remove(index);
    save(path, &items)?;
    Ok(Some(removed))
}

pub fn find(items: &[Favorite], id: &str) -> Option<Favorite> {
    items.iter().find(|favorite| favorite.id == id).cloned()
}

pub fn validate(favorite: &Favorite) -> Result<(), String> {
    if favorite.id.trim().is_empty() || favorite.id.len() > 128 {
        return Err("favorite id must contain between 1 and 128 bytes".to_string());
    }
    if !matches!(favorite.backend.as_str(), "codex" | "opencode") {
        return Err("favorite backend must be codex or opencode".to_string());
    }
    if !matches!(favorite.scope.as_str(), "message" | "selection") {
        return Err("favorite scope must be message or selection".to_string());
    }
    if favorite.thread_id.trim().is_empty()
        || favorite.thread_id.len() > 256
        || favorite.turn_id.trim().is_empty()
        || favorite.turn_id.len() > 256
        || favorite.item_id.trim().is_empty()
        || favorite.item_id.len() > 256
    {
        return Err("favorite source anchor is invalid".to_string());
    }
    if favorite.thread_title.len() > 512
        || favorite.project_path.len() > 2_048
        || favorite.created_at.trim().is_empty()
        || favorite.created_at.len() > 128
    {
        return Err("favorite source metadata is too large".to_string());
    }
    if favorite.title.trim().is_empty() || favorite.title.len() > 512 {
        return Err("favorite title must contain between 1 and 512 bytes".to_string());
    }
    if favorite.content.trim().is_empty() || favorite.content.len() > MAX_CONTENT_BYTES {
        return Err(format!(
            "favorite content must contain between 1 and {MAX_CONTENT_BYTES} bytes"
        ));
    }
    if favorite.question.len() > MAX_CONTENT_BYTES {
        return Err(format!(
            "favorite question must fit within {MAX_CONTENT_BYTES} bytes"
        ));
    }
    if favorite.note.len() > MAX_NOTE_BYTES {
        return Err(format!(
            "favorite note must fit within {MAX_NOTE_BYTES} bytes"
        ));
    }
    if favorite.tags.len() > 20
        || favorite
            .tags
            .iter()
            .any(|tag| tag.trim().is_empty() || tag.len() > 128)
    {
        return Err("favorite tags are invalid".to_string());
    }
    Ok(())
}

fn summary(favorite: &Favorite) -> FavoriteSummary {
    FavoriteSummary {
        id: favorite.id.clone(),
        scope: favorite.scope.clone(),
        backend: favorite.backend.clone(),
        thread_id: favorite.thread_id.clone(),
        thread_title: favorite.thread_title.clone(),
        project_path: favorite.project_path.clone(),
        turn_id: favorite.turn_id.clone(),
        item_id: favorite.item_id.clone(),
        title: favorite.title.clone(),
        question_snippet: snippet(&favorite.question, 140),
        snippet: snippet(&favorite.content, 280),
        tags: favorite.tags.clone(),
        created_at: favorite.created_at.clone(),
    }
}

fn snippet(content: &str, limit: usize) -> String {
    let normalized = content.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut characters = normalized.chars();
    let snippet = characters.by_ref().take(limit).collect::<String>();
    if characters.next().is_some() {
        format!("{snippet}…")
    } else {
        snippet
    }
}

fn save(path: &Path, items: &[Favorite]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "favorites path has no parent directory".to_string())?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let data = serde_json::to_vec_pretty(&FavoriteStore {
        version: store_version(),
        items: items.to_vec(),
    })
    .map_err(|error| error.to_string())?;
    let temporary = path.with_extension(format!("json.tmp-{}", std::process::id()));
    fs::write(&temporary, data).map_err(|error| error.to_string())?;
    fs::rename(&temporary, path).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn favorite(id: &str, title: &str, content: &str) -> Favorite {
        Favorite {
            id: id.to_string(),
            scope: "message".to_string(),
            backend: "codex".to_string(),
            thread_id: "thread-1".to_string(),
            thread_title: "Books".to_string(),
            project_path: "/tmp/books".to_string(),
            turn_id: "turn-1".to_string(),
            item_id: format!("item-{id}"),
            title: title.to_string(),
            question: "What should we build?".to_string(),
            content: content.to_string(),
            note: "Remember this".to_string(),
            tags: vec!["design".to_string()],
            created_at: "2026-07-24T00:00:00.000Z".to_string(),
        }
    }

    #[test]
    fn searches_content_metadata_notes_and_tags() {
        let items = vec![
            favorite("a", "Architecture", "A durable event log"),
            favorite("b", "Testing", "A regression suite"),
        ];
        assert_eq!(list(&items, "event", 20).items[0].id, "a");
        assert_eq!(list(&items, "remember", 20).total, 2);
        assert_eq!(list(&items, "design", 20).total, 2);
        assert_eq!(list(&items, "books", 20).total, 2);
    }

    #[test]
    fn validates_native_source_anchors() {
        assert!(validate(&favorite("a", "Useful", "content")).is_ok());
        let mut invalid = favorite("a", "", "content");
        assert!(validate(&invalid).is_err());
        invalid = favorite("a", "Useful", "");
        assert!(validate(&invalid).is_err());
        invalid = favorite("a", "Useful", "content");
        invalid.item_id.clear();
        assert!(validate(&invalid).is_err());
    }

    #[test]
    fn allows_multiple_selected_excerpts_from_one_message() {
        let path = std::env::temp_dir().join(format!(
            "codex-thread-studio-selection-favorites-{}.json",
            std::process::id()
        ));
        let mut first = favorite("selection-a", "First excerpt", "first");
        first.scope = "selection".to_string();
        let mut second = favorite("selection-b", "Second excerpt", "second");
        second.scope = "selection".to_string();
        second.item_id = first.item_id.clone();
        let saved = insert(&path, vec![first], second).expect("second excerpt should be accepted");
        assert_eq!(saved.scope, "selection");
        fs::remove_file(path).ok();
    }
}
