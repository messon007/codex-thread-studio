# Third-party notices

Codex Thread Studio vendors the following browser assets so rendering and artifact readers work without a network connection:

- Marked 17.0.2 — MIT — `ui/vendor/marked.esm.js`
- DOMPurify 3.4.13 — Apache-2.0 or MPL-2.0 — `ui/vendor/purify.es.mjs`
- GitHub Markdown CSS 5.8.1 — MIT — `ui/vendor/github-markdown.css`
- Mermaid 11.16.1 — MIT — `ui/vendor/mermaid.min.js`
- CodeMirror 6 and its Lezer parser packages — MIT — `ui/vendor/workspace-editor.mjs`
- xterm.js 6.0.0 and Fit addon 0.11.0 — MIT — `ui/vendor/workspace-terminal.mjs`
- EPUB.js 0.3.93 — BSD-2-Clause — `ui/vendor/epub.mjs`
- PDF.js 5.4.296 — Apache-2.0 — `ui/vendor/pdf.min.mjs` and worker bundle
- ExcelJS 4.4.0 — MIT — `ui/vendor/artifact-table.mjs`

The Mermaid standalone bundle retains the upstream license banners for its bundled transitive components.

The CodeMirror bundle also contains its MIT-licensed runtime helpers `crelt`, `style-mod`,
`w3c-keyname`, and `@marijn/find-cluster-break`.

The EPUB.js bundle contains its lockfile-pinned XML, ZIP, storage, event, path, and utility
dependencies. Their BSD-2-Clause, MIT, Apache-2.0, and dual MIT/Zlib license texts are included
beside the bundle. The vulnerable XML parser range requested by EPUB.js is replaced by the audited
`@xmldom/xmldom` 0.8.13 package through an npm override.

The corresponding license texts are included in `ui/vendor/licenses/`. The files are copied from
exact, lockfile-pinned npm packages by `npm run vendor:markdown` and
`npm run vendor:workspace`, `npm run vendor:epub`, and `npm run vendor:artifacts`; they should not be edited by hand.
