import assert from 'node:assert/strict'
import { readFileSync, readdirSync } from 'node:fs'
import test from 'node:test'

import {
  annotationPromptDefaults,
  getLocale,
  normalizeLocalizedTemplates,
  resolveLanguage,
  setLanguage,
  t,
  translateDocument,
  translationEntries,
} from './i18n.mjs'

test('default comment templates do not add blank lines around placeholders', () => {
  for (const template of Object.values(annotationPromptDefaults)) {
    assert.doesNotMatch(template, /\n\n\{\{(?:annotations|additional)\}\}/u)
  }
})

test('resolves explicit and system languages', () => {
  assert.equal(resolveLanguage('zh-CN', 'en-US'), 'zh-CN')
  assert.equal(resolveLanguage('en-US', 'zh-CN'), 'en-US')
  assert.equal(resolveLanguage('system', 'zh-HK'), 'zh-CN')
  assert.equal(resolveLanguage('system', 'de-DE'), 'en-US')
})

test('keeps Chinese continuation and steering actions concise', () => {
  setLanguage('zh-CN')
  assert.equal(t('Continue'), '\u7ee7\u7eed')
  assert.equal(t('Steer'), '\u8ffd\u52a0')
  assert.equal(t('Resume'), '\u6062\u590d')
  setLanguage('en-US')
})

test('translates interface text and interpolates values', () => {
  setLanguage('en-US')
  assert.equal(getLocale(), 'en-US')
  assert.equal(t('Settings'), 'Settings')
  assert.equal(t('Active'), 'Active')
  assert.equal(t('Run'), 'Run')
  assert.equal(t('Found {count} matching favorites', { count: 3 }), 'Found 3 matching favorites')
  assert.equal(t('untranslated user text'), 'untranslated user text')
  setLanguage('zh-CN')
  assert.equal(t('Settings'), '\u8BBE\u7F6E')
  assert.equal(t('Found {count} matching favorites', { count: 3 }), '\u627E\u5230 3 \u6761\u5339\u914D\u6536\u85CF')
  setLanguage('en-US')
  assert.equal(t('\u8BBE\u7F6E'), 'Settings')
})

test('contains the Chinese catalog keyed by English source messages', () => {
  assert.ok(Object.keys(translationEntries['zh-CN']).length > 100)
  assert.equal(translationEntries['zh-CN'].Settings, '\u8BBE\u7F6E')
  const localizedMessages = Object.values(translationEntries['zh-CN'])
  assert.equal(new Set(localizedMessages).size, localizedMessages.length)
})

test('normalizes only valid localized comment templates', () => {
  const chinese = '\u8BF7\u6839\u636E\u8FD9\u4E9B\u5185\u5BB9\u56DE\u590D\uFF1A\n{{annotations}}'
  const english = 'Please respond to these notes:\n{{annotations}}'
  assert.deepEqual(normalizeLocalizedTemplates({
    'zh-CN': chinese,
    'en-US': english,
    'ja-JP': 'ignored {{annotations}}',
  }), { 'zh-CN': chinese, 'en-US': english })
  assert.deepEqual(normalizeLocalizedTemplates({ 'zh-CN': 'missing placeholder' }), {})
})

test('translates an English source subtree in both directions without changing protected content', () => {
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
  const label = new Text('Settings')
  const protectedText = new Text('Settings')
  const root = new Element([
    label,
    new Element([protectedText], {}, true),
  ], { title: 'Open settings' })

  setLanguage('zh-CN')
  translateDocument(root)
  assert.equal(label.nodeValue, '\u8BBE\u7F6E')
  assert.equal(root.getAttribute('title'), '\u6253\u5F00\u8BBE\u7F6E')
  assert.equal(protectedText.nodeValue, 'Settings')

  setLanguage('en-US')
  translateDocument(root)
  assert.equal(label.nodeValue, 'Settings')
  assert.equal(root.getAttribute('title'), 'Open settings')
})

