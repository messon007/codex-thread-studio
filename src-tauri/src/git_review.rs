use serde::{Deserialize, Serialize};

pub const MAX_STATUS_FILES: usize = 1_000;
pub const MAX_DIFF_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_HISTORY_COMMITS: usize = 100;
pub const MAX_COMMIT_FILES: usize = 1_000;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitRootRequest {
    pub root: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHistoryRequest {
    pub root: String,
    pub offset: Option<usize>,
    pub limit: Option<usize>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCommitRequest {
    pub root: String,
    pub commit: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCommitDiffRequest {
    pub root: String,
    pub commit: String,
    pub path: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitDiffRequest {
    pub root: String,
    pub path: String,
    pub scope: GitDiffScope,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitPathsRequest {
    pub root: String,
    pub paths: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum GitDiffScope {
    Staged,
    Unstaged,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitStatusResponse {
    pub root: String,
    pub branch: String,
    pub detached: bool,
    pub has_head: bool,
    pub files: Vec<GitChangedFile>,
    pub truncated: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitChangedFile {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_path: Option<String>,
    pub index_status: String,
    pub worktree_status: String,
    pub staged: bool,
    pub unstaged: bool,
    pub untracked: bool,
    pub conflicted: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHistoryResponse {
    pub root: String,
    pub commits: Vec<GitCommitSummary>,
    pub has_more: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCommitSummary {
    pub hash: String,
    pub short_hash: String,
    pub author_name: String,
    pub author_email: String,
    pub authored_at: String,
    pub subject: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCommitFilesResponse {
    pub root: String,
    pub commit: String,
    pub files: Vec<GitCommitFile>,
    pub truncated: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCommitFile {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_path: Option<String>,
    pub status: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitDiffResponse {
    pub root: String,
    pub path: String,
    pub scope: GitDiffScopeResponse,
    pub content: String,
    pub binary: bool,
    pub truncated: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCommitDiffResponse {
    pub root: String,
    pub commit: String,
    pub path: String,
    pub content: String,
    pub binary: bool,
    pub truncated: bool,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GitDiffScopeResponse {
    Staged,
    Unstaged,
}

impl From<GitDiffScope> for GitDiffScopeResponse {
    fn from(value: GitDiffScope) -> Self {
        match value {
            GitDiffScope::Staged => Self::Staged,
            GitDiffScope::Unstaged => Self::Unstaged,
        }
    }
}

pub fn parse_status(root: String, output: &[u8]) -> Result<GitStatusResponse, String> {
    let mut records = output
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty());
    let mut branch = String::new();
    let mut detached = false;
    let mut has_head = true;
    let mut files = Vec::new();

    while let Some(record) = records.next() {
        if record.starts_with(b"## ") {
            let header = String::from_utf8_lossy(&record[3..]);
            let name = header.split("...").next().unwrap_or(&header);
            if name == "HEAD (no branch)" {
                branch = "Detached HEAD".to_owned();
                detached = true;
            } else if let Some(name) = name.strip_prefix("No commits yet on ") {
                branch = name.to_owned();
                has_head = false;
            } else if let Some(name) = name.strip_prefix("Initial commit on ") {
                branch = name.to_owned();
                has_head = false;
            } else {
                branch = name.to_owned();
            }
            continue;
        }
        if record.len() < 4 || record[2] != b' ' {
            return Err("git returned an invalid porcelain status record".to_owned());
        }
        let index = record[0] as char;
        let worktree = record[1] as char;
        if index == '!' && worktree == '!' {
            continue;
        }
        let path = String::from_utf8_lossy(&record[3..]).into_owned();
        let renamed = matches!(index, 'R' | 'C') || matches!(worktree, 'R' | 'C');
        let previous_path = if renamed {
            records
                .next()
                .map(|value| String::from_utf8_lossy(value).into_owned())
        } else {
            None
        };
        let untracked = index == '?' && worktree == '?';
        let conflicted =
            index == 'U' || worktree == 'U' || matches!((index, worktree), ('D', 'D') | ('A', 'A'));
        files.push(GitChangedFile {
            path,
            previous_path,
            index_status: index.to_string(),
            worktree_status: worktree.to_string(),
            staged: !untracked && index != ' ',
            unstaged: untracked || worktree != ' ',
            untracked,
            conflicted,
        });
    }
    let truncated = files.len() > MAX_STATUS_FILES;
    files.truncate(MAX_STATUS_FILES);
    Ok(GitStatusResponse {
        root,
        branch: if branch.is_empty() {
            "HEAD".to_owned()
        } else {
            branch
        },
        detached,
        has_head,
        files,
        truncated,
    })
}

pub fn status_arguments(root: &str) -> Vec<String> {
    [
        "--literal-pathspecs",
        "-C",
        root,
        "status",
        "--porcelain=v1",
        "-z",
        "--branch",
        "--untracked-files=all",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

pub fn repository_root_arguments(root: &str) -> Vec<String> {
    [
        "--literal-pathspecs",
        "-C",
        root,
        "rev-parse",
        "--show-toplevel",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

pub fn history_arguments(root: &str, offset: usize, limit: usize) -> Vec<String> {
    vec![
        "--literal-pathspecs".to_owned(),
        "-C".to_owned(),
        root.to_owned(),
        "log".to_owned(),
        "--no-color".to_owned(),
        "--no-decorate".to_owned(),
        format!("--skip={offset}"),
        format!("--max-count={}", limit.saturating_add(1)),
        "--format=%H%x1f%h%x1f%an%x1f%ae%x1f%aI%x1f%s%x1e".to_owned(),
    ]
}

pub fn commit_files_arguments(root: &str, commit: &str) -> Vec<String> {
    [
        "--literal-pathspecs",
        "-C",
        root,
        "show",
        "--format=",
        "--first-parent",
        "--name-status",
        "-z",
        "-M",
        "-C",
        commit,
        "--",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

pub fn commit_diff_arguments(root: &str, commit: &str, path: &str) -> Vec<String> {
    [
        "--literal-pathspecs",
        "-C",
        root,
        "show",
        "--format=",
        "--first-parent",
        "--no-ext-diff",
        "--no-color",
        "--unified=3",
        commit,
        "--",
        path,
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

pub fn parse_repository_root(output: &[u8]) -> Result<String, String> {
    let root = String::from_utf8_lossy(output).trim().to_owned();
    if root.is_empty() || root.contains('\0') || root.contains('\n') || root.contains('\r') {
        return Err("git returned an invalid repository root".to_owned());
    }
    Ok(root)
}

pub fn parse_history(
    root: String,
    output: &[u8],
    limit: usize,
) -> Result<GitHistoryResponse, String> {
    let mut commits = Vec::new();
    for raw_record in output.split(|byte| *byte == 0x1e) {
        let record = raw_record
            .iter()
            .copied()
            .skip_while(|byte| matches!(byte, b'\n' | b'\r'))
            .collect::<Vec<_>>();
        if record.is_empty() {
            continue;
        }
        let fields = record
            .split(|byte| *byte == 0x1f)
            .map(|field| String::from_utf8_lossy(field).into_owned())
            .collect::<Vec<_>>();
        if fields.len() != 6 {
            return Err("git returned an invalid history record".to_owned());
        }
        commits.push(GitCommitSummary {
            hash: fields[0].clone(),
            short_hash: fields[1].clone(),
            author_name: fields[2].clone(),
            author_email: fields[3].clone(),
            authored_at: fields[4].clone(),
            subject: fields[5].clone(),
        });
    }
    let has_more = commits.len() > limit;
    commits.truncate(limit);
    Ok(GitHistoryResponse {
        root,
        commits,
        has_more,
    })
}

pub fn parse_commit_files(
    root: String,
    commit: String,
    output: &[u8],
) -> Result<GitCommitFilesResponse, String> {
    let records = output
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
        .collect::<Vec<_>>();
    let mut files = Vec::new();
    let mut index = 0;
    while index < records.len() {
        let status = String::from_utf8_lossy(records[index]).into_owned();
        index += 1;
        let code = status.chars().next().unwrap_or_default();
        let renamed = matches!(code, 'R' | 'C');
        let previous_path = if renamed {
            let Some(path) = records.get(index) else {
                return Err("git returned an invalid renamed path record".to_owned());
            };
            index += 1;
            Some(String::from_utf8_lossy(path).into_owned())
        } else {
            None
        };
        let Some(path) = records.get(index) else {
            return Err("git returned an invalid commit path record".to_owned());
        };
        index += 1;
        files.push(GitCommitFile {
            path: String::from_utf8_lossy(path).into_owned(),
            previous_path,
            status: code.to_string(),
        });
    }
    let truncated = files.len() > MAX_COMMIT_FILES;
    files.truncate(MAX_COMMIT_FILES);
    Ok(GitCommitFilesResponse {
        root,
        commit,
        files,
        truncated,
    })
}

pub fn diff_arguments(root: &str, path: &str, scope: GitDiffScope, untracked: bool) -> Vec<String> {
    let mut arguments = [
        "--literal-pathspecs",
        "-C",
        root,
        "diff",
        "--no-ext-diff",
        "--no-color",
        "--unified=3",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<Vec<_>>();
    if untracked {
        arguments.extend([
            "--no-index".to_owned(),
            "--".to_owned(),
            "/dev/null".to_owned(),
            path.to_owned(),
        ]);
    } else {
        if scope == GitDiffScope::Staged {
            arguments.push("--cached".to_owned());
        }
        arguments.extend(["--".to_owned(), path.to_owned()]);
    }
    arguments
}

pub fn mutation_arguments(
    root: &str,
    paths: &[String],
    stage: bool,
    has_head: bool,
) -> Vec<String> {
    let mut arguments = ["--literal-pathspecs", "-C", root]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if stage {
        arguments.extend(["add".to_owned(), "--".to_owned()]);
    } else if has_head {
        arguments.extend(["restore".to_owned(), "--staged".to_owned(), "--".to_owned()]);
    } else {
        arguments.extend(["rm".to_owned(), "--cached".to_owned(), "--".to_owned()]);
    }
    arguments.extend(paths.iter().cloned());
    arguments
}

pub fn validate_relative_path(path: &str) -> Result<(), String> {
    if path.is_empty() || path.contains('\0') || path.contains('\n') || path.contains('\r') {
        return Err("Git paths must be non-empty single-line paths".to_owned());
    }
    let normalized = path.replace('\\', "/");
    if normalized.starts_with('/')
        || normalized.split('/').any(|part| part == "..")
        || normalized.get(1..3) == Some(":/")
    {
        return Err("Git paths must stay inside the project directory".to_owned());
    }
    Ok(())
}

pub fn validate_commit_id(commit: &str) -> Result<(), String> {
    if !matches!(commit.len(), 40 | 64) || !commit.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("Git commit must be a full object ID".to_owned());
    }
    Ok(())
}

pub fn bounded_diff(bytes: Vec<u8>) -> (String, bool) {
    let truncated = bytes.len() > MAX_DIFF_BYTES;
    let mut bytes = if truncated {
        bytes[..MAX_DIFF_BYTES].to_vec()
    } else {
        bytes
    };
    while std::str::from_utf8(&bytes).is_err() && !bytes.is_empty() {
        bytes.pop();
    }
    (String::from_utf8(bytes).unwrap_or_default(), truncated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output};

    struct TestRepository(PathBuf);

    impl TestRepository {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "codex-thread-studio-git-review-{}",
                uuid::Uuid::new_v4().simple()
            ));
            std::fs::create_dir_all(&root).unwrap();
            let repository = Self(root);
            repository.git(&["init", "-b", "main"]);
            repository.git(&["config", "user.name", "Studio Test"]);
            repository.git(&["config", "user.email", "studio@example.invalid"]);
            repository.git(&["config", "core.autocrlf", "false"]);
            repository
        }

        fn path(&self) -> &Path {
            &self.0
        }

        fn git(&self, arguments: &[&str]) -> Output {
            let output = Command::new("git")
                .arg("-C")
                .arg(&self.0)
                .args(arguments)
                .output()
                .expect("run Git fixture command");
            assert!(
                output.status.success(),
                "git {arguments:?}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            output
        }

        fn run_arguments(&self, arguments: &[String]) -> Output {
            Command::new("git")
                .args(arguments)
                .output()
                .expect("run generated Git command")
        }
    }

    impl Drop for TestRepository {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn parses_branch_staged_unstaged_untracked_and_rename_records() {
        let output = b"## feature/review...origin/feature/review\0 M README.md\0M  src/main.rs\0?? data/report.csv\0R  docs/new.md\0docs/old.md\0";
        let status = parse_status("/project".to_owned(), output).unwrap();
        assert_eq!(status.branch, "feature/review");
        assert_eq!(status.files.len(), 4);
        assert!(status.files[0].unstaged && !status.files[0].staged);
        assert!(status.files[1].staged && !status.files[1].unstaged);
        assert!(status.files[2].untracked);
        assert_eq!(
            status.files[3].previous_path.as_deref(),
            Some("docs/old.md")
        );
    }

    #[test]
    fn handles_unborn_and_detached_headers() {
        let unborn = parse_status("/p".to_owned(), b"## No commits yet on main\0").unwrap();
        assert_eq!(unborn.branch, "main");
        assert!(!unborn.has_head);
        let detached = parse_status("/p".to_owned(), b"## HEAD (no branch)\0").unwrap();
        assert!(detached.detached);
        assert!(detached.has_head);
    }

    #[test]
    fn rejects_paths_that_escape_the_workspace() {
        for path in [
            "",
            "../secret",
            "/etc/passwd",
            "C:\\Windows\\win.ini",
            "a\nfile",
        ] {
            assert!(validate_relative_path(path).is_err(), "{path:?}");
        }
        for path in ["README.md", "docs/guide.md", "data report.csv"] {
            assert!(validate_relative_path(path).is_ok(), "{path:?}");
        }
    }

    #[test]
    fn truncates_diff_on_a_utf8_boundary() {
        let mut bytes = vec![b'a'; MAX_DIFF_BYTES - 1];
        bytes.extend_from_slice("\u{754c}".as_bytes());
        let (content, truncated) = bounded_diff(bytes);
        assert!(truncated);
        assert!(content.is_char_boundary(content.len()));
    }

    #[test]
    fn parses_paginated_history_records() {
        let output = b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\x1faaaaaaa\x1fAda\x1fada@example.com\x1f2026-09-15T10:00:00+08:00\x1fFirst commit\x1e\nbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\x1fbbbbbbb\x1fLin\x1flin@example.com\x1f2026-09-14T09:00:00+08:00\x1fSecond commit\x1e";
        let history = parse_history("/project".to_owned(), output, 1).unwrap();
        assert_eq!(history.commits.len(), 1);
        assert!(history.has_more);
        assert_eq!(history.commits[0].short_hash, "aaaaaaa");
        assert_eq!(history.commits[0].subject, "First commit");
    }

    #[test]
    fn parses_commit_files_with_rename_records() {
        let output = b"M\0README.md\0R100\0docs/old.md\0docs/new.md\0";
        let response = parse_commit_files(
            "/project".to_owned(),
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
            output,
        )
        .unwrap();
        assert_eq!(response.files.len(), 2);
        assert_eq!(response.files[0].status, "M");
        assert_eq!(response.files[1].path, "docs/new.md");
        assert_eq!(
            response.files[1].previous_path.as_deref(),
            Some("docs/old.md")
        );
    }

    #[test]
    fn accepts_only_full_hex_commit_ids() {
        assert!(validate_commit_id("0123456789abcdef0123456789abcdef01234567").is_ok());
        assert!(validate_commit_id(
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        )
        .is_ok());
        for commit in [
            "HEAD",
            "deadbeef",
            "0123456789abcdef0123456789abcdef0123456g",
        ] {
            assert!(validate_commit_id(commit).is_err());
        }
    }

    #[test]
    fn command_arguments_keep_literal_paths_after_double_dash() {
        let path = "docs/data [final].csv";
        assert_eq!(
            diff_arguments("/project", path, GitDiffScope::Staged, false),
            [
                "--literal-pathspecs",
                "-C",
                "/project",
                "diff",
                "--no-ext-diff",
                "--no-color",
                "--unified=3",
                "--cached",
                "--",
                path,
            ]
        );
        assert_eq!(
            mutation_arguments("/project", &["-report.csv".to_owned()], true, true),
            [
                "--literal-pathspecs",
                "-C",
                "/project",
                "add",
                "--",
                "-report.csv",
            ]
        );
        assert_eq!(
            commit_diff_arguments(
                "/project",
                "0123456789abcdef0123456789abcdef01234567",
                "docs/data [final].csv",
            ),
            [
                "--literal-pathspecs",
                "-C",
                "/project",
                "show",
                "--format=",
                "--first-parent",
                "--no-ext-diff",
                "--no-color",
                "--unified=3",
                "0123456789abcdef0123456789abcdef01234567",
                "--",
                "docs/data [final].csv",
            ]
        );
    }

    #[test]
    fn generated_history_commands_read_real_commits_and_files() {
        let repository = TestRepository::new();
        std::fs::write(repository.path().join("README.md"), "first\n").unwrap();
        repository.git(&["add", "--", "README.md"]);
        repository.git(&["commit", "-m", "first"]);
        std::fs::write(repository.path().join("README.md"), "second\n").unwrap();
        repository.git(&["commit", "-am", "second"]);

        let root = repository.path().to_string_lossy().into_owned();
        let history_output = repository.run_arguments(&history_arguments(&root, 0, 1));
        assert!(history_output.status.success());
        let history = parse_history(root.clone(), &history_output.stdout, 1).unwrap();
        assert_eq!(history.commits.len(), 1);
        assert!(history.has_more);
        assert_eq!(history.commits[0].subject, "second");

        let commit = &history.commits[0].hash;
        let files_output = repository.run_arguments(&commit_files_arguments(&root, commit));
        assert!(files_output.status.success());
        let files = parse_commit_files(root.clone(), commit.clone(), &files_output.stdout).unwrap();
        assert_eq!(files.files.len(), 1);
        assert_eq!(files.files[0].path, "README.md");

        let diff_output =
            repository.run_arguments(&commit_diff_arguments(&root, commit, "README.md"));
        assert!(diff_output.status.success());
        assert!(String::from_utf8_lossy(&diff_output.stdout).contains("+second"));
    }

    #[test]
    fn resolves_repository_root_before_using_status_paths() {
        let repository = TestRepository::new();
        std::fs::create_dir_all(repository.path().join("src/nested")).unwrap();
        std::fs::write(repository.path().join("src/nested/example.txt"), "before\n").unwrap();
        repository.git(&["add", "--", "src/nested/example.txt"]);
        repository.git(&["commit", "-m", "baseline"]);
        std::fs::write(repository.path().join("src/nested/example.txt"), "after\n").unwrap();

        let nested = repository.path().join("src");
        let nested = nested.to_string_lossy().into_owned();
        let root_output = repository.run_arguments(&repository_root_arguments(&nested));
        assert!(root_output.status.success());
        let root = parse_repository_root(&root_output.stdout).unwrap();
        assert_eq!(
            std::fs::canonicalize(Path::new(&root)).unwrap(),
            std::fs::canonicalize(repository.path()).unwrap()
        );

        let status_output = repository.run_arguments(&status_arguments(&root));
        assert!(status_output.status.success());
        let status = parse_status(root.clone(), &status_output.stdout).unwrap();
        let file = status
            .files
            .iter()
            .find(|file| file.path == "src/nested/example.txt")
            .unwrap();
        assert!(file.unstaged);

        let diff_output = repository.run_arguments(&diff_arguments(
            &root,
            &file.path,
            GitDiffScope::Unstaged,
            false,
        ));
        assert!(diff_output.status.success());
        assert!(String::from_utf8_lossy(&diff_output.stdout).contains("+after"));
    }

    #[test]
    fn unborn_repository_uses_index_only_removal_to_unstage() {
        assert_eq!(
            mutation_arguments("/project", &["README.md".to_owned()], false, false),
            [
                "--literal-pathspecs",
                "-C",
                "/project",
                "rm",
                "--cached",
                "--",
                "README.md",
            ]
        );
    }

    #[test]
    fn generated_commands_cover_real_staged_unstaged_and_literal_paths() {
        let repository = TestRepository::new();
        std::fs::write(repository.path().join("README.md"), "# Baseline\n").unwrap();
        std::fs::write(
            repository.path().join("data report.csv"),
            "name,score\na,1\n",
        )
        .unwrap();
        repository.git(&["add", "--", "README.md", "data report.csv"]);
        repository.git(&["commit", "-m", "baseline"]);

        std::fs::write(repository.path().join("README.md"), "# Changed\n").unwrap();
        std::fs::write(
            repository.path().join("data report.csv"),
            "name,score\na,2\n",
        )
        .unwrap();
        repository.git(&["add", "--", "data report.csv"]);
        std::fs::write(
            repository.path().join("data report.csv"),
            "name,score\na,2\nb,3\n",
        )
        .unwrap();
        std::fs::write(repository.path().join("-notes.md"), "untracked\n").unwrap();

        let root = repository.path().to_string_lossy().into_owned();
        let output = repository.run_arguments(&status_arguments(&root));
        assert!(output.status.success());
        let status = parse_status(root.clone(), &output.stdout).unwrap();
        let readme = status
            .files
            .iter()
            .find(|file| file.path == "README.md")
            .unwrap();
        assert!(readme.unstaged && !readme.staged);
        let csv = status
            .files
            .iter()
            .find(|file| file.path == "data report.csv")
            .unwrap();
        assert!(csv.staged && csv.unstaged);
        assert!(status
            .files
            .iter()
            .any(|file| file.path == "-notes.md" && file.untracked));

        let staged = repository.run_arguments(&diff_arguments(
            &root,
            "data report.csv",
            GitDiffScope::Staged,
            false,
        ));
        assert!(staged.status.success());
        assert!(String::from_utf8_lossy(&staged.stdout).contains("+a,2"));

        let stage = repository.run_arguments(&mutation_arguments(
            &root,
            &["-notes.md".to_owned()],
            true,
            true,
        ));
        assert!(stage.status.success());
        let unstage = repository.run_arguments(&mutation_arguments(
            &root,
            &["-notes.md".to_owned()],
            false,
            true,
        ));
        assert!(unstage.status.success());
        assert!(repository.path().join("-notes.md").is_file());
    }

    #[test]
    fn unborn_unstage_preserves_the_working_tree_file() {
        let repository = TestRepository::new();
        std::fs::write(repository.path().join("README.md"), "draft\n").unwrap();
        repository.git(&["add", "--", "README.md"]);
        let root = repository.path().to_string_lossy().into_owned();
        let output = repository.run_arguments(&status_arguments(&root));
        let status = parse_status(root.clone(), &output.stdout).unwrap();
        assert!(!status.has_head);
        let output = repository.run_arguments(&mutation_arguments(
            &root,
            &["README.md".to_owned()],
            false,
            status.has_head,
        ));
        assert!(output.status.success());
        assert!(repository.path().join("README.md").is_file());
        let output = repository.run_arguments(&status_arguments(&root));
        let status = parse_status(root, &output.stdout).unwrap();
        assert!(status.files[0].untracked);
    }
}
