import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const source = readFileSync(new URL('./embedded-browser.html', import.meta.url), 'utf8')
const panelSource = readFileSync(new URL('./embedded-browser-panel.html', import.meta.url), 'utf8')
const windowsSource = readFileSync(new URL('../src-tauri/src/embedded_browser_windows.rs', import.meta.url), 'utf8')

test('embedded browser tab strip follows normal browser control order', () => {
  const tabs = source.indexOf('id="tabs"')
  const newTab = source.indexOf('class="new-tab"')
  const closeBrowser = source.indexOf('class="workspace-close"')

  assert.ok(tabs >= 0)
  assert.ok(newTab > tabs, 'new-tab belongs immediately after the tab list')
  assert.ok(closeBrowser > newTab, 'workspace close remains at the far right')
  assert.match(source.slice(newTab, closeBrowser), /data-action="new-tab"/u)
  assert.match(source.slice(closeBrowser), /data-action="toggle-browser"/u)
  assert.match(source, /\.tabs \{[^}]*flex: 0 1 auto;/u)
  assert.match(source, /\.workspace-close \{[^}]*margin: 0 0 1px auto;/u)
  assert.match(source, /grid-template-rows: 32px 43px/u)
  assert.match(source, /\.tabs-row \{[^}]*padding: 4px 10px 0;/u)
  assert.match(source, /\.toolbar \{[^}]*padding: 0 10px;/u)
})

test('browser menu is the rightmost address-toolbar action', () => {
  const tools = source.match(/<div class="tools">([\s\S]*?)<\/div>/u)?.[1] || ''
  const comment = tools.indexOf('id="comment"')
  const browserMenu = tools.indexOf('data-action="browser-menu"')

  assert.ok(comment >= 0)
  assert.ok(browserMenu > comment)
  assert.equal(browserMenu, tools.lastIndexOf('data-action='))
  assert.match(tools, /aria-label="更多浏览器操作"/u)
})

test('Windows native browser menu follows the Linux GTK menu protocol', () => {
  assert.match(windowsSource, /const BROWSER_MENU_WIDTH: i32 = 292;/u)
  assert.match(windowsSource, /WS_EX_TOOLWINDOW \| WS_EX_NOACTIVATE/u)
  assert.match(windowsSource, /WS_POPUP/u)
  const paintStart = windowsSource.indexOf('fn paint_native_browser_menu')
  const paintEnd = windowsSource.indexOf('unsafe fn draw_native_menu_item', paintStart)
  const paintSource = windowsSource.slice(paintStart, paintEnd)
  const labels = [
    '新建标签页', '重新加载', '复制当前链接', 'draw_native_zoom_row', '适应页面宽度',
    '批注选中内容', '浏览器信息', '下载内容', '退出浏览器',
  ]
  let previous = -1
  for (const label of labels) {
    const needle = label === 'draw_native_zoom_row' ? label : `"${label}"`
    const index = paintSource.indexOf(needle, previous + 1)
    assert.ok(index > previous, `${label} follows the Linux menu order`)
    previous = index
  }
  assert.match(windowsSource, /"新建标签页",\s*Some\("Ctrl\+T"\)/u)
  assert.match(windowsSource, /"重新加载",\s*Some\("Ctrl\+R"\)/u)
  assert.match(windowsSource, /"浏览器信息",\s*Some\("›"\)/u)
  assert.match(windowsSource, /"下载内容",\s*Some\("›"\)/u)
  assert.match(windowsSource, /draw_native_square[\s\S]*"−"[\s\S]*zoom_percent[\s\S]*"\+"/u)
  assert.doesNotMatch(windowsSource, /关闭菜单|close-browser-menu/u)
  assert.doesNotMatch(panelSource, /关闭菜单|close-browser-menu|overflow:\s*auto/u)
  assert.match(source, /window\.__embeddedBrowserToolbar/u)
  assert.match(panelSource, /type: 'browser-panel-action'/u)
  assert.match(panelSource, /type: 'browser-panel-size'/u)
  assert.match(panelSource, /activeView\.scrollHeight/u)
  assert.match(panelSource, /window\.__embeddedBrowserPanel/u)
})
