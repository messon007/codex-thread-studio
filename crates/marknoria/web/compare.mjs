import { fileController } from './files.mjs'
import { sourcePane } from './compare-source.mjs'
import { readPosition, writePosition } from './position.mjs'
import { readerTools } from './reader-tools.mjs'

const token = location.hash.slice(1)
const files = fileController(token)
const status = document.querySelector('#compare-status')
const detail = document.querySelector('#compare-line')
const sides = ['left', 'right']
const panes = Object.fromEntries(sides.map(side => [side, { file: null, mode: 'preview', position: { line: 1, view: 'preview' }, diff: null, generation: 0 }]))
let comparisonId = null
let metadata = null
let change = -1
let stopped = false
let opening = false
let generation = 0
let navigation = 0
let detailRequest = null
let detailRow = null
let detailPage = null
let detailGeneration = 0

async function api(path, options = {}) {
  for (let attempt = 0; attempt < 4; attempt += 1) {
    const response = await fetch(new URL(`.${path}`, import.meta.url), { ...options, headers: { 'x-reader-token': token, ...(comparisonId ? { 'x-reader-comparison': comparisonId } : {}), 'Content-Type': 'application/json' } })
    if (response.status === 429 && attempt < 3) { await new Promise(resolve => setTimeout(resolve, 50 * 2 ** attempt)); continue }
    if (!response.ok) throw new Error((await response.text()).slice(0, 800) || `Request failed (${response.status})`)
    const result = await response.json()
    if (path.startsWith('/api/compare') && options.method !== 'POST' && result.comparisonId && result.comparisonId !== comparisonId) throw new Error('Stale comparison response')
    return result
  }
}

const reader = side => {
  const instance = document.getElementById(`${side}-preview`).contentWindow?.readerExperiment
  return instance?.documentId === panes[side].file?.id ? instance : null
}
for (const side of sides) panes[side].tools = readerTools(document.getElementById(`${side}-preview`), document.getElementById(`${side}-tools`), () => reader(side))

function ensureDiff(side) {
  const pane = panes[side]
  if (!metadata) return
  const frame = document.getElementById(`${side}-preview`)
  const owner = reader(side) ? frame.contentDocument : document
  if (pane.diff?.viewport.ownerDocument === owner) return
  if (pane.diff) {
    capture(side)
    pane.diff.dispose()
    if (pane.diff.viewport.ownerDocument !== document) pane.diff.viewport.remove()
  }
  let viewport = document.getElementById(`${side}-diff`)
  if (owner !== document) {
    if (!owner.querySelector('[data-comparison-style]')) {
      const style = owner.createElement('style')
      style.dataset.comparisonStyle = ''
      style.textContent = [...document.querySelector('link[href="./compare.css"]').sheet.cssRules].map(rule => rule.cssText).join('\n')
      owner.head.append(style)
    }
    viewport = owner.createElement('main')
    viewport.id = `${side}-diff`
    viewport.className = 'compare-diff'
    viewport.tabIndex = 0
    viewport.hidden = true
    viewport.setAttribute('aria-label', `${side} source differences`)
    viewport.append(owner.createElement('div'))
    owner.getElementById('workspace').append(viewport)
  }
  pane.diff = sourcePane(viewport, side, metadata, api, openLine)
  if (owner !== document) reader(side).source.attach({
    viewport,
    line: () => pane.diff.line(),
    async go(line) {
      navigation += 1
      const ticket = ++pane.generation
      try {
        const result = await api(`/api/compare/row?side=${side}&line=${line}`)
        if (!stopped && ticket === pane.generation && pane.mode === 'source') pane.diff.show(result.row, line)
      } catch (error) { if (!stopped && ticket === pane.generation) status.textContent = error.message }
    },
  })
}

function capture(side) {
  const pane = panes[side]
  const position = pane.diff?.visible
    ? { line: pane.diff.line() } : reader(side)?.position() || pane.position
  pane.position = { line: position.line, view: pane.mode }
  if (pane.file) writePosition(token, pane.file.id, pane.position)
  return pane.position
}

function controls() {
  const enabled = Boolean(metadata?.changes && sides.every(side => panes[side].mode === 'source'))
  for (const id of ['change-previous', 'change-next']) {
    const button = document.getElementById(id)
    button.disabled = !enabled
    button.title = enabled ? 'Jump to an exact source difference' : 'Available when both documents use Source'
  }
  document.querySelector('#change-position').textContent = enabled && change >= 0 ? `${change + 1} / ${metadata.changes}` : ''
  document.querySelector('.compare-legend').hidden = !metadata || !sides.some(side => panes[side].mode === 'source')
  if (metadata) status.textContent = `${metadata.changes.toLocaleString()} change groups · +${metadata.added.toLocaleString()} −${metadata.removed.toLocaleString()} lines · ${enabled ? 'Exact source differences' : 'Change navigation requires Source on both sides'}${metadata.boundedAlignment ? ' · Time-bounded alignment; large changes may be grouped' : ''} · read only`
}

