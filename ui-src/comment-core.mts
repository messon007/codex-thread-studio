import type { CommentContext, CommentDraft, CommentProvider, CommentRecord, CommentSource } from './comment-types.mjs'

interface DraftOptions { idFactory?: (() => string) | undefined; now?: (() => string) | undefined; registry?: CommentSourceRegistry | undefined }
interface NormalizeOptions extends DraftOptions { migrateSource?: (draft: unknown) => unknown; limitPerOwner?: number }
interface PromptEntryOptions { index?: number | string; numberWidth?: number | string; anchor?: string; excerpt?: string; note?: string }

export function commentRecord(value: unknown): CommentRecord {
  return value !== null && typeof value === 'object' ? value as CommentRecord : {}
}

const MAX_TEXT_LENGTH = 16_000
const MAX_PROVIDER_LENGTH = 64

/**
 * Stable, source-agnostic Comment model.
 *
 * Source-specific semantics live behind providers. The core deliberately does
 * not know what a URL, file range, chat turn, EPUB CFI, or image region means.
 */
export class CommentSourceRegistry {
  #providers = new Map<string, Readonly<CommentProvider>>()

  register(provider: CommentProvider) {
    const id = String(provider?.id || '').trim().slice(0, MAX_PROVIDER_LENGTH)
    if (!id) throw new Error('Comment source provider requires an id')
    if (this.#providers.has(id)) throw new Error(`Comment source provider already registered: ${id}`)
    this.#providers.set(id, Object.freeze({ ...provider, id }))
    return this
  }

  has(id: unknown) {
    return this.#providers.has(String(id || ''))
  }

  normalizeSource(value: unknown): CommentSource {
    const source = commentRecord(value)
    const providerId = String(source?.provider || '').trim().slice(0, MAX_PROVIDER_LENGTH)
    const provider = this.#providers.get(providerId)
    const anchor = provider?.normalizeAnchor
      ? provider.normalizeAnchor(source?.anchor)
      : cloneSerializable(source?.anchor)
    return {
      provider: providerId || 'unknown',
      version: positiveInteger(source?.version) || 1,
      anchor: commentRecord(anchor),
    }
  }

  describe(draft: CommentDraft, context: CommentContext = {}) {
    const provider = this.#providers.get(draft?.source?.provider)
    return provider?.describe?.(draft, context) || context.unknownLabel || 'Comment'
  }

  promptAnchor(draft: CommentDraft, context: CommentContext = {}) {
    const provider = this.#providers.get(draft?.source?.provider)
    return String(provider?.promptAnchor?.(draft, context) || '')
  }

  promptInstructions(drafts: readonly CommentDraft[], context: CommentContext = {}) {
    const instructions = []
    const seen = new Set()
    for (const draft of drafts || []) {
      const provider = this.#providers.get(draft?.source?.provider)
      const instruction = String(provider?.promptInstruction?.(draft, context) || '').trim()
      if (instruction && !seen.has(instruction)) {
        seen.add(instruction)
        instructions.push(instruction)
      }
    }
    return instructions
  }

  async reopen(draft: CommentDraft, context: CommentContext = {}) {
    const provider = this.#providers.get(draft?.source?.provider)
    if (!provider?.reopen) return false
    await provider.reopen(draft, context)
    return true
  }
}

export function createCommentDraft(value: unknown, { idFactory = defaultIdFactory, now = () => new Date().toISOString(), registry }: DraftOptions = {}): CommentDraft | null {
  const input = commentRecord(value)
  const excerpt = boundedText(input?.excerpt ?? input?.quote)
  if (!excerpt) return null
  const source = registry
    ? registry.normalizeSource(input?.source)
    : normalizeOpaqueSource(input?.source)
  return {
    id: boundedText(input?.id, 128) || idFactory(),
    excerpt,
    note: boundedText(input?.note ?? input?.comment),
    createdAt: boundedText(input?.createdAt, 128) || now(),
    source,
  }
}

export function normalizeCommentDrafts(value: unknown, {
  idFactory = defaultIdFactory,
  migrateSource = () => ({ provider: 'unknown', version: 1, anchor: {} }),
  registry,
  limitPerOwner = 32,
}: NormalizeOptions = {}): Record<string, CommentDraft[]> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return {}
  return Object.fromEntries(Object.entries(value).flatMap(([ownerId, drafts]) => {
    if (!ownerId || !Array.isArray(drafts)) return []
    const normalized = drafts.slice(0, limitPerOwner).flatMap((draft) => {
      const created = createCommentDraft({
        ...commentRecord(draft),
        source: commentRecord(draft).source || migrateSource(draft),
      }, { idFactory, registry })
      return created ? [created] : []
    })
    const key = ownerId.includes(':') ? ownerId : `codex:${ownerId}`
    return normalized.length ? [[key, normalized]] : []
  }))
}

export function commentSelectionSnapshot(value: unknown, registry?: CommentSourceRegistry) {
  const selection = commentRecord(value)
  return createCommentDraft({
    excerpt: selection?.excerpt ?? selection?.quote,
    source: selection?.source,
  }, { idFactory: () => '', now: () => '', registry })
}

export function formatCommentPromptEntry({ index = 0, numberWidth = 0, anchor = '', excerpt = '', note = '' }: PromptEntryOptions = {}) {
  const number = Math.max(0, Number.parseInt(String(index), 10) || 0) + 1
  const width = Math.max(String(number).length, Number.parseInt(String(numberWidth), 10) || 0)
  const numberLabel = String(number).padStart(width, ' ')
  const location = String(anchor || '').trim().replace(/\s+/gu, ' ')
  const excerptText = String(excerpt || '').trim().replace(/\r\n?/gu, '\n')
  const quote = excerptText ? `${numberLabel} > ${excerptText}` : ''
  const comment = String(note || '').trim().replace(/\r\n?/gu, '\n')
  const markerIndent = ' '.repeat(width + 1)
  return [
    quote,
    location ? `${markerIndent}@ ${location}` : '',
    comment ? `${markerIndent}< ${comment}` : '',
  ].filter(Boolean).join('\n')
}

function normalizeOpaqueSource(value: unknown): CommentSource {
  const source = commentRecord(value)
  return {
    provider: boundedText(source?.provider, MAX_PROVIDER_LENGTH) || 'unknown',
    version: positiveInteger(source?.version) || 1,
    anchor: cloneSerializable(source?.anchor) || {},
  }
}

function boundedText(value: unknown, limit = MAX_TEXT_LENGTH) {
  return String(value || '').trim().slice(0, limit)
}

function positiveInteger(value: unknown) {
  const number = Number(value)
  return Number.isInteger(number) && number > 0 ? number : null
}

function cloneSerializable(value: unknown): CommentRecord {
  if (!value || typeof value !== 'object') return {}
  try {
    return commentRecord(JSON.parse(JSON.stringify(value)))
  } catch {
    return {}
  }
}

function defaultIdFactory() {
  return globalThis.crypto?.randomUUID?.() || `${Date.now()}-${Math.random().toString(16).slice(2)}`
}
