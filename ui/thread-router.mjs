export const ROUTER_BACKENDS = Object.freeze(['codex', 'opencode'])
export const DEFAULT_FALLBACK_CONDITION = 'No regular session is suitable for the user query.'
export const ROUTER_CONTEXT_KEY = 'codex-thread-studio/thread-router'

export function sessionRefKey(backend, id) {
  const normalizedBackend = cleanBackend(backend)
  const normalizedId = cleanText(id, 256)
  return normalizedBackend && normalizedId ? `${normalizedBackend}:${normalizedId}` : ''
}

export function parseSessionRefKey(value) {
  const key = cleanText(value, 320)
  const separator = key.indexOf(':')
  if (separator <= 0) return null
  const backend = cleanBackend(key.slice(0, separator))
  const id = cleanText(key.slice(separator + 1), 256)
  return backend && id ? { backend, id, key: `${backend}:${id}` } : null
}

export function normalizeThreadRouter(value) {
  const source = value && typeof value === 'object' ? value : {}
  const controllers = {}
  for (const [backend, id] of Object.entries(source.controllers || {})) {
    const key = cleanBackend(backend)
    const controllerId = cleanText(id, 256)
    if (key && controllerId) controllers[key] = controllerId
  }
  const legacyThreadId = cleanText(source.threadId, 256)
  if (legacyThreadId && !controllers.codex) controllers.codex = legacyThreadId
  const requestedBackend = cleanBackend(source.controllerBackend)
  const controllerBackend = requestedBackend || (controllers.codex ? 'codex' : Object.keys(controllers)[0] || 'codex')
  const controllerKeys = new Set(Object.entries(controllers).map(([backend, id]) => sessionRefKey(backend, id)))
  const fallbacks = []
  const seenFallbacks = new Set()
  const appendFallback = (rawKey, rawCondition) => {
    const parsed = parseSessionRefKey(rawKey)
    const key = parsed?.key || sessionRefKey('codex', rawKey)
    if (!key || controllerKeys.has(key) || seenFallbacks.has(key) || fallbacks.length >= 3) return
    seenFallbacks.add(key)
    fallbacks.push({
      sessionKey: key,
      condition: cleanText(rawCondition, 4096) || DEFAULT_FALLBACK_CONDITION,
    })
  }
  for (const entry of Array.isArray(source.fallbacks) ? source.fallbacks : []) {
    if (entry && typeof entry === 'object') appendFallback(entry.sessionKey, entry.condition)
  }
  if (!fallbacks.length) {
    for (const [rawKey, raw] of Object.entries(source.responsibilities || {})) {
      if (raw?.fallback === 'fallback' || ['learning', 'general'].includes(raw?.fallback)) {
        appendFallback(rawKey, DEFAULT_FALLBACK_CONDITION)
      }
    }
  }
  return { controllerBackend, controllers, fallbacks }
}

export function migrateLegacyResponsibilities(openingMessages, router) {
  const messages = { ...(openingMessages || {}) }
  const normalized = normalizeThreadRouter(router)
  const controllerKeys = new Set(Object.entries(normalized.controllers).map(([backend, id]) => sessionRefKey(backend, id)))
  for (const [rawKey, raw] of Object.entries(router?.responsibilities || {}).slice(0, 2048)) {
    const parsed = parseSessionRefKey(rawKey)
    const key = parsed?.key || sessionRefKey('codex', rawKey)
    const description = cleanText(raw?.description, 4096)
    if (!key || controllerKeys.has(key) || !description || messages[key]?.responsibility) continue
    messages[key] = { ...(messages[key] || {}), responsibility: description }
  }
  return messages
}

export function routerControllerRef(config, backend = null) {
  const normalized = normalizeThreadRouter(config)
  const selectedBackend = cleanBackend(backend) || normalized.controllerBackend
  const id = normalized.controllers[selectedBackend]
  return id ? { backend: selectedBackend, id, key: sessionRefKey(selectedBackend, id) } : null
}

export function isRouterSession(config, backend, id) {
  return routerControllerRef(config, backend)?.id === String(id || '')
}