async function applyMode(side) {
  const pane = panes[side]
  const ticket = ++pane.generation
  const frame = document.getElementById(`${side}-preview`)
  const diff = document.getElementById(`${side}-diff`)
  pane.tools.bind()
  for (const view of ['preview', 'source']) {
    const button = document.getElementById(`${side}-mode-${view}`)
    button.setAttribute('aria-pressed', String(pane.mode === view))
    button.disabled = !pane.file
  }
  diff.hidden = true
  frame.hidden = !pane.file
  if (!pane.file) return
  ensureDiff(side)
  if (pane.mode === 'source' && metadata) {
    if (reader(side)) await reader(side).source.show(pane.position.line)
    else {
      frame.hidden = true
      const result = await api(`/api/compare/row?side=${side}&line=${pane.position.line}`)
      if (ticket !== pane.generation || stopped) return
      pane.diff.show(result.row, pane.position.line)
    }
  } else if (reader(side)) {
    if (pane.mode === 'source') await reader(side).source.show(pane.position.line)
    else frame.contentWindow.postMessage({ type: 'marknoria:navigate', line: pane.position.line }, location.origin)
  }
}

async function setMode(side, mode) {
  capture(side)
  panes[side].mode = mode
  navigation += 1
  controls()
  try { await applyMode(side) } catch (error) { status.textContent = error.message }
}

function clearDiff() {
  for (const side of sides) {
    capture(side)
    panes[side].generation += 1
    reader(side)?.source.attach(null)
    panes[side].diff?.dispose()
    if (panes[side].diff?.viewport.ownerDocument !== document) panes[side].diff?.viewport.remove()
    panes[side].diff = null
  }
  metadata = null
  comparisonId = null
  change = -1
  navigation += 1
  controls()
}

async function refresh() {
  const state = await api('/api/panes')
  const changed = sides.some(side => state[side]?.id !== panes[side].file?.id)
  if (changed) {
    clearDiff()
    generation += 1
    for (const side of sides) {
      const pane = panes[side]
      const next = state[side]
      const newFile = next?.id !== pane.file?.id
      pane.file = next
      document.getElementById(`${side}-file`).textContent = next?.path || 'Choose a Markdown file'
      document.getElementById(`${side}-file`).title = next?.path || ''
      document.getElementById(`${side}-empty`).hidden = Boolean(next)
      if (newFile) {
        pane.tools.clear()
        pane.position = next ? readPosition(token, next.id) : { line: 1, view: 'preview' }
        pane.mode = pane.position.view
        if (next) document.getElementById(`${side}-preview`).src = new URL(`./pane/${next.id}/${side}/?embedded=1&comparison=1&line=${pane.position.line}#${token}`, import.meta.url).href
      }
      void applyMode(side).catch(error => { status.textContent = error.message })
    }
  }
  if (!sides.every(side => state[side]?.ready || state[side]?.error)) {
    status.textContent = sides.some(side => state[side]?.error) ? 'A document could not be indexed; reopen it to retry.' : 'Read either document now. Open another file to compare · read only'
    return
  }
  if (metadata) return
  const ticket = generation
  if (!comparisonId) {
    if (state.comparisonId) comparisonId = state.comparisonId
    else {
      const result = await api('/api/compare', { method: 'POST', body: JSON.stringify({ left: state.left.path, right: state.right.path, readers: [state.left.id, state.right.id] }) })
      if (ticket !== generation || stopped) return
      comparisonId = result.comparisonId
    }
  }
  const result = await api('/api/compare')
  if (ticket !== generation || stopped) return
  if (result.state === 'error') throw new Error(result.error)
  if (result.state !== 'ready') { status.textContent = 'Computing exact source differences… Both documents remain readable.'; return }
  metadata = result
  const url = new URL(location.href)
  url.search = `?comparison=${comparisonId}`
  history.replaceState(null, '', url)
  for (const side of sides) {
    capture(side)
    await applyMode(side)
  }
  controls()
}

