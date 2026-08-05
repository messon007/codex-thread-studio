import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const html = readFileSync(new URL('./index.html', import.meta.url), 'utf8')
const styles = readFileSync(new URL('./styles.css', import.meta.url), 'utf8')

function rule(selector) {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  return styles.match(new RegExp(`${escaped}\\s*\\{([^}]+)\\}`))?.[1] || ''
}

test('brand geometry matches the Agent Deck Studio sidebar contract', () => {
  assert.match(html, /<strong>Codex Thread Studio<\/strong>\s*<span>Codex Desktop<\/span>/)
  assert.match(rule('.brand-row'), /height:\s*74px/)
  assert.match(rule('.brand-row'), /padding:\s*16px 18px/)
  assert.match(rule('.brand-mark'), /width:\s*38px/)
  assert.match(rule('.brand-mark'), /height:\s*38px/)
  assert.match(rule('.brand-mark'), /border-radius:\s*11px/)
  assert.match(rule('.brand-copy strong'), /font-size:\s*16px/)
  assert.match(rule('.brand-copy strong'), /white-space:\s*nowrap/)
  assert.match(rule('.brand-copy span'), /font-size:\s*11px/)
  assert.match(rule('.brand-copy span'), /margin-top:\s*3px/)
})

test('application actions live in an extensible footer menu instead of the brand row', () => {
  assert.doesNotMatch(html, /class="brand-actions"/)
  assert.match(html, /id="studio-menu-button"/)
  assert.match(html, /id="open-favorites"/)
  assert.match(html, /id="connections-button"/)
  assert.match(html, /id="settings-button"/)
  assert.match(html, /id="about-button"/)
  assert.match(html, /id="connections-dialog"/)
  assert.match(html, /id="backend-dialog"[^>]*class="dialog about-dialog"/)
  assert.match(rule('.action-menu.studio-menu'), /top:\s*auto/)
  assert.match(rule('.action-menu.studio-menu'), /bottom:\s*calc\(100% \+ 7px\)/)
})

test('filters use the same flat label-and-number structure as Agent Deck Studio', () => {
  assert.match(html, /class="filters thread-filters"/)
  assert.match(html, />全部 <span id="count-all">0<\/span><\/button>/)
  assert.match(html, />运行 <span id="count-active">0<\/span><\/button>/)
  assert.match(html, />待处理 <span id="count-attention">0<\/span><\/button>/)
  assert.match(rule('.filters'), /display:\s*flex/)
  assert.match(rule('.filters'), /padding:\s*0 16px 10px/)
  assert.match(rule('.filters'), /border-bottom:\s*1px solid var\(--border\)/)
  assert.match(rule('.filter'), /font-size:\s*12px/)
  assert.match(rule('.filter span'), /font-variant-numeric:\s*tabular-nums/)
  assert.match(rule('.filter.active'), /background:\s*var\(--brand-soft\)/)
})