test('runtime web source keeps Chinese text exclusively in i18n.mjs', () => {
  const runtimeFiles = readdirSync(new URL('.', import.meta.url), { withFileTypes: true })
    .filter((entry) => entry.isFile()
      && /\.(?:js|mjs|html)$/u.test(entry.name)
      && entry.name !== 'i18n.mjs'
      && !entry.name.startsWith('vendor'))
    .map((entry) => entry.name)
  const violations = runtimeFiles.filter((name) => /\p{Script=Han}/u.test(
    readFileSync(new URL(name, import.meta.url), 'utf8'),
  ))
  assert.deepEqual(violations, [])
})

test('native and prototype runtime source contains no embedded Chinese UI text', () => {
  const sourceRoots = [
    new URL('../src-tauri/src/', import.meta.url),
    new URL('../prototypes/', import.meta.url),
  ]
  const files = []
  const visit = (directory) => {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      const target = new URL(entry.name + (entry.isDirectory() ? '/' : ''), directory)
      if (entry.isDirectory()) visit(target)
      else if (/\.(?:rs|html|js|mjs)$/u.test(entry.name)) files.push(target)
    }
  }
  sourceRoots.forEach(visit)
  const violations = files.filter((file) => /\p{Script=Han}/u.test(readFileSync(file, 'utf8')))
    .map((file) => file.pathname)
  assert.deepEqual(violations, [])
})

test('every native browser translation source exists in the Chinese catalog', () => {
  const source = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  const block = source.match(/const embeddedBrowserTranslationSources = \[([\s\S]*?)\n\]/u)?.[1] || ''
  const messages = [...block.matchAll(/'([^']+)'/gu)].map((match) => match[1])
  assert.ok(messages.length > 20)
  assert.deepEqual(messages.filter((message) => !translationEntries['zh-CN'][message]), [])
})

test('saving settings closes the dialog before rerendering dynamic UI', () => {
  const source = readFileSync(new URL('./settings-application.mjs', import.meta.url), 'utf8')
  const body = source.match(/function saveSettings\(event\) \{([\s\S]*?)\n\}/)?.[1] || ''
  assert.ok(body.indexOf("$('#settings-dialog').close()") >= 0)
  assert.ok(body.indexOf("$('#settings-dialog').close()") < body.indexOf('renderLocalizedUI()'))
})

test('comments and favorites use compact right-area launchers without add menus', () => {
  const html = readFileSync(new URL('./index.html', import.meta.url), 'utf8')
  const source = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  const styles = readFileSync(new URL('./styles.css', import.meta.url), 'utf8')
  assert.match(html, /id="open-thread-comments"[^>]*workspace-tool-launcher[^>]*icon-only/)
  assert.match(html, /id="open-thread-favorites"[^>]*workspace-tool-launcher[^>]*icon-only/)
  assert.match(html, /id="thread-comments-count"[^>]*workspace-tool-count[^>]*hidden/)
  assert.match(html, /id="thread-favorites-count"[^>]*workspace-tool-count[^>]*hidden/)
  assert.match(html, /id="thread-resources-count"[^>]*workspace-tool-count[^>]*hidden/)
  assert.match(styles, /\.workspace-tool-count \{[^}]*top: -4px;[^}]*right: -4px;[^}]*font-size: var\(--ui-font-xxs\);[^}]*font-variant-numeric: tabular-nums;/u)
  assert.doesNotMatch(styles, /\.workspace-tool-count,[^\n]*font-size: var\(--ui-font-xxs\)/u)
  assert.match(styles, /\.annotation-rail h2, \.resources-heading h2, \.favorites-rail h2 \{[^}]*display: flex;[^}]*align-items: center;[^}]*gap: 6px;/u)
  assert.match(styles, /\.annotation-count \{[^}]*height: max\(19px, calc\(var\(--ui-font-xxs\) \+ 4px\)\);[^}]*align-items: center;[^}]*justify-content: center;[^}]*font-variant-numeric: tabular-nums;/u)
  assert.doesNotMatch(html, /id="(?:annotation|favorite)-menu-button"/)
  assert.doesNotMatch(html, /id="(?:open-annotation-rail|open-session-favorites)"/)
  assert.doesNotMatch(source, /(?:annotation|favorite)-menu-button/)
  assert.doesNotMatch(source, /button\.querySelector\('span'\)\.textContent = favorite/)
  assert.match(html, /id="selection-comment"/)
  assert.match(html, /id="selection-favorite"/)
})
