export function createCodexViewModel() {
  return {
    threadId: null,
    turns: [],
    activeTurnId: null,
    status: 'disconnected',
    error: null,
    diff: '',
    usage: null,
    approvals: [],
  }
}

export function hydrateCodexThread(model, thread) {
  model.threadId = thread?.id || model.threadId
  model.turns = Array.isArray(thread?.turns) ? structuredCloneSafe(thread.turns) : []
  model.error = null
  const active = [...model.turns].reverse().find((turn) => turn?.status === 'inProgress')
  model.activeTurnId = active?.id || null
  model.status = model.activeTurnId ? 'running' : 'idle'
  return model
}

export function applyCodexNotification(model, message) {
  const method = message?.method
  const params = message?.params || {}
  if (!method) return false

  if (method === 'turn/started') {
    upsertTurn(model, params.turn || {})
    model.activeTurnId = params.turn?.id || model.activeTurnId
    model.status = 'running'
    model.error = null
    return true
  }
  if (method === 'turn/completed') {
    upsertTurn(model, params.turn || {})
    if (model.activeTurnId === params.turn?.id) model.activeTurnId = null
    model.status = params.turn?.status === 'failed' ? 'failed' : 'idle'
    model.error = params.turn?.error?.message || null
    return true
  }
  if (method === 'item/started' || method === 'item/completed') {
    const turn = ensureTurn(model, params.turnId)
    upsertItem(turn, params.item || {})
    return true
  }
  if (method === 'item/agentMessage/delta') {
    const item = ensureItem(model, params.turnId, params.itemId, 'agentMessage')
    item.text = `${item.text || ''}${params.delta || ''}`
    return true
  }
  if (method === 'item/reasoning/summaryTextDelta') {
    const item = ensureItem(model, params.turnId, params.itemId, 'reasoning')
    if (!Array.isArray(item.summary)) item.summary = []
    const index = Number.isInteger(params.summaryIndex) ? params.summaryIndex : 0
    item.summary[index] = `${item.summary[index] || ''}${params.delta || ''}`
    return true
  }
  if (method === 'item/reasoning/textDelta') {
    const item = ensureItem(model, params.turnId, params.itemId, 'reasoning')
    if (!Array.isArray(item.content)) item.content = []
    const index = Number.isInteger(params.contentIndex) ? params.contentIndex : 0
    item.content[index] = `${item.content[index] || ''}${params.delta || ''}`
    return true
  }
  if (method === 'item/commandExecution/outputDelta') {
    const item = ensureItem(model, params.turnId, params.itemId, 'commandExecution')
    item.aggregatedOutput = `${item.aggregatedOutput || ''}${params.delta || ''}`
    return true
  }
  if (method === 'turn/plan/updated') {
    const turn = ensureTurn(model, params.turnId)
    upsertItem(turn, {
      id: `studio-plan-${params.turnId || 'active'}`,
      type: 'planUpdate',
      explanation: params.explanation || '',
      plan: Array.isArray(params.plan) ? params.plan : [],
    })
    return true
  }
  if (method === 'turn/diff/updated') {
    model.diff = params.diff || ''
    return true
  }
  if (method === 'thread/tokenUsage/updated') {
    model.usage = params.tokenUsage || params.usage || params
    return true
  }
  if (method === 'thread/status/changed' && (!model.threadId || params.threadId === model.threadId)) {
    model.status = normalizeThreadStatus(params.status)
    return true
  }
  if (method === 'error') {
    model.error = params.error?.message || params.message || 'Codex turn failed'
    model.status = 'failed'
    return true
  }
  if (message.id != null && method.includes('/requestApproval')) {
    const existing = model.approvals.findIndex((approval) => approval.id === message.id)
    const approval = { id: message.id, method, params }
    if (existing >= 0) model.approvals[existing] = approval
    else model.approvals.push(approval)
    return true
  }
  if (method === 'serverRequest/resolved') {
    model.approvals = model.approvals.filter((approval) => String(approval.id) !== String(params.requestId))
    return true
  }
  return false
}

export function resolveCodexApproval(model, requestId) {
  model.approvals = model.approvals.filter((approval) => String(approval.id) !== String(requestId))
}

export function textFromUserContent(content) {
  if (!Array.isArray(content)) return ''
  return content
    .filter((item) => item?.type === 'text')
    .map((item) => item.text || '')
    .join('\n')
}

function normalizeThreadStatus(status) {
  if (status?.type === 'active') return 'running'
  if (status?.type === 'systemError') return 'failed'
  return status?.type || 'idle'
}

function upsertTurn(model, incoming) {
  const id = incoming?.id || model.activeTurnId || `pending-${model.turns.length}`
  const index = model.turns.findIndex((turn) => turn.id === id)
  if (index < 0) {
    model.turns.push({ id, status: incoming.status || 'inProgress', items: [], ...structuredCloneSafe(incoming) })
    return model.turns.at(-1)
  }
  const items = model.turns[index].items || []
  model.turns[index] = { ...model.turns[index], ...structuredCloneSafe(incoming) }
  if (!Array.isArray(incoming.items) || incoming.items.length === 0) model.turns[index].items = items
  return model.turns[index]
}

function ensureTurn(model, turnId) {
  const id = turnId || model.activeTurnId || `pending-${model.turns.length}`
  let turn = model.turns.find((candidate) => candidate.id === id)
  if (!turn) {
    turn = { id, status: 'inProgress', items: [] }
    model.turns.push(turn)
  }
  if (!Array.isArray(turn.items)) turn.items = []
  return turn
}

function upsertItem(turn, incoming) {
  const id = incoming?.id || `anonymous-${turn.items.length}`
  const index = turn.items.findIndex((item) => item.id === id)
  if (index < 0) {
    turn.items.push({ ...structuredCloneSafe(incoming), id })
    return turn.items.at(-1)
  }
  turn.items[index] = { ...turn.items[index], ...structuredCloneSafe(incoming), id }
  return turn.items[index]
}

function ensureItem(model, turnId, itemId, type) {
  const turn = ensureTurn(model, turnId)
  let item = turn.items.find((candidate) => candidate.id === itemId)
  if (!item) {
    item = { id: itemId || `stream-${turn.items.length}`, type }
    turn.items.push(item)
  }
  return item
}

function structuredCloneSafe(value) {
  if (typeof structuredClone === 'function') return structuredClone(value)
  return JSON.parse(JSON.stringify(value))
}
