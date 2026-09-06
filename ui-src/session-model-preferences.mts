export interface SessionModelOptions {
  model?: string
  effort?: string
  [option: string]: unknown
}

export function normalizeStoredTurnOptions(value: unknown): Record<string, SessionModelOptions> {
  const normalized: Record<string, SessionModelOptions> = {}
  if (!value || typeof value !== 'object' || Array.isArray(value)) return normalized
  for (const [key, entry] of Object.entries(value).slice(0, 2048)) {
    if (typeof key !== 'string' || !key.includes(':') || key.length > 320
      || !isRecord(entry)) continue
    const model = String(entry.model || '').trim().slice(0, 256)
    const effort = String(entry.effort || '').trim().slice(0, 64)
    if (!model && !effort) continue
    if (/\p{Cc}/u.test(model) || /\p{Cc}/u.test(effort)) continue
    normalized[key] = {
      ...(model ? { model } : {}),
      ...(effort ? { effort } : {}),
    }
  }
  return normalized
}

export function sessionModelPreferencePayload(sessionKey: string, options: SessionModelOptions = {}) {
  return {
    sessionKey,
    model: String(options.model || ''),
    effort: String(options.effort || ''),
  }
}

/** Read at dispatch time, not enqueue time. Preserve transient turn options too. */
export function copySessionTurnOptions(
  stored: Record<string, SessionModelOptions>, key: string, defaults: SessionModelOptions,
): SessionModelOptions {
  return { ...(stored[key] || defaults) }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object' && !Array.isArray(value)
}
