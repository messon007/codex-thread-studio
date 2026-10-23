import { EditorState, Compartment, ChangeSet } from '@codemirror/state'
import { EditorView, keymap, lineNumbers, drawSelection, highlightActiveLine, highlightActiveLineGutter } from '@codemirror/view'
import { history, historyKeymap, defaultKeymap, undo, redo, undoDepth, redoDepth, indentWithTab } from '@codemirror/commands'
import { syntaxHighlighting, defaultHighlightStyle, bracketMatching } from '@codemirror/language'
import { markdown } from '@codemirror/lang-markdown'
import { javascript, typescriptLanguage } from '@codemirror/lang-javascript'
import { json } from '@codemirror/lang-json'
import { python } from '@codemirror/lang-python'
import { cpp } from '@codemirror/lang-cpp'
import { rust } from '@codemirror/lang-rust'
import { yaml } from '@codemirror/lang-yaml'
import { oneDark } from '@codemirror/theme-one-dark'

const languages = { javascript: javascript().language, js: javascript().language, typescript: typescriptLanguage, ts: typescriptLanguage, json: json().language, python: python().language, py: python().language, cpp: cpp().language, c: cpp().language, rust: rust().language, rs: rust().language, yaml: yaml().language, yml: yaml().language }
const encoder = new TextEncoder()

export function codeView(parent, options = {}) {
  let line = options.line || 1
  let bytes = 0
  let readOnly = Boolean(options.readOnly)
  let base
  let changes
  const editing = new Compartment()
  const theme = new Compartment()
  const style = EditorView.theme({
    '&': { height: '100%', backgroundColor: 'var(--reader-panel)', color: 'var(--reader-text)' },
    '.cm-scroller': { fontFamily: 'var(--code-font-family,Consolas,monospace)', fontSize: '13px', lineHeight: '24px', overflow: 'auto' },
    '.cm-content': { padding: '12px 0' },
    '.cm-gutters': { backgroundColor: 'var(--reader-soft)', color: 'var(--reader-faint)', borderColor: 'var(--reader-border)' },
    '.cm-lineNumbers .cm-gutterElement': { minWidth: '48px', padding: '0 10px' },
    '&.cm-focused': { outline: 'none' },
  })
  const color = () => document.documentElement.dataset.theme === 'dark' ? oneDark : syntaxHighlighting(defaultHighlightStyle)
  const extensions = [
    lineNumbers({ formatNumber: number => String(line + number - 1) }), style,
    theme.of(color()), editing.of([EditorState.readOnly.of(readOnly), EditorView.editable.of(!readOnly)]),
    markdown({ codeLanguages: info => languages[info.split(/\s/u)[0].toLowerCase()] || null }),
    bracketMatching(), drawSelection(),
    ...(options.readOnly ? [] : [history({ minDepth: 100 }), highlightActiveLine(), highlightActiveLineGutter(), keymap.of([...historyKeymap, ...defaultKeymap, indentWithTab, { key: 'Mod-s', run: () => { options.save?.(); return true } }])]),
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
      options.changed?.()
    }),
  ]
  const view = new EditorView({ parent })
  const observer = new MutationObserver(() => view.dispatch({ effects: theme.reconfigure(color()) }))
  observer.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] })
  return {
    view,
    load(text, startLine) {
      line = startLine
      bytes = encoder.encode(text).length
      view.setState(EditorState.create({ doc: text, extensions }))
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
    async reveal(target) {
      const relative = Math.max(1, Math.min(view.state.doc.lines, target - line + 1))
      view.dispatch({ selection: { anchor: view.state.doc.line(relative).from }, effects: EditorView.scrollIntoView(view.state.doc.line(relative).from, { y: 'start' }) })
      await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))
    },
    focus: () => view.focus(),
    dispose() { observer.disconnect(); view.destroy() },
  }
}
