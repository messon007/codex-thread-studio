export function distanceFromBottom({ scrollHeight = 0, scrollTop = 0, clientHeight = 0 } = {}) {
  return Math.max(0, Number(scrollHeight) - Number(scrollTop) - Number(clientHeight))
}

export function shouldFollowLatestOnReturn({ orderedTurnIds = [], readingTurnId = '', recentTurnCount = 2, bottomDistance = Infinity, bottomThreshold = 48 } = {}) {
  const ids = orderedTurnIds.map((id) => String(id || '')).filter(Boolean)
  const readingId = String(readingTurnId || '')
  if (readingId && ids.length) return ids.slice(-Math.max(1, recentTurnCount)).includes(readingId)
  return Number(bottomDistance) <= bottomThreshold
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
    handleScroll(metrics) {
      following = distanceFromBottom(metrics) <= threshold
      return following
    },
  }
}

export function createTranscriptContentObserver({ ResizeObserverClass = globalThis.ResizeObserver, onResize } = {}) {
  const observer = typeof ResizeObserverClass === 'function' && typeof onResize === 'function'
    ? new ResizeObserverClass(() => onResize())
    : null

  return {
    observe(container) {
      observer?.disconnect()
      if (!container) return
      for (const child of container.children || []) observer?.observe(child)
    },
    disconnect() {
      observer?.disconnect()
    },
  }
}
