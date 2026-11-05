import { mkdir } from 'node:fs/promises'
import { join } from 'node:path'

export async function checkTableAcceptance(page) {
  await page.evaluate(async () => {
    const { mountTableCells } = await import('/reader-assets/table-cells.mjs')
    const host = document.createElement('div')
    host.id = 'table-acceptance'
    host.style.cssText = 'position:fixed;inset:20px;z-index:10000;background:var(--panel)'
    document.body.append(host)
    const text = '<img src=x onerror=alert(1)> 中文 long text '.repeat(100) + '\nLast line'
    let selections = 0
    const table = document.createElement('table'), body = table.createTBody(), row = body.insertRow()
    const cell = row.insertCell(); cell.textContent = text; row.insertCell().textContent = 'short'; host.append(table)
    const cells = mountTableCells(host, { locate: () => ({kind:'cell',table:0,row:0,column:0}), onSelection: value => { if (value) selections++ } })
    cells.update(new Map([[cell, text]]))
    window.tableAcceptance = { text, get selections() { return selections }, reader: { destroy: () => cells.dispose() } }

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
  if (await dialog.locator('[data-cell-copy], footer').count()) throw Error('Redundant cell copy footer')
  if (process.env.STUDIO_SCREENSHOT_DIR) {
    await mkdir(process.env.STUDIO_SCREENSHOT_DIR, { recursive: true })
    await dialog.screenshot({ path: join(process.env.STUDIO_SCREENSHOT_DIR, 'table-cell-content.png') })
  }
  await page.keyboard.press('Escape')
  await dialog.waitFor({ state: 'detached' })
  await cell.focus()
  await page.keyboard.press('Enter')
  await dialog.waitFor({ state: 'visible' })
  await dialog.getByRole('button',{name:'Close'}).click()
  await cell.dblclick()
  await page.evaluate(() => { window.tableAcceptance.reader.destroy(); document.querySelector('#table-acceptance').remove(); delete window.tableAcceptance })
  if (await dialog.count()) throw Error('Cell dialog survived reader disposal')
  return 'PASS: long table cell full text, safe rendering, wrapping, scrolling, double-click, Enter, Escape, close and reader disposal'
}
