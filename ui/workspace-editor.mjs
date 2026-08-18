import {
  basicSetup,
  EditorState,
  EditorView,
  indentWithTab,
  keymap,
  markdown,
  openSearchPanel,
} from './vendor/workspace-editor.mjs'

const editorTheme = EditorView.theme({
  '&': {
    height: '100%',
    width: '100%',
    minWidth: '0',
    maxWidth: '100%',
    flex: '1 1 auto',
    backgroundColor: 'var(--panel)',
    color: 'var(--text)',
    fontFamily: 'var(--code-font-family)',
    fontSize: 'var(--code-font-size)',
    fontWeight: 'var(--code-font-weight)',
  },
  '.cm-scroller': { width: '100%', minWidth: '0', overflow: 'auto', lineHeight: '1.62' },
  '.cm-content': { padding: '18px 0 52px', caretColor: 'var(--brand)' },
  '.cm-line': { padding: '0 20px' },
  '.cm-gutters': {
    minWidth: '46px',
    paddingTop: '18px',
    borderRight: '1px solid var(--border)',
    backgroundColor: 'var(--panel-soft)',
    color: 'var(--faint)',
  },
  '.cm-activeLine, .cm-activeLineGutter': { backgroundColor: 'var(--brand-soft)' },
  '&.cm-focused': { outline: 'none', boxShadow: 'inset 2px 0 0 var(--brand)' },
  '.cm-selectionBackground, &.cm-focused .cm-selectionBackground': { backgroundColor: 'var(--brand-soft) !important' },
  '.cm-searchMatch': { backgroundColor: 'var(--amber-soft)', outline: '1px solid var(--amber)' },
  '.cm-panels': {
    borderColor: 'var(--border)',
    backgroundColor: 'var(--panel-soft)',
    color: 'var(--muted)',
    fontFamily: 'var(--ui-font-family)',
    fontSize: '11px',
  },
  '.cm-panel.cm-search': { padding: '8px 38px 8px 10px', display: 'flex', alignItems: 'center', gap: '6px', flexWrap: 'wrap' },
  '.cm-panel.cm-search label': { display: 'inline-flex', alignItems: 'center', gap: '4px' },
  '.cm-panel.cm-search input': {
    height: '28px',
    padding: '0 8px',
    border: '1px solid var(--border-strong)',
    borderRadius: '7px',
    outline: 'none',
    backgroundColor: 'var(--panel)',
    color: 'var(--text)',
    font: '500 11px var(--ui-font-family)',
  },
  '.cm-panel.cm-search input:focus': { borderColor: 'var(--brand)', boxShadow: '0 0 0 3px var(--brand-soft)' },
  '.cm-panel.cm-search button': {
    minHeight: '27px',
    padding: '0 9px',
    border: '1px solid var(--border)',
    borderRadius: '7px',
    backgroundColor: 'var(--panel)',
    color: 'var(--muted)',
    font: '600 10px var(--ui-font-family)',
  },
  '.cm-panel.cm-search button:hover': { borderColor: 'var(--border-strong)', color: 'var(--text)' },
  '.cm-panel.cm-search [name=close]': { top: '8px', right: '10px', width: '27px', padding: '0', fontSize: '15px' },
})

export function createWorkspaceEditor({ parent, content = '', language = 'text', onChange, onSave }) {
  if (!parent) throw new Error('Workspace editor requires a parent element')
  const extensions = [
    basicSetup,
    editorTheme,
    keymap.of([
      indentWithTab,
      { key: 'Mod-s', run: () => { onSave?.(); return true } },
    ]),
    EditorView.updateListener.of((update) => {
      if (update.docChanged) onChange?.(update.state.doc.toString())
    }),
  ]
  if (language === 'markdown') extensions.push(markdown())
  const view = new EditorView({
    parent,
    state: EditorState.create({ doc: String(content), extensions }),
  })
  return {
    view,
    focus: () => view.focus(),
    openSearch: () => openSearchPanel(view),
    value: () => view.state.doc.toString(),
    revealOffset: (offset) => {
      const anchor = Math.min(view.state.doc.length, Math.max(0, Number(offset) || 0))
      view.dispatch({
        selection: { anchor },
        effects: EditorView.scrollIntoView(anchor, { y: 'center' }),
      })
      view.focus()
    },
    destroy: () => view.destroy(),
  }
}
