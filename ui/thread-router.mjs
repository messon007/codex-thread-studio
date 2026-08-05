export const ROUTER_FALLBACKS = Object.freeze(['none', 'fallback'])

export function normalizeThreadRouter(value) {
  const source = value && typeof value === 'object' ? value : {}
  const threadId = cleanText(source.threadId, 256) || null
  const responsibilities = {}
  for (const [id, raw] of Object.entries(source.responsibilities || {}).slice(0, 2048)) {
    const thread = cleanText(id, 256)
    if (!thread || thread === threadId) continue
    const entry = raw && typeof raw === 'object' ? raw : {}
    const legacyFallback = ['learning', 'general'].includes(entry.fallback)
    responsibilities[thread] = {
      description: cleanText(entry.description, 4096),
      fallback: legacyFallback ? 'fallback' : ROUTER_FALLBACKS.includes(entry.fallback) ? entry.fallback : 'none',
    }
  }
  return { threadId, responsibilities }
}

export function routerDecisionSchema() {
  return {
    type: 'object',
    additionalProperties: false,
    required: ['action', 'targetThreadId', 'forwardedPrompt', 'reason', 'message'],
    properties: {
      action: { type: 'string', enum: ['dispatch', 'clarify'] },
      targetThreadId: { type: 'string' },
      forwardedPrompt: { type: 'string' },
      reason: { type: 'string' },
      message: { type: 'string' },
    },
  }
}

export function routerCandidates(config, threads, openingMessages = {}) {
  const normalized = normalizeThreadRouter(config)
  return (Array.isArray(threads) ? threads : [])
    .filter((thread) => thread?.id && String(thread.id) !== normalized.threadId && !thread.archived && !thread.ephemeral)
    .map((thread) => {
      const id = String(thread.id)
      const responsibility = normalized.responsibilities[id] || { description: '', fallback: 'none' }
      const opening = openingMessages[`codex:${id}`]?.text || ''
      return {
        id,
        title: cleanText(thread.name || thread.title || '', 300) || 'Untitled',
        cwd: cleanText(thread.cwd || '', 1024),
        responsibility: cleanText(responsibility.description, 4096),
        fallback: responsibility.fallback === 'fallback' ? 'fallback' : 'none',
        openingMessage: cleanText(opening, 800),
      }
    })
}

export function routerDeveloperInstructions(candidates) {
  const catalog = (Array.isArray(candidates) ? candidates : []).map((candidate) => ({
    threadId: candidate.id,
    title: candidate.title,
    cwd: candidate.cwd,
    responsibility: candidate.responsibility || '(not explicitly configured; infer only from title, directory, and openingMessage)',
    fallback: candidate.fallback,
    openingMessage: candidate.openingMessage,
  }))
  return `You are the routing controller for Codex Thread Studio. You do not solve the user's task and you do not call tools.

Choose exactly one existing target thread when the request can be routed. Return action "clarify" only when no safe choice can be made.

Routing rules, in priority order:
1. An explicit thread title or responsibility named by the user wins.
2. Prefer the most specific responsibility match.
3. If no specific target matches, consider only targets whose fallback is "fallback", then choose the best one by comparing their responsibility descriptions with the request.
4. If no fallback responsibility is suitable, ask a concise clarification question.
5. Never invent a thread id, never choose the router itself, and never dispatch to more than one target.
6. forwardedPrompt must preserve the user's actual request and useful context. Do not add an answer.
7. reason and message must be concise and written in the user's language.

Available target threads:
${JSON.stringify(catalog, null, 2)}`
}

export function parseRouterDecision(value, candidateIds = []) {
  const parsed = parseJson(value)
  if (!parsed || typeof parsed !== 'object') throw new Error('Router did not return an object')
  if (!['dispatch', 'clarify'].includes(parsed.action)) throw new Error('Router returned an unsupported action')
  const decision = {
    action: parsed.action,
    targetThreadId: cleanText(parsed.targetThreadId, 256),
    forwardedPrompt: cleanText(parsed.forwardedPrompt, 64 * 1024),
    reason: cleanText(parsed.reason, 4096),
    message: cleanText(parsed.message, 4096),
  }
  if (decision.action === 'dispatch') {
    if (!candidateIds.map(String).includes(decision.targetThreadId)) throw new Error('Router selected a thread outside the configured catalog')
    if (!decision.forwardedPrompt) throw new Error('Router returned an empty forwarded prompt')
  } else {
    decision.targetThreadId = ''
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

export function routerDecisionForTurn(turn, candidateIds = []) {
  const text = finalAgentText(turn)
  if (!text) return null
  try { return parseRouterDecision(text, candidateIds) }
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

function cleanText(value, max) {
  return typeof value === 'string' ? value.trim().slice(0, max) : ''
}
