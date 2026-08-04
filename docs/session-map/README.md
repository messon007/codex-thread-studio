# Session Map feature package

Session Map is the proposed compact navigation workspace for long-running Codex Thread Studio conversations. It keeps the original goal, current location, remaining scope, and progress visible while a conversation moves into details.

This package is the design baseline, not an implementation claim:

- [Product and technical specification](specification.md)
- [Extensible template system](template-system.md)
- [Visual and interaction rationale](design-rationale.md)
- [Interactive design demo](demo/index.html)

## Review order

1. Open the demo and try the abstract Hierarchy, Path, Flow, and Blank structures.
2. Read the interaction table in the specification. It defines which Map actions may affect the composer.
3. Review the declarative Structure manifest and migration model before choosing a persistence schema.
4. Approve the V1 boundary and acceptance criteria before implementation.

## Run the demo

The demo has no dependencies and makes no network requests. Either open `demo/index.html` directly, or serve this repository and visit the file:

```bash
python3 -m http.server 8765
# http://127.0.0.1:8765/docs/session-map/demo/
```

The refined demo deliberately leaves Studio's current sidebar and conversation unchanged so review can focus on the lightweight Map rail. Frequent actions are icon-first, Item commands use one trailing More menu, and low-frequency Map commands remain under the header More menu. The demo stores custom-Structure experiments in browser `localStorage` only and does not modify real Studio Threads.

## Experimental implementation

The `experiment/session-map` branch now includes the first integrated implementation. Read [implementation.md](implementation.md) before testing; it records the exact optional-creation rule, storage/API boundary, Codex synchronization path, current OpenCode limitation, and verification commands.
