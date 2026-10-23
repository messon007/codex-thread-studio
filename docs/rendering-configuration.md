# Markdown and Mermaid rendering configuration

Studio keeps rendering preferences in its ordinary `settings.json`; there is intentionally no settings-dialog control for them yet.

- Linux and macOS: `$XDG_CONFIG_HOME/codex-thread-studio/settings.json`, or
  `~/.config/codex-thread-studio/settings.json` when `XDG_CONFIG_HOME` is unset. macOS uses this same
  path rather than `~/Library/Application Support`.
- Windows: `%XDG_CONFIG_HOME%\codex-thread-studio\settings.json`. Define `XDG_CONFIG_HOME` or `HOME`
  before launching, otherwise Studio falls back to its temporary directory.

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

Document Preview uses Comrak 0.57.0 WebAssembly in a module Worker. The original
source remains available for Source/Edit; HTML still passes through the existing
DOMPurify policy and diagram/image postprocessing on the UI thread. Chat message
rendering remains on Marked.

Documents containing `@startuml` explicitly use the existing Marked parser in
the Worker for raw/nested PlantUML support, which the Comrak adapter does not yet
provide. This is syntax-based routing, not a failure fallback. Ordinary Markdown
uses only Comrak: WASM loading or parsing failures display an error rather than
silently switching engines. Closing a document or
opening another terminates its parser Worker; parsing is limited to 30 seconds.
Each completed Worker is terminated as well, releasing its WASM heap. This does
not remove UI-thread sanitization/layout costs or change the 5 MiB text-file cap.

`markdown.mode` accepts:

- `reading`: larger type, more line height, and more whitespace around paragraphs, headings, lists, quotes, tables, and code. Intended for books and long explanations.
- `technical`: balanced density with prominent tables and code. This is the default.
- `compact`: smaller type, tighter vertical rhythm, and denser code blocks. Intended for logs, checklists, and scanning.

These modes change presentation only. They do not change Markdown parsing or the document content. Fenced `text`, `plaintext`, and `txt` blocks are always presented without a language header or copy button; programming-language blocks retain both.

Diagrams render offline in fenced `mermaid`, `plantuml` (also `puml` or `uml`), and `d2` blocks. Standalone `@startuml`…`@enduml` blocks also render as PlantUML. Fenced PlantUML without the delimiters is wrapped automatically. All three formats retain a source toggle and show their source if rendering fails or exceeds 100,000 characters. PlantUML external includes, themes, imports, and data-loading functions are not available in Markdown; D2 file imports are also unavailable. The generated SVG is sanitized before display.

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
