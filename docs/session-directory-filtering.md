# Session directory filtering

Studio can hide sessions from selected project directories without deleting, archiving, or modifying their backend history. The filter applies to both Codex and OpenCode entries in every sidebar view and count.

Edit the ordinary Studio settings file while Studio is closed:

- Linux and macOS: `$XDG_CONFIG_HOME/codex-thread-studio/settings.json`, or
  `~/.config/codex-thread-studio/settings.json` when `XDG_CONFIG_HOME` is unset. macOS uses this same
  path rather than `~/Library/Application Support`.
- Windows: `%XDG_CONFIG_HOME%\codex-thread-studio\settings.json`. Define `XDG_CONFIG_HOME` or `HOME`
  before launching, otherwise Studio falls back to its temporary directory.

Add Gitignore-style rules under `sessionDirectoryIgnore` at the top level:

```json
{
  "sessionDirectoryIgnore": [
    "# Hide this complete directory tree",
    "/home/user/projects/private-workspace/",
    "**/node_modules/",
    "scratch-*",
    "!/home/user/projects/private-workspace/keep-this/"
  ]
}
```

Rules are evaluated in order and later rules override earlier rules. Studio supports the Gitignore constructs useful for directories: `*`, `**`, `?`, character ranges such as `[0-9]`, trailing `/`, `!` negation, and `#` comments. A rule without `/` matches a directory name at any level. Absolute rules match the complete session directory. POSIX and WSL paths are case-sensitive; Windows drive and UNC paths are case-insensitive. Prefer `/` separators in patterns.

The older `hiddenSessionDirectories` array remains supported. Its entries match one exact directory plus every descendant, but new configurations should use `sessionDirectoryIgnore`.

Directory-filtered sessions are also excluded from Thread Router's regular target catalog and fallback target picker. Search text and the temporary All/Active/Attention sidebar views do not affect routing targets.

Studio reads these options at startup. Close Studio before editing because later UI preference saves rewrite `settings.json`. Remove or negate a rule and restart Studio to show those sessions again.
