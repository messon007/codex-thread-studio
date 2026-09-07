// Isolated smoke profile only: exercises real SQLite endpoints, no model requests.
export async function checkSessionMapAcceptance(page) {
  const html = await (await page.request.get(`${new URL(page.url()).origin}/`)).text()
  return page.evaluate(async html => {
    const check = (value, message) => { if (!value) throw Error(message) }
    const document = new DOMParser().parseFromString(html, 'text/html')
    const actions = [...document.querySelectorAll('[data-map-item-action]')].map(node => node.dataset.mapItemAction)
    check(JSON.stringify(actions) === JSON.stringify(['current', 'done']), 'node menu is not limited to two actions')
    check(!document.querySelector('#session-map-add-root, #session-map-edit-goal, #session-map-item-dialog, #session-map-goal-dialog'), 'manual Map editing remains')
    check(!document.querySelector('#session-map-create-goal').required, 'Map still requires a manually written goal')
    const headers = { Authorization: `Bearer ${window.__CODEX_THREAD_STUDIO_GATEWAY__.token}`, 'Content-Type': 'application/json' }
    const request = async (path, body) => {
      const response = await fetch(path, { headers, method: body ? 'POST' : 'GET', ...(body ? { body: JSON.stringify(body) } : {}) })
      check(response.ok, `Map request failed: ${response.status}`)
      return response.json()
    }
    const endpoint = '/studio/session-map/codex/map-acceptance'
    let map = await request('/studio/session-map', { backend: 'codex', threadId: 'map-acceptance', goal: 'Learn topics', structure: 'hierarchy', items: [{ id: 'a', title: 'A' }, { id: 'b', title: 'B' }] })
    check(map.items.every(item => item.state === 'notStarted'), 'new nodes are not incomplete')
    const apply = async operations => { map = await request(`${endpoint}/operations`, { actor: 'user', baseRevision: map.revision, operations }) }
    await apply([{ op: 'setCurrent', itemId: 'a' }])
    await apply([{ op: 'setCurrent', itemId: 'b' }])
    check(map.currentItemId === 'b' && map.items.filter(item => item.state === 'active').length === 1, 'multiple current nodes')
    check(map.items.find(item => item.id === 'a').state === 'notStarted', 'previous current did not become incomplete')
    await apply([{ op: 'setState', itemId: 'b', state: 'done' }])
    map = await request(endpoint)
    check(map.currentItemId === null && map.items.find(item => item.id === 'b').state === 'done', 'completion was not persisted correctly')
    return 'PASS: simplified Map menus, optional AI goal, incomplete defaults, exclusive current and durable completion'
  }, html)
}
