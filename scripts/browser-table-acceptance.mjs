import { mkdir } from 'node:fs/promises'
import { join } from 'node:path'

export async function checkTableAcceptance(page) {
  await page.evaluate(async () => {
    const { renderTableArtifact } = await import('/table-reader.mjs')
    const host = document.createElement('div')
    host.id = 'table-acceptance'
    host.style.cssText = 'position:fixed;inset:20px;z-index:10000;background:var(--panel)'
    document.body.append(host)
    const text = '<img src=x onerror=alert(1)> 中文 long text '.repeat(100) + '\nLast line'
    let selections = 0
    window.tableAcceptance = { text, get selections() { return selections }, reader: renderTableArtifact({
      container: host, workbook: { sheets: [{ name: 'Test', rows: [[text, 'short']] }] },
      onSelection: () => { selections++ },
    }) }
  })
  const cell = page.locator('#table-acceptance td').first()
  await cell.dblclick()
  const dialog = page.locator('.table-cell-dialog')
  await dialog.waitFor({ state: 'visible' })
  const result = await page.evaluate(() => {
    const content = document.querySelector('.table-cell-content')
    return {
      exact: content.textContent === window.tableAcceptance.text,
      safe: !content.querySelector('img'),
      scroll: content.scrollHeight > content.clientHeight,
      wraps: getComputedStyle(content).whiteSpace === 'pre-wrap',
      selection: window.tableAcceptance.selections > 0,
    }
  })
  if (Object.values(result).some(value => !value)) throw Error(`Cell viewer failure: ${JSON.stringify(result)}`)
  await page.evaluate(() => {
    window.tableAcceptance.originalCopy = navigator.clipboard.writeText
    navigator.clipboard.writeText = async value => { window.tableAcceptance.copied = value }
  })
  try {
    await dialog.locator('[data-cell-copy]').click()
    if (!await page.evaluate(() => window.tableAcceptance.copied === window.tableAcceptance.text)) throw Error('Cell copy was truncated')
  } finally {
    await page.evaluate(() => { navigator.clipboard.writeText = window.tableAcceptance.originalCopy })
  }
  if (process.env.STUDIO_SCREENSHOT_DIR) {
    await mkdir(process.env.STUDIO_SCREENSHOT_DIR, { recursive: true })
    await dialog.screenshot({ path: join(process.env.STUDIO_SCREENSHOT_DIR, 'table-cell-content.png') })
  }
  await page.keyboard.press('Escape')
  await dialog.waitFor({ state: 'detached' })
  await cell.focus()
  await page.keyboard.press('Enter')
  await dialog.waitFor({ state: 'visible' })
  await dialog.locator('[data-cell-close]').click()
  await cell.dblclick()
  await page.evaluate(() => { window.tableAcceptance.reader.destroy(); document.querySelector('#table-acceptance').remove(); delete window.tableAcceptance })
  if (await dialog.count()) throw Error('Cell dialog survived reader disposal')
  return 'PASS: long table cell full text, safe rendering, wrapping, scrolling, double-click, Enter, Escape, close and reader disposal'
}
