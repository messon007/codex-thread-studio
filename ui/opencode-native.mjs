export function normalizeOpenCodeSessions(sessions, statuses = {}) {
  return (Array.isArray(sessions) ? sessions : []).map((session) => ({
    id: session.id,
    name: session.title || '',
    preview: session.title || '',
    cwd: session.directory || '',
    parentThreadId: session.parentID || null,
    status: normalizeOpenCodeStatus(statuses?.[session.id] || 'idle'),
    source: 'opencode',
    model: session.modelID || session.model?.modelID || (session.model?.providerID && session.model?.id ? `${session.model.providerID}/${session.model.id}` : ''),
    updatedAt: session.time?.updated || session.time?.created || 0,
    native: session,
  }))
}

export function openCodeThreadFromHistory(session, messages, status) {
  const turns = []
  const turnById = new Map()
  const messageTurns = {}
  const messageRoles = {}
  let current = null

  for (const message of Array.isArray(messages) ? messages : []) {
    const info = message?.info || {}
    if (info.id) messageRoles[info.id] = info.role
    if (info.role === 'user') {
      current = {
        id: info.id || `user-${turns.length}`,
        status: 'completed',
        items: [{
          id: info.id || `user-item-${turns.length}`,
          type: 'userMessage',
          content: (message.parts || []).flatMap(userContentFromPart),
        }],
      }
      turns.push(current)
      turnById.set(current.id, current)
      if (info.id) messageTurns[info.id] = current.id
      continue
    }

    const turnId = info.parentID || current?.id || info.id || `assistant-${turns.length}`
    current = turnById.get(turnId) || current
    if (!current) {
      current = { id: turnId, status: 'completed', items: [] }
      turns.push(current)
      turnById.set(turnId, current)
    }
    if (info.id) messageTurns[info.id] = current.id
    for (const part of message.parts || []) upsertItem(current, openCodePartToItem(part, info.role))
    if (info.error) {
      current.status = 'failed'
      current.error = { message: errorText(info.error) }
    }
  }

  const normalizedStatus = normalizeOpenCodeStatus(status)
  const active = normalizedStatus === 'running' ? turns.at(-1) : null
  if (active) active.status = 'inProgress'
  return {
    id: session?.id,
    name: session?.title || '',
    cwd: session?.directory || '',
    parentThreadId: session?.parentID || null,
    source: 'opencode',
    status: normalizedStatus,
    turns,
    messageTurns,
    messageRoles,
  }
}

export function applyOpenCodeEvent(model, event, selectedSessionId) {
  const payload = event?.payload || event
  const type = payload?.type
  const properties = payload?.properties || {}
  const sessionId = properties.sessionID || properties.sessionId || properties.info?.sessionID || properties.part?.sessionID
  if (sessionId && selectedSessionId && sessionId !== selectedSessionId) return { handled: false, sessionId, type }

  if (type === 'session.status') {
    model.status = normalizeOpenCodeStatus(properties.status)
    if (model.status !== 'running') model.activeTurnId = null
    else model.activeTurnId ||= model.turns.at(-1)?.id || null
    return { handled: true, kind: 'metadata', sessionId, type }
  }
  if (type === 'session.idle') {
    model.status = 'idle'
    const active = model.turns.at(-1)
    const turnId = model.activeTurnId || active?.id || null
    model.activeTurnId = null
    if (active?.status === 'inProgress') active.status = 'completed'
    return { handled: true, kind: 'full', sessionId, type, turnId }
  }
  if (type === 'session.error') {
    model.error = errorText(properties.error || properties)
    model.status = 'failed'
    model.activeTurnId = null
    return { handled: true, kind: 'full', sessionId, type }
  }
  if (type === 'message.updated') {
    const info = properties.info || {}
    let turn = null
    model.messageRoles ||= {}
    if (info.id) model.messageRoles[info.id] = info.role
    if (info.role === 'user') {
      turn = ensureTurn(model, info.id)
      model.messageTurns ||= {}
      model.messageTurns[info.id] = turn.id
      model.activeTurnId = turn.id
      model.status = 'running'
    } else if (info.id) {
      turn = ensureTurn(model, info.parentID || model.activeTurnId)
      model.messageTurns ||= {}
      model.messageTurns[info.id] = turn.id
      if (info.error) {
        turn.status = 'failed'
        turn.error = { message: errorText(info.error) }
      }
    }
    return { handled: true, kind: 'full', sessionId, type, turnId: turn?.id || null }
  }
  if (type === 'message.part.updated') {
    const part = properties.part || {}
    const turn = ensureTurn(model, model.messageTurns?.[part.messageID] || model.activeTurnId)
    upsertItem(turn, openCodePartToItem(part, model.messageRoles?.[part.messageID] || 'assistant'))
    return { handled: true, kind: 'full', sessionId, type, turnId: turn.id }
  }
  if (type === 'message.part.delta') {
    const turn = ensureTurn(model, model.messageTurns?.[properties.messageID] || model.activeTurnId)
    let item = turn.items.find((candidate) => candidate.id === properties.partID)
    if (!item) {
      item = { id: properties.partID, type: properties.field === 'text' ? 'agentMessage' : 'unknown' }
      turn.items.push(item)
    }
    const field = properties.field || 'text'
    if (item.type === 'reasoning') {
      item.content = [`${item.content?.[0] || ''}${properties.delta || ''}`]
    } else {
      item[field] = `${item[field] || ''}${properties.delta || ''}`
    }
    return { handled: true, kind: 'stream', sessionId, type, turnId: turn.id, itemId: item.id }
  }
  if (type === 'permission.asked') {
    const id = properties.id || properties.requestID || properties.permissionID
    if (id && !model.approvals.some((approval) => String(approval.id) === String(id))) {
      model.approvals.push({ id, method: 'opencode/permission', params: properties })
    }
    return { handled: true, kind: 'full', sessionId, type }
  }
  if (type === 'permission.replied') {
    const id = properties.id || properties.requestID || properties.permissionID
    model.approvals = model.approvals.filter((approval) => String(approval.id) !== String(id))
    return { handled: true, kind: 'full', sessionId, type }
  }
  if (type === 'session.diff') {
    model.diff = typeof properties.diff === 'string' ? properties.diff : JSON.stringify(properties.diff || properties, null, 2)
    return { handled: true, kind: 'metadata', sessionId, type }
  }
  return { handled: false, sessionId, type }
}

