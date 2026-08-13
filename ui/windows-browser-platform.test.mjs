import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const manifest = readFileSync(new URL('../src-tauri/Cargo.toml', import.meta.url), 'utf8')
const workspaceManifest = readFileSync(new URL('../Cargo.toml', import.meta.url), 'utf8')
const mainSource = readFileSync(new URL('../src-tauri/src/main.rs', import.meta.url), 'utf8')
const windowsSource = readFileSync(new URL('../src-tauri/src/embedded_browser_windows.rs', import.meta.url), 'utf8')
const toolbar = readFileSync(new URL('./embedded-browser.html', import.meta.url), 'utf8')

test('Windows browser dependencies do not pull Linux GTK or WebKitGTK directly', () => {
  const linux = manifest.match(/\[target\.'cfg\(target_os = "linux"\)'\.dependencies\]([\s\S]*?)(?=\n\[|$)/u)?.[1] || ''
  const windows = manifest.match(/\[target\.'cfg\(windows\)'\.dependencies\]([\s\S]*?)(?=\n\[|$)/u)?.[1] || ''

  assert.match(linux, /^gtk\s*=/mu)
  assert.match(linux, /^webkit2gtk\s*=/mu)
  assert.match(windows, /^wry\s*=/mu)
  assert.match(windows, /^webview2-com\s*=/mu)
  assert.doesNotMatch(windows, /^(gtk|webkit2gtk)\s*=/mu)
})

test('Windows browser stays behind a compile-time boundary and uses published WRY', () => {
  assert.doesNotMatch(workspaceManifest, /\[patch\.crates-io\]/u)
  assert.doesNotMatch(workspaceManifest, /vendor\/wry/u)
  assert.match(mainSource, /#\[cfg\(windows\)\]\s*mod embedded_browser_windows;/u)
  assert.match(windowsSource, /^#!\[cfg\(windows\)\]/u)
  assert.match(windowsSource, /queue_browser_action\(window, move \|\|/u)
})

test('trusted toolbar exposes exit, information and downloads actions without remote-page controls', () => {
  assert.match(toolbar, /make\('退出浏览器', 'exit-browser'\)/u)
  assert.match(toolbar, /make\('浏览器信息', 'browser-info'\)/u)
  assert.match(toolbar, /make\('下载记录', 'browser-downloads'\)/u)
  assert.match(toolbar, /action\('open-downloads-directory'\)/u)
})
