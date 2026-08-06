# Third-party notices

Codex Thread Studio vendors the following browser assets so Markdown rendering works without a network connection:

- Marked 17.0.2 — MIT — `ui/vendor/marked.esm.js`
- DOMPurify 3.4.12 — Apache-2.0 or MPL-2.0 — `ui/vendor/purify.es.mjs`
- GitHub Markdown CSS 5.8.1 — MIT — `ui/vendor/github-markdown.css`
- Mermaid 11.16.1 — MIT — `ui/vendor/mermaid.min.js`

The Mermaid standalone bundle retains the upstream license banners for its bundled transitive components.

The corresponding license texts are included in `ui/vendor/licenses/`. The files are copied from exact, lockfile-pinned npm packages by `npm run vendor:markdown`; they should not be edited by hand.