async function navigate(index) {
  if (!metadata?.changes || !sides.every(side => panes[side].mode === 'source')) return false
  const ticket = ++navigation
  try {
    const target = (index + metadata.changes) % metadata.changes
    const result = await api(`/api/compare/hunk?index=${target}`)
    if (ticket !== navigation || stopped) return false
    change = target
    for (const side of sides) {
      panes[side].generation += 1
      panes[side].diff.show(result.row, result[`${side}Line`], true)
    }
    controls()
    return true
  } catch (error) { status.textContent = error.message; return false }
}

async function loadLine(offset, previous = false) {
  detailRequest?.abort()
  detailRequest = new AbortController()
  const ticket = ++detailGeneration
  document.querySelector('#line-text').textContent = 'Loading source…'
  for (const id of ['line-previous', 'line-next']) document.getElementById(id).disabled = true
  try {
    const result = await api(`/api/compare/line?row=${detailRow.row}&side=${detailRow.side}&offset=${offset}&previous=${previous}`, { signal: detailRequest.signal })
    if (ticket !== detailGeneration || !detail.open) return
    detailPage = result.result
    document.querySelector('#line-title').textContent = `${metadata[detailRow.side]} · line ${detailPage.line}`
    document.querySelector('#line-text').textContent = detailPage.text
    document.querySelector('#line-position').textContent = `Bytes ${detailPage.offset.toLocaleString()}–${detailPage.end.toLocaleString()} / ${detailPage.total.toLocaleString()} · exact source · read only`
    document.querySelector('#line-previous').disabled = detailPage.offset === 0
    document.querySelector('#line-next').disabled = detailPage.end >= detailPage.total
  } catch (error) { if (error.name !== 'AbortError') document.querySelector('#line-text').textContent = error.message }
}

function openLine(row, side) { detailRow = { row, side }; detailPage = null; detail.showModal(); void loadLine(0) }
document.querySelector('#line-close').addEventListener('click', () => detail.close())
detail.addEventListener('close', () => { detailGeneration += 1; detailRequest?.abort() })
document.querySelector('#line-previous').addEventListener('click', () => { if (detailPage?.offset) void loadLine(detailPage.offset, true) })
document.querySelector('#line-next').addEventListener('click', () => { if (detailPage && detailPage.end < detailPage.total) void loadLine(detailPage.end) })
document.querySelector('#change-previous').addEventListener('click', () => navigate(change < 0 ? metadata.changes - 1 : change - 1))
document.querySelector('#change-next').addEventListener('click', () => navigate(change + 1))
for (const side of sides) {
  for (const mode of ['preview', 'source']) document.getElementById(`${side}-mode-${mode}`).addEventListener('click', () => setMode(side, mode))
  document.getElementById(`${side}-preview`).addEventListener('load', () => { void applyMode(side).catch(error => { status.textContent = error.message }) })
  document.getElementById(`choose-${side}`).addEventListener('click', () => files.choose(async path => {
    if (opening) return
    opening = true
    for (const button of document.querySelectorAll('#choose-left,#choose-right,#compare-close')) button.disabled = true
    try {
      for (const name of sides) capture(name)
      await api('/api/panes', { method: 'POST', body: JSON.stringify({ side, path }) })
      await refresh()
    } finally {
      opening = false
      for (const button of document.querySelectorAll('#choose-left,#choose-right,#compare-close')) button.disabled = false
    }
  }, `Choose ${side} Markdown file`, panes[side].file?.path))
}
addEventListener('message', event => {
  if (event.origin !== location.origin || event.data?.type !== 'marknoria:ready') return
  for (const side of sides) {
    if (event.source === document.getElementById(`${side}-preview`).contentWindow && event.data.documentId === panes[side].file?.id) {
      void applyMode(side).catch(error => { status.textContent = error.message })
    }
  }
})
document.querySelector('#compare-close').addEventListener('click', async () => {
  try {
    for (const side of sides) capture(side)
    await api('/api/panes', { method: 'DELETE' })
    const target = new URL('./', import.meta.url)
    target.hash = token
    location.assign(target.href)
  } catch (error) { status.textContent = error.message }
})
addEventListener('pagehide', () => {
  stopped = true
  for (const side of sides) { capture(side); panes[side].diff?.dispose(); panes[side].tools.clear() }
  detailRequest?.abort()
  files.dispose()
}, { once: true })
window.comparisonReader = { panes, navigate, setMode, get metadata() { return metadata } }
async function poll() {
  while (!stopped && token) {
    try { if (!opening) await refresh() } catch (error) { status.textContent = error.message }
    await new Promise(resolve => setTimeout(resolve, metadata ? 1000 : 250))
  }
}
void poll()
