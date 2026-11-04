//! Worker-side native review projections. Indices retain original model identities.
use crate::workspace::{ChangedFile, Commit};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Unstaged,
    Staged,
    History,
}
impl Mode {
    pub fn from_selected(selected: u32) -> Self {
        match selected {
            1 => Self::Staged,
            2 => Self::History,
            _ => Self::Unstaged,
        }
    }
}
#[derive(Debug)]
pub struct Rows {
    pub files: Vec<usize>,
    pub commits: Vec<usize>,
    pub total_files: usize,
}
pub fn project(
    files: &[ChangedFile],
    commits: &[Commit],
    mode: Mode,
    query: &str,
) -> Result<Rows, String> {
    if query.len() > 4096 {
        return Err("Git filter exceeds 4 KiB".into());
    }
    let query = query.trim().to_lowercase();
    let matches = |text: &str| query.is_empty() || text.to_lowercase().contains(&query);
    let mut total_files = 0;
    let files = files
        .iter()
        .enumerate()
        .filter_map(|(index, file)| {
            if !match mode {
                Mode::Staged => file.staged,
                Mode::Unstaged => file.unstaged || file.untracked,
                Mode::History => true,
            } {
                return None;
            }
            total_files += 1;
            // In history the shared search filters commits, not the selected commit's files.
            (mode == Mode::History
                || matches(&file.path)
                || file.previous_path.as_deref().is_some_and(matches))
            .then_some(index)
        })
        .collect();
    let commits = if mode == Mode::History {
        commits
            .iter()
            .enumerate()
            .filter_map(|(index, commit)| {
                [
                    &commit.hash,
                    &commit.short_hash,
                    &commit.subject,
                    &commit.author_name,
                    &commit.author_email,
                ]
                .into_iter()
                .any(|text| matches(text))
                .then_some(index)
            })
            .collect()
    } else {
        Vec::new()
    };
    Ok(Rows {
        files,
        commits,
        total_files,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn file_scope_and_rename_search_preserve_original_indices_and_literal_paths() {
        let files: Vec<ChangedFile> = serde_json::from_value(json!([
            {"path":"docs/中文 .md", "previousPath":"Old/Name.md", "staged":true},
            {"path":"untracked.rs", "untracked":true},
            {"path":"both.rs", "staged":true, "unstaged":true}
        ]))
        .unwrap();
        let rows = project(&files, &[], Mode::Staged, " old/NAME ").unwrap();
        assert_eq!(rows.files, [0]);
        assert_eq!(rows.total_files, 2);
        assert_eq!(files[0].path, "docs/中文 .md");
        assert_eq!(
            project(&files, &[], Mode::Unstaged, "").unwrap().files,
            [1, 2]
        );
    }
    #[test]
    fn commit_search_covers_email_hash_subject_and_author_without_filtering_commit_files() {
        let commits: Vec<Commit> = serde_json::from_value(json!([
            {"hash":"ABC123", "shortHash":"ABC", "subject":"Fix 中文", "authorName":"Alice", "authorEmail":"alice@example.invalid", "authoredAt":"2026-10-25"},
            {"hash":"DEF456", "shortHash":"DEF", "subject":"Other", "authorName":"Bob", "authoredAt":"2026-10-24"}
        ])).unwrap();
        let files: Vec<ChangedFile> =
            serde_json::from_value(json!([{"path":"not matching", "status":"M"}])).unwrap();
        for query in ["abc", "中文", "ALICE", "@example.invalid"] {
            let rows = project(&files, &commits, Mode::History, query).unwrap();
            assert_eq!(rows.commits, [0]);
            assert_eq!(rows.files, [0]);
        }
        assert!(project(&files, &commits, Mode::History, "absent")
            .unwrap()
            .commits
            .is_empty());
        assert!(project(&files, &commits, Mode::History, &"x".repeat(4097)).is_err());
    }
}
