// Shared synthetic-document presentation contract, run in Chromium and real Wry.
export async function run() {
  const root = document.querySelector('#space')
  const deadline = performance.now() + 30000
  while (!root?.querySelector('h2') || !root.querySelector('table tbody tr') || !root.querySelector('pre code')) {
    if (performance.now() > deadline) throw Error('Markdown corpus did not become readable')
    await new Promise(resolve => setTimeout(resolve, 20))
  }
  let assertions = 0
  const check = (value, message) => { if (!value) throw Error(message); assertions++ }
  check(root.querySelector('h1').textContent === 'Public 中文😀 corpus', 'BOM/Unicode heading changed')
  check(root.querySelector('strong').textContent === 'bold', 'Strong text missing')
  check(root.querySelector('em').textContent === 'emphasis', 'Emphasis missing')
  check(root.querySelector('del').textContent === 'deleted', 'Strike-through missing')
  const tasks = [...root.querySelectorAll('li input[type="checkbox"]')]
  check(tasks.length === 2 && tasks.every(input => input.disabled) && tasks[0].checked && !tasks[1].checked, 'GFM tasks changed')
  const links = [...root.querySelectorAll('a')].map(link => link.getAttribute('href'))
  for (const href of ['https://example.org/a_(b)', 'http://www.example.org/path', 'mailto:me@example.org', 'https://target.example.org', 'https://reference.example.org', 'https://table.example.org']) check(links.includes(href), 'Missing safe link ' + href)
  check([...root.querySelectorAll('a')].every(link => link.rel.includes('noopener') && link.rel.includes('noreferrer')), 'Link opener authority expanded')
  check(!root.querySelector('pre a') && root.querySelector('pre').textContent.includes('www.code.example.org'), 'Code became an autolink')
  check([...root.querySelectorAll('span')].some(node=>node.textContent==='Literal HTML policy')&&!root.textContent.includes('<span onclick='), 'Safe HTML span was not rendered or unsafe attributes survived')
  check(root.querySelector('kbd')?.textContent==='Ctrl'&&[...root.querySelectorAll('strong')].some(node=>node.textContent==='HTML bold'),'Safe inline HTML boundaries were auto-closed')
  check(root.querySelector('details[open] summary')?.textContent==='HTML details'&&root.querySelector('details p')?.textContent==='Safe HTML 中文😀.','Safe block HTML was not rendered')
  const rawLink=[...root.querySelectorAll('a')].find(node=>node.textContent==='Safe raw link')
  check(rawLink?.href==='https://raw.example.org/'&&!rawLink.id&&!rawLink.hasAttribute('data-source-line'),'Raw HTML forged host/source identity or lost its safe URL')
  check(!root.querySelector('script,[onclick]') && !globalThis.compromised, 'Document script gained authority')
  check(root.querySelector('p br') !== null, 'Hard line breaks changed')
  const row = root.querySelector('table tbody tr')
  check(row.cells[0].textContent === '中文|😀' && row.cells[1].textContent === '001', 'Escaped pipe or exact text changed')
  check(Number(row.dataset.sourceLine) === 17 && Number(row.dataset.sourceEnd) > Number(row.dataset.sourceStart), 'Original table source anchor changed')
  check(root.querySelector('h2').textContent === 'Final section' && root.textContent.includes('EOF sentinel 中文😀.'), 'Final source content missing')
  return { assertions, gfm: true, bomCrLfUnicode: true, sourceAnchors: true, safeHtml: true, codeLiterals: true, navigationToUsefulMs: performance.now() }
}
