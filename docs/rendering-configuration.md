# Markdown and Mermaid rendering configuration

Studio keeps rendering preferences in its ordinary `settings.json`; there is intentionally no settings-dialog control for them yet.

- Linux: `~/.config/codex-thread-studio/settings.json`
- macOS: `~/Library/Application Support/codex-thread-studio/settings.json`
- Windows: `%APPDATA%\\codex-thread-studio\\settings.json`

Close Studio before editing the file, then restart it. Studio writes the defaults into the file on startup and preserves them when other settings are saved.

```json
{
  "markdown": {
    "mode": "technical"
  },
  "mermaid": {
    "style": "auto",
    "density": "standard",
    "curve": "rounded",
    "layout": "auto",
    "fontSize": 15
  }
}
```

The file also contains other Studio preferences; keep those fields intact.

## Markdown

`markdown.mode` accepts:

- `reading`: larger type, more line height, and more whitespace around paragraphs, headings, lists, quotes, tables, and code. Intended for books and long explanations.
- `technical`: balanced density with prominent tables and code. This is the default.
- `compact`: smaller type, tighter vertical rhythm, and denser code blocks. Intended for logs, checklists, and scanning.

These modes change presentation only. They do not change Markdown parsing or the document content. Fenced `text`, `plaintext`, and `txt` blocks are always presented without a language header or copy button; programming-language blocks retain both.

## Mermaid

The following values are accepted:

| Field | Allowed values | Meaning |
| --- | --- | --- |
| `style` | `auto`, `classic`, `neo`, `handDrawn`, `document` | Overall Mermaid theme and drawing look. `auto` follows Studio light/dark mode using Mermaid's Neo theme. |
| `density` | `compact`, `standard`, `loose` | Flowchart node spacing, rank spacing, and padding. |
| `curve` | `rounded`, `linear`, `step`, `basis` | Flowchart connector curve. |
| `layout` | `auto`, `dagre`, `elk` | Layout engine. `auto` lets Mermaid choose its default. |
| `fontSize` | integer `12`–`20` | Diagram text size in pixels. |

The diagram font family follows Studio's configured UI font. For security and reliable multilingual SVG text, the following are fixed and cannot be overridden: `securityLevel: strict`, `startOnLoad: false`, `suppressErrorRendering: true`, and `htmlLabels: false`. Diagram source is limited to 100,000 characters and generated SVG is sanitized again before display.
