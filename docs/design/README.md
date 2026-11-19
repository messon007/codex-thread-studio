# Design guidance and public assets

Start UI work with the [interaction and interface guidelines](interaction-guidelines.md)
and [typography standard](../typography.md). The guidelines consolidate user
decisions across the project's conversation history; feature specifications retain
their detailed behavior contracts. Update the guidelines when a later user decision
changes a durable convention, and record the scope of exceptions.

## Public assets

Keep intentionally selected, privacy-reviewed design and acceptance images here.
Use fictional sessions and isolated fixtures; do not commit personal conversation
captures, credentials or machine-specific configuration. Temporary captures belong
outside the repository (for example in a local temporary directory).

The six images in `acceptance-2026-08-13/` support the historical
[acceptance report](../acceptance-2026-08-13.md). They are not screenshots of the
current interface. They were reviewed before relocation from the old root-level
screenshots directory; test paths and test data are intentional.

The repository's root `settings.json` is a sanitized example, not the active user
profile. Shared directories and Router session identities are intentionally empty.
Changing this example does not change an installed Studio profile.

The obsolete EPT patch was removed: it targeted generated JavaScript and no longer
applied. Maintained response presentation and regression tests live in
`ui-src/transcript-presentation.mts` and `ui/transcript-presentation.test.mjs`.