export function catalogsWithSingleRouter(config, catalogs) {
  const normalized = normalizeThreadRouter(config)
  const active = routerControllerRef(normalized)
  const controllerKeys = new Set(Object.entries(normalized.controllers).map(([backend, id]) => sessionRefKey(backend, id)))
  return Object.fromEntries(Object.entries(catalogs || {}).map(([backend, threads]) => [
    backend,
    (Array.isArray(threads) ? threads : []).filter((thread) => {
      const key = sessionRefKey(backend, thread?.id)
      return !controllerKeys.has(key) || key === active?.key
    }),
  ]))
}

export function managedRouterThread(threads, routerId, routerWorkspace) {
  const id = cleanText(routerId, 256)
  const cwd = cleanText(routerWorkspace, 4096)
  if (!id || !cwd) return null
  return (Array.isArray(threads) ? threads : []).find((thread) =>
    String(thread?.id || '') === id && String(thread?.cwd || '') === cwd,
  ) || null
}

export function recoverManagedRouterCatalog(threads, routerId, routerWorkspace, recovered) {
  const catalog = Array.isArray(threads) ? threads : []
  if (managedRouterThread(catalog, routerId, routerWorkspace)) return catalog
  if (!managedRouterThread([recovered], routerId, routerWorkspace)) return catalog
  return [recovered, ...catalog.filter((thread) => String(thread?.id || '') !== String(routerId || ''))]
}

export function shouldCreateManagedRouter(routerId) {
  return !cleanText(routerId, 256)
}

export function routerDecisionSchema(candidateKeys = []) {
  const allowedKeys = [...new Set((Array.isArray(candidateKeys) ? candidateKeys : [])
    .map((candidate) => typeof candidate === 'string' ? candidate : candidate?.key)
    .map((key) => parseSessionRefKey(key)?.key)
    .filter(Boolean))]
  return {
    type: 'object',
    additionalProperties: false,
    required: ['action', 'targetSessionKey', 'forwardedPrompt', 'reason', 'message'],
    properties: {
      action: { type: 'string', enum: ['dispatch'] },
      targetSessionKey: {
        type: 'string',
        ...(allowedKeys.length ? { enum: allowedKeys } : {}),
        description: 'Copy exactly one available sessionKey from the supplied catalog.',
      },
      forwardedPrompt: { type: 'string' },
      reason: { type: 'string' },
      message: { type: 'string' },
    },
  }
}

export function routerCandidates(config, catalogs, openingMessages = {}) {
  const normalized = normalizeThreadRouter(config)
  const controllerKeys = new Set(Object.entries(normalized.controllers).map(([backend, id]) => sessionRefKey(backend, id)))
  const fallbackByKey = new Map(normalized.fallbacks.map((entry) => [entry.sessionKey, entry.condition]))
  const entries = catalogs && !Array.isArray(catalogs)
    ? Object.entries(catalogs).flatMap(([backend, threads]) => (threads || []).map((thread) => ({ backend, thread })))
    : (Array.isArray(catalogs) ? catalogs : []).map((thread) => ({ backend: 'codex', thread }))
  return entries
    .map(({ backend, thread }) => ({ backend: cleanBackend(backend), thread }))
    .filter(({ backend, thread }) => backend && thread?.id && !thread.archived && !thread.ephemeral)
    .map(({ backend, thread }) => {
      const id = String(thread.id)
      const key = sessionRefKey(backend, id)
      const opening = openingMessages[key] || {}
      const fallbackCondition = fallbackByKey.get(key) || ''
      return {
        key,
        backend,
        id,
        title: cleanText(thread.name || thread.title || '', 300) || 'Untitled',
        cwd: cleanText(thread.cwd || '', 1024),
        responsibility: cleanText(opening.responsibility, 4096),
        fallback: fallbackCondition ? 'fallback' : 'none',
        fallbackCondition,
        openingMessage: cleanText(opening.text, 800),
      }
    })
    .filter((candidate) => candidate.key && !controllerKeys.has(candidate.key))
}

