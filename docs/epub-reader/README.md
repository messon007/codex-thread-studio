# EPUB reader

## Product boundary

EPUB is another Document presentation in Codex Thread Studio. It does not create a
second reader window and it does not turn Studio into a book-library manager. The
chat and session list remain visible while the selected session's book occupies the
shared right workspace.

The first release supports the reading loop that matters for AI-assisted study:

- open an unencrypted `.epub` below the selected session directory;
- inspect a hierarchical table of contents and jump to a chapter;
- move to the previous or next page/section and see current chapter/progress;
- choose paginated or continuous reading;
- choose light, sepia, or dark paper and adjust type size;
- restore the most recent CFI and reader preferences for the same local book;
- select text, add an optional question, and add the excerpt to the current chat's
  shared Comment draft;
- reopen a draft at its EPUB CFI while the local book still exists.

DRM, library/catalog management, format conversion, cloud synchronization,
text-to-speech, publisher scripting, audio/video overlays, and editing an EPUB are
out of scope.

## Interaction model

The existing Document header remains the visual anchor. Text/source/edit controls
are hidden for EPUB. A compact book toolbar contains:

- Contents toggle;
- previous and next page;
- a reading settings popover (font size, paper theme, and flow);
- the shared reload and close actions.

The contents drawer lives inside the Document rail and never covers Chat. On a
narrow rail it overlays the book canvas; on a wider rail it takes a fixed column.
The footer shows the current chapter and percentage. Reader controls stay terse;
their full meaning is available through tooltips and accessible labels.

EPUB selection uses the existing selection popover and annotation dialog. The
result is a normal Comment draft with an EPUB-specific source anchor, so composing,
deleting, and sending comments need no reader-specific implementation.

## Data and anchors

An EPUB source anchor contains bounded values only:

- project root and relative file path;
- the book content hash;
- EPUB CFI range;
- spine href and chapter label for human-readable fallback;
- selected excerpt and user note remain in the generic Comment draft.

Reading state is structured data stored in the Studio SQLite database and keyed by
canonical path plus content hash. It contains the latest CFI, progress, font scale,
paper theme, flow, and update time. A replaced book starts with fresh state rather
than applying a stale CFI to different content.

## Security and resource limits

EPUB is an untrusted ZIP of web resources.

- The backend confines the canonical path to the selected project root.
- Input is capped at 128 MiB, 10,000 entries, 512 MiB declared uncompressed data,
  and 64 MiB for any one entry before bytes reach the WebView.
- The archive must contain `META-INF/container.xml`; encrypted/DRM publications are
  rejected.
- EPUB.js scripted content remains disabled. Rendered chapter documents have
  scripts, active embeds, forms, refresh metadata, and event handlers removed by a
  rendition hook.
- The Studio Content Security Policy allows local/data/blob book resources but not
  remote book scripts or active objects.
- External hyperlinks never navigate the Studio application. They are handed to
  the global embedded-browser policy after explicit user activation.
- The EPUB renderer and archive buffer are destroyed when the document closes or a
  different file is opened.

## Performance

- EPUB.js is a separate, pinned, vendored bundle and is dynamically imported only
  when the first EPUB is opened.
- The file is fetched once as an `ArrayBuffer`; sections are rendered on demand by
  the engine.
- Location generation is not required before first paint. Percentage uses engine
  location data when available and a spine fallback otherwise.
- Reading-state writes are debounced and use one row per book version.

## Later extensions

The first release intentionally keeps the toolbar small. Full-book search,
bookmarks/highlights, reading statistics, text-to-speech, and library management can
be added later without changing the EPUB CFI anchor or the generic Comment contract.
Search should be implemented as a cancellable, bounded background scan over spine
sections rather than delaying first paint.

## Acceptance checklist

- [ ] A normal EPUB 2 or EPUB 3 book opens from Files and from the composer `@` menu.
- [ ] Invalid, oversized, encrypted, or out-of-root books fail without rendering.
- [ ] The first page appears without starting a backend model or terminal.
- [ ] Nested table-of-contents items jump to the expected section.
- [ ] Previous/next, keyboard arrow navigation, paginated flow, and continuous flow work.
- [ ] Font size and light/sepia/dark paper remain readable in both Studio themes.
- [ ] Closing and reopening the same unchanged book restores its reading position.
- [ ] Selecting text can create two or more independent Comment drafts without overwriting.
- [ ] A sent draft contains the excerpt, question, book path, chapter, and CFI.
- [ ] EPUB scripts and remote active content do not execute.
- [ ] Closing EPUB releases its object URLs, iframe, hooks, and archive buffer.
- [ ] Existing Markdown, HTML, image, Files, Terminal, Map, and Browser behavior is unchanged.

## References

- W3C EPUB 3.3: <https://www.w3.org/TR/epub-33/>
- EPUB.js: <https://github.com/futurepress/epub.js>
- Foliate JS security and book interface: <https://github.com/johnfactotum/foliate-js>
