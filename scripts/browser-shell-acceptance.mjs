export async function checkShellAcceptance(page) {
  return page.evaluate(async () => {
    const app = await (await fetch('/app.js')).text()
    const start = app.indexOf('function renderItem(')
    const end = app.indexOf('\nfunction ', start + 1)
    if (start < 0 || end < 0) throw Error('Missing item renderer')
    const escapeHtml = value => String(value).replace(/[&<>"']/g, ch => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[ch]))
    const render = new Function('escapeHtml', 't', 'statusLabel', `${app.slice(start, end)}; return renderItem`)(escapeHtml, value => value, value => value)
    const host = document.createElement('div')
    const output = '<script>bad()</script>\n' + 'branch\n'.repeat(30)
    host.innerHTML = render({ type: 'commandExecution', source: 'userShell', id: 'manual', status: 'completed', command: 'git branch -vv', aggregatedOutput: output }, 'turn')
    if (host.querySelector('details') || host.querySelector('script') || host.querySelector('pre')?.textContent !== output) throw Error('Manual command rendering collapsed, unsafe or truncated')
    host.innerHTML = render({ type: 'commandExecution', source: 'userShell', id: 'manual', status: 'inProgress', command: 'ls' }, 'turn')
    if (!host.querySelector('pre')) throw Error('Missing streaming output target')
    return 'PASS: manual shell command output renders directly, untruncated and escaped; empty running output has a streaming target'
  })
}