export function normalizeOpenCodeStatus(status) {
  const type = status?.type || status
  if (type === 'busy' || type === 'active' || type === 'retry') return 'running'
  if (type === 'error') return 'failed'
  return type === 'idle' ? 'idle' : 'notLoaded'
}

export function openCodeModelList(providerResult) {
  const providers = providerResult?.providers || providerResult?.all || []
  const connected = new Set(providerResult?.connected || [])
  return providers.filter((provider) => !connected.size || connected.has(provider.id)).flatMap((provider) =>
    Object.entries(provider.models || {}).map(([id, model]) => ({
      id: `${provider.id}/${id}`,
      model: `${provider.id}/${id}`,
      displayName: `${model?.name || id} · ${provider.name || provider.id}`,
      isDefault: providerResult?.default?.[provider.id] === id || providerResult?.default?.[provider.id] === `${provider.id}/${id}`,
      supportedReasoningEfforts: [],
    })),
  )
}

export function splitOpenCodeModel(value) {
  const [providerID, ...model] = String(value || '').split('/')
  return providerID && model.length ? { providerID, modelID: model.join('/') } : null
}

function userContentFromPart(part) {
  if (part?.type === 'text') return [{ type: 'text', text: part.text || '' }]
  if (part?.type === 'file') return [{ type: 'text', text: `@${part.filename || part.url || part.path || 'file'}` }]
  return []
}

function openCodePartToItem(part, role) {
  const id = part?.id || `${part?.messageID || 'message'}-${part?.type || 'part'}`
  if (part?.type === 'text') return { id, type: role === 'user' ? 'userMessage' : 'agentMessage', text: part.text || '', content: [{ type: 'text', text: part.text || '' }] }
  if (part?.type === 'reasoning') return { id, type: 'reasoning', content: [part.text || ''] }
  if (part?.type === 'tool') {
    const state = part.state || {}
    if (['bash', 'shell'].includes(part.tool)) {
      return { id, type: 'commandExecution', command: state.input?.command || state.title || '', aggregatedOutput: state.output || state.error || '', status: toolStatus(state.status) }
    }
    return { id, type: 'mcpToolCall', server: 'OpenCode', tool: part.tool || 'tool', arguments: state.input || {}, result: state.output || '', error: state.error || '', status: toolStatus(state.status) }
  }
  if (part?.type === 'patch') return { id, type: 'fileChange', changes: [{ kind: 'patch', path: (part.files || []).join(', '), diff: part.hash || '' }], status: 'completed' }
  if (part?.type === 'compaction') return { id, type: 'contextCompaction' }
  if (part?.type === 'step-finish') return { id, type: 'stepFinish', tokens: part.tokens, cost: part.cost, reason: part.reason }
  return { ...part, id, type: part?.type || 'unknown' }
}

function toolStatus(status) {
  return ({ pending: 'inProgress', running: 'inProgress', completed: 'completed', error: 'failed' })[status] || status || 'inProgress'
}

function ensureTurn(model, id) {
  const turnId = id || model.activeTurnId || `opencode-${model.turns.length}`
  let turn = model.turns.find((candidate) => candidate.id === turnId)
  if (!turn) {
    turn = { id: turnId, status: 'inProgress', items: [] }
    model.turns.push(turn)
  }
  if (!Array.isArray(turn.items)) turn.items = []
  return turn
}

function upsertItem(turn, item) {
  if (!item) return
  const index = turn.items.findIndex((candidate) => candidate.id === item.id)
  if (index < 0) turn.items.push(item)
  else turn.items[index] = { ...turn.items[index], ...item }
}

function errorText(error) {
  if (typeof error === 'string') return error
  return error?.data?.message || error?.message || error?.name || JSON.stringify(error || {})
}
