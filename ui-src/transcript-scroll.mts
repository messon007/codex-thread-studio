import type { ScrollMetrics, ScrollState, RestorePlan } from './transcript-types.mjs'

export function distanceFromBottom({ scrollHeight = 0, scrollTop = 0, clientHeight = 0 }: ScrollMetrics = {}) {
  return Math.max(0, Number(scrollHeight) - Number(scrollTop) - Number(clientHeight))
}

export function shouldPinTranscriptOnTakeover({ metrics, hasLaterTurns = false, bottomThreshold = 48 }: { metrics?: ScrollMetrics; hasLaterTurns?: boolean; bottomThreshold?: number } = {}) {
  return Boolean(hasLaterTurns) || distanceFromBottom(metrics) > bottomThreshold
}

export function shouldFollowLatestOnReturn({ orderedTurnIds = [], readingTurnId = '', recentTurnCount = 2, bottomDistance = Infinity, bottomThreshold = 48 }: { orderedTurnIds?: string[]; readingTurnId?: string; recentTurnCount?: number; bottomDistance?: number; bottomThreshold?: number } = {}) {
  const ids = orderedTurnIds.map((id) => String(id || '')).filter(Boolean)
  const readingId = String(readingTurnId || '')
  const nearBottom = Number(bottomDistance) <= bottomThreshold
  if (readingId && ids.length) return nearBottom && ids.slice(-Math.max(1, recentTurnCount)).includes(readingId)
  return nearBottom
}

export function transcriptRestorePlan({ saved, historyReady = true, historyComplete = true, orderedTurnIds = [] }: { saved?: ScrollState | null; historyReady?: boolean; historyComplete?: boolean; orderedTurnIds?: string[] } = {}): RestorePlan {
  if (!saved) return { type: 'none' }
  if (!historyReady) return { type: 'defer' }
  const anchorTurnId = String(saved.anchorTurnId || '')
  if (anchorTurnId) {
    const available = orderedTurnIds.some((id) => String(id || '') === anchorTurnId)
    if (available) return { type: 'anchor', anchorTurnId }
    return { type: historyComplete ? 'stale' : 'unavailable' }
  }
  if (Number.isFinite(saved.scrollTop)) return { type: 'scrollTop', scrollTop: Number(saved.scrollTop) }
  return { type: 'stale' }
}

export function transcriptResizeAction({ hasPendingRestore = false, following = true } = {}) {
  if (hasPendingRestore) return 'restore'
  return following ? 'follow' : 'preserve'
}

export function transcriptScrollEventAction({ hasPendingRestore = false, userInitiated = false, following = true } = {}) {
  if (hasPendingRestore) return 'ignore'
  if (userInitiated) return 'capture'
  return following ? 'follow' : 'preserve'
}

export function createTranscriptScrollFollower({ threshold = 48, initial = true } = {}) {
  let following = Boolean(initial)
  return {
    get following() {
      return following
    },
    reset() {
      following = true
    },
    pause() {
      following = false
    },
    handleScroll(metrics: ScrollMetrics) {
      following = distanceFromBottom(metrics) <= threshold
      return following
    },
  }
}

export function createTranscriptContentObserver({ ResizeObserverClass = globalThis.ResizeObserver, onResize }: { ResizeObserverClass?: new (callback: () => void) => { disconnect(): void; observe(element: Element): void }; onResize?: () => void } = {}) {
  const observer = typeof ResizeObserverClass === 'function' && typeof onResize === 'function'
    ? new ResizeObserverClass(() => onResize())
    : null

  return {
    observe(container: Element | null) {
      observer?.disconnect()
      if (!container) return
      for (const child of container.children || []) observer?.observe(child)
    },
    disconnect() {
      observer?.disconnect()
    },
  }
}
