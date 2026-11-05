# Scoped UI translation

`web/translation.mjs` exports `createTranslator({locale:'en', messages:{...}})` and
`uiLabels(translate,{signal})`. English message keys are the fallback. Hosts can
keep their own `translate(key, variables)` callback; numeric/boolean variables
retain their types. Values are inserted as text/attributes, never as HTML.

```js
const translate = createTranslator({
  locale: 'zh-CN',
  messages: { Notes: '笔记', Copy: '复制', 'Row {row}': '第 {row} 行' }
})
const notes = mountNotes({ ...scopedNoteOptions, translate })
translate.update({ locale: 'en-US', messages: { Notes: 'Notebook' } })
// update replaces the dictionary when supplied; omitting messages retains it.
notes.dispose()
```

The locale is canonical BCP 47, at most 64 characters. A dictionary has at most
512 keys/64 KiB UTF-8, 256-unit keys, 4,096-unit values and 32 placeholders per
value. Variables have at most 16 scalar entries of 256 units each; numeric values are finite and within the safe-integer magnitude (fractions are allowed). Numbers and dates use the explicit locale, defaulting to English. Up to eight number formatters are cached. Use strings for exact identifiers or values; dataset fields are not UI variables. Formatted UI
text is at most 16,384 units. Invalid dictionary/locale updates leave the existing
configuration intact. `translate.state()` reports locale/count/revision and
subscriber count without echoing document data or the dictionary.

`uiLabels` binds only explicit UI nodes and supported text/title/aria-label/
placeholder attributes. It never observes or walks document DOM. Existing labels
and dates update through subscriptions; locale changes do not reparse source,
refetch notes or reset selection. Each scope has at most 1,024 weak node bindings;
obsolete detached bindings are pruned under pressure, and a reused node is rebound
by `set`. Hosts reattaching their own detached UI should bind it again. A translator
has at most 256 subscribers. Abort/dispose removes subscriptions and bindings.
`clear` releases a transient label before literal/error text replaces it, avoiding
resurrection of expired feedback when the locale changes.

Notes uses this interface for controls, dialogs, counts, dates, coordinate labels
and accessibility text. Quotes, comments, document/heading names, result column
names and backend errors remain literal. Source/query provenance remains in its
stable data format; translation does not reinterpret it as a source location.
Source/SQL toolbars, column resize/scroll accessibility labels, full-cell dialogs
and complete-input charts use the same explicit bindings. Locale updates leave
original column names, values, SVG bars and selection unchanged. The full embedded settings bridge accepts bounded `locale`/`messages` and
exposes its presentation version/state (see `embedding.md`). Reader chrome and
source/edit range and dirty labels are integrated. PDF controls/page accessibility labels and EPUB settings/progress controls
also update in place, retaining canvases/text selection and CFIs. The shared jq window also updates live controls/counters while retaining literal
filter/result/example text. Remaining file/status chrome remains integration work; this module alone is not a claim that every reader UI has been localized.

`tests/browser/translation-check.mjs` runs the bounded dictionary/prototype,
parameter, text-injection, scope, date, pruning/remount and disposal contracts.
`notes-component-check.mjs` exercises the actual translated Notes UI, including
updating an already open library without another storage request. Both support
`--webkit`; the Notes test also supports `--compact`.
