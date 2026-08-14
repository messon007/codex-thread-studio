# Session directory filtering

Studio can hide sessions from selected project directories without deleting, archiving, or modifying their backend history. The filter applies to both Codex and OpenCode entries in every sidebar view and count.

Edit the ordinary Studio settings file while Studio is closed:

- Linux: `~/.config/codex-thread-studio/settings.json`
- macOS: `~/Library/Application Support/codex-thread-studio/settings.json`
- Windows: `%APPDATA%\codex-thread-studio\settings.json`

Add Gitignore-style rules under `sessionDirectoryIgnore` at the top level:

```json
{
  "sessionDirectoryIgnore": [
    "# Hide this complete directory tree",
    "/home/rui/desktop/lisource/aswcodex/",
    "**/node_modules/",
    "scratch-*",
    "!/home/rui/desktop/lisource/aswcodex/keep-this/"
  ]
}
```

Rules are evaluated in order and later rules override earlier rules. Studio supports the Gitignore constructs useful for directories: `*`, `**`, `?`, character ranges such as `[0-9]`, trailing `/`, `!` negation, and `#` comments. A rule without `/` matches a directory name at any level. Absolute rules match the complete session directory. POSIX and WSL paths are case-sensitive; Windows drive and UNC paths are case-insensitive. Prefer `/` separators in patterns.

The older `hiddenSessionDirectories` array remains supported. Its entries match one exact directory plus every descendant, but new configurations should use `sessionDirectoryIgnore`.

Directory-filtered sessions are also excluded from Thread Router's regular target catalog and fallback target picker. Search text and the temporary All/Active/Attention sidebar views do not affect routing targets.

Studio reads these options at startup. Close Studio before editing because later UI preference saves rewrite `settings.json`. Remove or negate a rule and restart Studio to show those sessions again.
