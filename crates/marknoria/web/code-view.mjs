import { search, searchKeymap, openSearchPanel, closeSearchPanel, searchPanelOpen } from '@codemirror/search'
import { EditorState, Compartment, ChangeSet } from '@codemirror/state'
import { EditorView, keymap, lineNumbers, drawSelection, highlightActiveLine, highlightActiveLineGutter } from '@codemirror/view'
import { history, historyKeymap, defaultKeymap, undo, redo, undoDepth, redoDepth, indentWithTab } from '@codemirror/commands'
import { syntaxHighlighting, defaultHighlightStyle, bracketMatching, StreamLanguage } from '@codemirror/language'
import { markdown } from '@codemirror/lang-markdown'
import { javascript, typescriptLanguage } from '@codemirror/lang-javascript'
import { json } from '@codemirror/lang-json'
import { python } from '@codemirror/lang-python'
import { cpp } from '@codemirror/lang-cpp'
import { rust } from '@codemirror/lang-rust'
import { yaml } from '@codemirror/lang-yaml'
import { toml } from '@codemirror/legacy-modes/mode/toml'
import { oneDark } from '@codemirror/theme-one-dark'
import { mountWordHighlights } from './word-highlights.mjs'

const languages = { javascript: javascript().language, js: javascript().language, typescript: typescriptLanguage, ts: typescriptLanguage, json: json().language, python: python().language, py: python().language, cpp: cpp().language, c: cpp().language, rust: rust().language, rs: rust().language, yaml: yaml().language, yml: yaml().language, toml: StreamLanguage.define(toml) }
const syntax = language => ['markdown', 'md'].includes(language) ? markdown({ codeLanguages: info => languages[info.split(/\s/u)[0].toLowerCase()] || null }) : languages[language] || []
const encoder = new TextEncoder()