export function routerDeveloperInstructions(candidates) {
  const catalog = (Array.isArray(candidates) ? candidates : []).map((candidate) => ({
    sessionKey: candidate.key,
    backend: candidate.backend,
    sessionId: candidate.id,
    title: candidate.title,
    cwd: candidate.cwd,
    responsibility: candidate.responsibility || '(not explicitly configured; infer only from title, directory, and openingMessage)',
    fallback: candidate.fallback,
    fallbackCondition: candidate.fallbackCondition || '',
    openingMessage: candidate.openingMessage,
  }))
  return `You are the routing controller for Codex Thread Studio. You do not solve the user's task and you do not call tools.

You MUST always choose exactly one existing target session from the complete catalog below. Never ask the user to clarify and never return a clarification action. Make the best routing decision from all available session metadata, even when the match is imperfect.

Routing rules, in priority order:
1. An explicit session title or responsibility named by the user wins.
2. Prefer the most specific responsibility match regardless of backend.
3. First consider regular targets only. If no regular target is suitable, consider at most three fallback targets and choose one only when its fallbackCondition matches the situation.
4. If no fallback condition applies, choose the closest regular target. If only fallback targets exist, choose the closest fallback target.
5. targetSessionKey must be copied byte-for-byte from one sessionKey in the catalog below. Never shorten, translate, summarize, or invent it. Never choose a Router controller or dispatch to more than one target.
6. forwardedPrompt must preserve the user's actual request and useful context. Do not add an answer.
7. reason and message must be concise and written in the user's language.

Available target sessions:
${JSON.stringify(catalog, null, 2)}`
}

export function routerApplicationContext(candidates) {
  return {
    [ROUTER_CONTEXT_KEY]: {
      kind: 'application',
      value: routerDeveloperInstructions(candidates),
    },
  }
}

export function parseRouterDecision(value, candidateKeys = [], { allowLegacyClarify = false } = {}) {
  const parsed = parseJson(value)
  if (!parsed || typeof parsed !== 'object') throw new Error('Router did not return an object')
  if (parsed.action !== 'dispatch' && !(allowLegacyClarify && parsed.action === 'clarify')) {
    throw new Error('Router returned an unsupported action')
  }
  const allowed = candidateKeys.map(String)
  let targetSessionKey = cleanText(parsed.targetSessionKey, 320)
  if (!targetSessionKey && parsed.targetThreadId) {
    const legacyId = cleanText(parsed.targetThreadId, 256)
    const matches = allowed.filter((key) => parseSessionRefKey(key)?.id === legacyId)
    targetSessionKey = matches.length === 1 ? matches[0] : legacyId
  }
  const decision = {
    action: parsed.action,
    targetSessionKey,
    forwardedPrompt: cleanText(parsed.forwardedPrompt, 64 * 1024),
    reason: cleanText(parsed.reason, 4096),
    message: cleanText(parsed.message, 4096),
  }
  if (decision.action === 'dispatch') {
    if (!allowed.includes(decision.targetSessionKey)) throw new Error('Router selected a session outside the configured catalog')
    if (!decision.forwardedPrompt) throw new Error('Router returned an empty forwarded prompt')
  } else {
    decision.targetSessionKey = ''
    decision.forwardedPrompt = ''
    if (!decision.message) throw new Error('Router did not provide a clarification question')
  }
  return decision
}

export function finalAgentText(turn) {
  return (turn?.items || [])
    .filter((item) => item?.type === 'agentMessage')
    .map((item) => String(item.text || ''))
    .filter(Boolean)
    .join('\n')
    .trim()
}

export function routerDecisionForTurn(turn, candidateKeys = []) {
  const text = finalAgentText(turn)
  if (!text) return null
  try { return parseRouterDecision(text, candidateKeys, { allowLegacyClarify: true }) }
  catch { return null }
}

function parseJson(value) {
  const text = String(value || '').trim()
  if (!text) throw new Error('Router returned no result')
  const unwrapped = text.startsWith('```')
    ? text.replace(/^```(?:json)?\s*/u, '').replace(/\s*```$/u, '')
    : text
  return JSON.parse(unwrapped)
}

function cleanBackend(value) {
  const backend = cleanText(value, 64)
  return /^[a-z][a-z0-9_-]*$/u.test(backend) ? backend : ''
}

function cleanText(value, max) {
  return typeof value === 'string' ? [...value.trim()].slice(0, max).join('') : ''
}
