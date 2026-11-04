# Studio interface translations

`zh-CN.json` is generated from this repository's `ui/i18n.mjs` exported
`translationEntries` and native-only additions in `../../locales/zh-CN.json`.
The shared catalog keeps its existing project license. No external translation
service or runtime JavaScript dependency is introduced.

Regenerate from the repository root with:

```sh
node scripts/vendor-gnome-i18n.mjs
node scripts/vendor-gnome-i18n.mjs --check
```

The checked-in JSON is compiled into the native executable. Node is a
development/CI tool only. Native additions must not duplicate shared source keys.
Contextual keys use the gettext convention `context\u0004English source`; the
English display falls back to the source after the separator.

Only explicitly registered interface properties use this catalog. Document,
conversation, filename, model-ID and editable user-value widgets are not scanned.