export function codeView(parent, options = {}) {
  let line = options.line || 1
  let bytes = 0
  let readOnly = Boolean(options.readOnly)
  let base
  let changes
  let selectedWords
  const editing = new Compartment()
  const theme = new Compartment()
  const phrases = new Compartment()
  const searchPhrases = ['Find','Replace','next','previous','all','match case','regexp','by word','replace','replace all','close','current match','replaced $ matches','replaced $ match','replaced match on line $','on line','Go to line','go']
  const translatedPhrases = () => Object.fromEntries(searchPhrases.map(key => [key,(options.translate || String)(key)]))
  const style = EditorView.theme({
    '&': { height: '100%', backgroundColor: 'var(--reader-panel)', color: 'var(--reader-text)' },
    '.cm-scroller': { fontFamily: 'var(--code-font-family,Consolas,monospace)', fontSize: 'var(--code-font-size,13px)', lineHeight: 'max(24px, 1.7em)', overflow: 'auto' },
    '.cm-content': { padding: '12px 0' },
    '.cm-gutters': { backgroundColor: 'var(--reader-soft)', color: 'var(--reader-faint)', borderColor: 'var(--reader-border)' },
    '.cm-lineNumbers .cm-gutterElement': { minWidth: '48px', padding: '0 10px' },
    '&.cm-focused': { outline: 'none' },
  })
  const color = () => document.documentElement.dataset.theme === 'dark' ? oneDark : syntaxHighlighting(defaultHighlightStyle)
  const extensions = [
    lineNumbers({ formatNumber: number => String(line + number - 1) }), style,
    theme.of(color()), editing.of([EditorState.readOnly.of(readOnly), EditorView.editable.of(!readOnly)]),
    phrases.of(EditorState.phrases.of(translatedPhrases())), search({ top: true }), keymap.of(searchKeymap),
    bracketMatching(), drawSelection(),
    ...(options.readOnly ? [EditorView.contentAttributes.of({ tabindex: '0' }), keymap.of(defaultKeymap)] : [history({ minDepth: 100 }), highlightActiveLine(), highlightActiveLineGutter(), keymap.of([...historyKeymap, ...defaultKeymap, indentWithTab, { key: 'Mod-s', run: () => { options.save?.(); return true } }])]),
    EditorState.changeFilter.of(transaction => {
      if (!transaction.docChanged || !options.maxBytes) return true
      let difference = 0
      transaction.changes.iterChanges((from, to, _newFrom, _newTo, inserted) => { difference += encoder.encode(inserted.toString()).length - encoder.encode(transaction.startState.sliceDoc(from, to)).length })
      if (bytes + difference > options.maxBytes) { options.error?.('The current editing page is limited to 512 KiB. Split this change into smaller edits.'); return false }
      return true
    }),
    EditorView.updateListener.of(update => {
      if (update.docChanged) {
        update.changes.iterChanges((from, to, _newFrom, _newTo, inserted) => { bytes += encoder.encode(inserted.toString()).length - encoder.encode(update.startState.sliceDoc(from, to)).length })
        changes = changes.compose(update.changes)
        if (update.state.doc.eq(base)) changes = ChangeSet.empty(base.length)
      }
      if (update.selectionSet || update.docChanged || update.viewportChanged || update.geometryChanged) selectedWords?.refresh()
      options.changed?.()
    }),
  ]
  const view = new EditorView({ parent })
  selectedWords = mountWordHighlights(view.contentDOM, {
    viewport: view.scrollDOM,
    selection() {
      const range = view.state.selection.main
      if (range.empty || range.to - range.from > 256) return null
      return {
        word: view.state.sliceDoc(range.from, range.to),
        before: Array.from(view.state.sliceDoc(Math.max(0, range.from - 2), range.from)).at(-1) || '',
        after: Array.from(view.state.sliceDoc(range.to, Math.min(view.state.doc.length, range.to + 2)))[0] || '',
      }
    },
  })
  const unsubscribe = options.translate?.subscribe?.(() => {
    const open = searchPanelOpen(view.state)
    view.dispatch({ effects: phrases.reconfigure(EditorState.phrases.of(translatedPhrases())) })
    // CodeMirror builds panel labels on creation; remount only that small UI,
    // retaining its query, document, selection, undo history and draft.
    if (open) { closeSearchPanel(view); openSearchPanel(view) }
  })
  const observer = new MutationObserver(() => view.dispatch({ effects: theme.reconfigure(color()) }))
  observer.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] })
  return {
    view,
    load(text, startLine, language = 'markdown') {
      line = startLine
      bytes = encoder.encode(text).length
      selectedWords.clear()
      view.setState(EditorState.create({ doc: text, extensions: [...extensions, syntax(language)] }))
      base = view.state.doc
      changes = ChangeSet.empty(base.length)
      options.changed?.()
    },
    get dirty() { return changes ? !changes.empty : false },
    get text() { return view.state.doc.toString() },
    patches() {
      const result = []
      changes.iterChanges((from, to, _newFrom, _newTo, inserted) => result.push({ from, to, insert: inserted.toString() }))
      return result
    },
    saved() { base = view.state.doc; changes = ChangeSet.empty(base.length); options.changed?.() },
    lock(locked) { readOnly = locked || Boolean(options.readOnly); view.dispatch({ effects: editing.reconfigure([EditorState.readOnly.of(readOnly), EditorView.editable.of(!readOnly)]) }) },
    undo: () => undo(view), redo: () => redo(view),
    get canUndo() { return undoDepth(view.state) > 0 }, get canRedo() { return redoDepth(view.state) > 0 },
    cursorLine() { return line + view.state.doc.lineAt(view.state.selection.main.head).number - 1 },
    get lineDelta() { return view.state.doc.lines - base.lines },
    mappedLine(target) {
      const relative = Math.max(1, Math.min(base.lines, target - line + 1))
      const position = changes.mapPos(base.line(relative).from, 1)
      return line + view.state.doc.lineAt(position).number - 1
    },
    topLine() { return line + view.state.doc.lineAt(view.lineBlockAtHeight(view.scrollDOM.scrollTop).from).number - 1 },
    async select(from, to) {
      if (![from,to].every(Number.isSafeInteger) || from < 0 || to < from || to > view.state.doc.length) throw Error('Source selection out of range')
      view.dispatch({ selection: { anchor: from, head: to }, effects: EditorView.scrollIntoView(from, { y: 'center' }) })
      view.focus()
      await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))
    },
    async reveal(target) {
      const relative = Math.max(1, Math.min(view.state.doc.lines, target - line + 1))
      view.dispatch({ selection: { anchor: view.state.doc.line(relative).from }, effects: EditorView.scrollIntoView(view.state.doc.line(relative).from, { y: 'start' }) })
      await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))
    },
    focus: () => view.focus(),
    scrollSnapshot: () => view.scrollSnapshot(),
    remeasure(snapshot) { if (snapshot) view.dispatch({ effects: snapshot }); view.requestMeasure() },
    search: () => openSearchPanel(view),
    // Edit instances always stay off, including temporary save/read-only locks.
    wordHighlights(enabled) { selectedWords.update({ enabled: Boolean(options.readOnly && enabled) }) },
    wordHighlightState: () => selectedWords.state(),
    dispose() { selectedWords.dispose(); unsubscribe?.(); observer.disconnect(); view.destroy() },
  }
}
