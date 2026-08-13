import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  getLocale,
  migrateLocalizedTemplates,
  resolveLanguage,
  setLanguage,
  t,
  translateDocument,
  translationEntries,
} from './i18n.mjs'

test('resolves explicit and system languages', () => {
  assert.equal(resolveLanguage('zh-CN', 'en-US'), 'zh-CN')
  assert.equal(resolveLanguage('en-US', 'zh-CN'), 'en-US')
  assert.equal(resolveLanguage('system', 'zh-HK'), 'zh-CN')
  assert.equal(resolveLanguage('system', 'de-DE'), 'en-US')
})

test('translates interface text and interpolates values', () => {
  setLanguage('en-US')
  assert.equal(getLocale(), 'en-US')
  assert.equal(t('设置'), 'Settings')
  assert.equal(t('运行'), 'Active')
  assert.equal(t('运行命令'), 'Run')
  assert.equal(t('找到 {count} 条匹配收藏', { count: 3 }), 'Found 3 matching favorites')
  assert.equal(t('untranslated user text'), 'untranslated user text')
  setLanguage('zh-CN')
  assert.equal(t('Settings'), '设置')
})

test('contains both required interface languages', () => {
  assert.ok(Object.keys(translationEntries['en-US']).length > 100)
})

test('migrates a legacy template into its actual language slot', () => {
  const chinese = '请根据这些内容回复：\n{{annotations}}'
  const migrated = migrateLocalizedTemplates({ 'en-US': chinese }, chinese, 'en-US')
  assert.deepEqual(migrated.templates, { 'zh-CN': chinese })
  assert.equal(migrated.legacyLocale, 'zh-CN')
  assert.equal(migrated.migratedLegacy, true)

  const english = 'Please respond to these notes:\n{{annotations}}'
  const preserved = migrateLocalizedTemplates(
    { 'zh-CN': chinese, 'en-US': english },
    english,
    'en-US',
  )
  assert.deepEqual(preserved.templates, { 'zh-CN': chinese, 'en-US': english })
  assert.equal(preserved.migratedLegacy, false)
})

test('translates a rendered subtree in both directions without changing protected content', () => {
  globalThis.Node = { TEXT_NODE: 3, ELEMENT_NODE: 1 }
  class Element {
    constructor(children = [], attributes = {}, protectedContent = false) {
      this.nodeType = Node.ELEMENT_NODE
      this.childNodes = children
      this.attributes = new Map(Object.entries(attributes))
      this.protectedContent = protectedContent
      for (const child of children) child.parentElement = this
    }
    matches() { return this.protectedContent }
    closest() { return this.protectedContent || this.parentElement?.closest() ? this : null }
    hasAttribute(name) { return this.attributes.has(name) }
    getAttribute(name) { return this.attributes.get(name) }
    setAttribute(name, value) { this.attributes.set(name, value) }
  }
  class Text {
    constructor(value) { this.nodeType = Node.TEXT_NODE; this.nodeValue = value }
  }
  const label = new Text('设置')
  const protectedText = new Text('设置')
  const root = new Element([
    label,
    new Element([protectedText], {}, true),
  ], { title: '打开设置' })

  setLanguage('en-US')
  translateDocument(root)
  assert.equal(label.nodeValue, 'Settings')
  assert.equal(root.getAttribute('title'), 'Open settings')
  assert.equal(protectedText.nodeValue, '设置')

  setLanguage('zh-CN')
  translateDocument(root)
  assert.equal(label.nodeValue, '设置')
  assert.equal(root.getAttribute('title'), '打开设置')
})

test('every static Chinese interface string has an English translation', () => {
  const html = readFileSync(new URL('./index.html', import.meta.url), 'utf8')
  const values = [
    ...[...html.matchAll(/>([^<>]+)</g)].map((match) => match[1].trim()),
    ...[...html.matchAll(/(?:placeholder|title|aria-label)="([^"]+)"/g)].map((match) => match[1].trim()),
  ].filter((value) => /\p{Script=Han}/u.test(value))
  setLanguage('en-US')
  const missing = [...new Set(values.filter((value) => t(value) === value))]
  assert.deepEqual(missing, [])
})

test('every exact Chinese JavaScript interface literal has an English translation', () => {
  const source = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  const values = [
    ...[...source.matchAll(/'([^'\n]*\p{Script=Han}[^'\n]*)'/gu)].map((match) => match[1]),
    ...[...source.matchAll(/"([^"\n]*\p{Script=Han}[^"\n]*)"/gu)].map((match) => match[1]),
  ].map((value) => value.replaceAll('\\n', '\n'))
    .filter((value) => !value.includes('${') && !value.includes('<'))
  setLanguage('en-US')
  const missing = [...new Set(values.filter((value) => t(value) === value))]
  assert.deepEqual(missing, [])
})

test('saving settings closes the dialog before rerendering dynamic UI', () => {
  const source = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  const body = source.match(/function saveSettings\(event\) \{([\s\S]*?)\n\}/)?.[1] || ''
  assert.ok(body.indexOf("$('#settings-dialog').close()") >= 0)
  assert.ok(body.indexOf("$('#settings-dialog').close()") < body.indexOf('renderLocalizedUI()'))
})

test('comments and favorites use compact right-area launchers without add menus', () => {
  const html = readFileSync(new URL('./index.html', import.meta.url), 'utf8')
  const source = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  assert.match(html, /id="open-thread-comments"[^>]*workspace-tool-launcher[^>]*icon-only/)
  assert.match(html, /id="open-thread-favorites"[^>]*workspace-tool-launcher[^>]*icon-only/)
  assert.doesNotMatch(html, /id="(?:annotation|favorite)-menu-button"/)
  assert.doesNotMatch(html, /id="(?:open-annotation-rail|open-session-favorites)"/)
  assert.doesNotMatch(source, /(?:annotation|favorite)-menu-button/)
  assert.match(html, /id="selection-comment"/)
  assert.match(html, /id="selection-favorite"/)
})
