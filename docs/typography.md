# Typography standard

Applies to both desktop and SSH browser clients, in every interface language.
Font settings select local client fonts; the SSH host does not rasterize text.

## Ownership

| Content | Profile |
| --- | --- |
| Navigation, controls, settings, status, metadata, counts | Interface |
| Messages, progress, annotations, translations, readable document text | Reading Content |
| Explicit code, source editor, terminal output | Code |

Do not use Code solely because text is technical (model names, paths, revisions,
keyboard shortcuts, status labels). Semantic emphasis is profile weight + 100,
capped at 700; ordinary text uses the profile weight unchanged. Browser-default
`strong`/`b` and fixed 650/750/800 weights must not bypass this rule.

## Interface scale

All sizes derive from the configured Interface base B, with readability floors:

| Role | Size | At B=14 |
| --- | --- | --- |
| Heading | B + 2 | 16px |
| Primary text / inputs | B | 14px |
| Secondary / list title | max(13, B - 1) | 13px |
| Compact actions / field labels | max(12, B - 2) | 12px |
| Hints / metadata | max(11, B - 3) | 11px |
| Numeric badges only | max(10, B - 4) | 10px |

Existing `sm` means compact actions; `xs` means metadata; `xxs` is reserved for
numeric badges. Never use badge sizing for words or Chinese labels. Body text
uses the primary text color; secondary text uses the muted color. Faint colors
are for tertiary/decorative information, not the sole label of an active action.
Keep disabled-state contrast distinct, and preserve explicit selected states.

## Layout and exceptions

Controls must accommodate B=11 through B=20 without vertically clipping labels.
Prefer minimum heights, wrapping, and scrollable overflow over fixed text boxes.
Icon glyphs and logos may retain fixed geometry; document why in tests. Ordinary
text must not use CSS scale transforms to imitate a smaller font.

Reading text derives offsets from Reading Content, not Interface. Markdown
headings and semantic bold preserve document hierarchy relative to that profile.
EPUB applies its reader zoom to Reading Content. PDF page fonts and positions
come from the document, not UI settings. Mermaid uses Reading Content family
and its separately exposed diagram size. External websites retain their own
typography; the embedded browser's Studio-owned toolbar uses Interface settings.
OS-native menus/dialogs may follow OS accessibility settings instead of CSS.

## Verification

Test configuration changes in the rendered browser, not only source regexes:
UI bases 11/14/20, weights 400/500/600, distinct Reading/Code profiles, both
languages and themes. Check settings, Composer/Queue, all side panels, document
toolbars, counts and embedded browser controls. Check clipping, wrapping and
keyboard access. Preserve PDF geometry and code alignment. Do not claim Windows
font rasterization or IME validation from a Linux screenshot.

Run `npm test` and `cargo test --workspace --locked` for regression coverage.
With a separately installed Playwright package, run
`node scripts/smoke-typography.mjs` (optional environment variables
`STUDIO_PLAYWRIGHT_MODULE` and `STUDIO_CHROMIUM_PATH`). It does not launch
backends or access the user's profile; screenshots are written to `/tmp`.
