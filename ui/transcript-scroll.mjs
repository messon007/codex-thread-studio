export function distanceFromBottom({ scrollHeight = 0, scrollTop = 0, clientHeight = 0 } = {}) {
  return Math.max(0, Number(scrollHeight) - Number(scrollTop) - Number(clientHeight))
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
