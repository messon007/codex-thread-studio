const DEFAULT_RECORD_LIMIT = 240
const MAX_DETAIL_STRING_LENGTH = 240

export function createPerformanceMonitor({
  limit = DEFAULT_RECORD_LIMIT,
  now = defaultNow,
  wallNow = Date.now,
} = {}) {
  const capacity = Math.max(1, Math.floor(Number(limit) || DEFAULT_RECORD_LIMIT))
  const records = new Array(capacity)
  let size = 0
  let cursor = 0
  let sequence = 0

  function record(name, durationMs, details = {}) {
    const entry = Object.freeze({
      sequence: ++sequence,
      name: String(name || 'unknown'),
      durationMs: roundDuration(durationMs),
      completedAt: Number(wallNow()),
      ...boundedDetails(details),
    })
    records[cursor] = entry
    cursor = (cursor + 1) % capacity
    size = Math.min(size + 1, capacity)
    return entry
  }

  function start(name, details = {}) {
    const startedAt = now()
    let completed = false
    return (completionDetails = {}) => {
      if (completed) return null
      completed = true
      return record(name, now() - startedAt, { ...details, ...completionDetails })
    }
  }

  function snapshot({ name = '', threadKey = '' } = {}) {
    const selected = orderedRecords().filter((entry) => {
      return (!name || entry.name === name) && (!threadKey || entry.threadKey === threadKey)
    })
    return {
      capacity,
      size: selected.length,
      records: selected.map((entry) => ({ ...entry })),
      summary: summarize(selected),
    }
  }

  function report(options = {}) {
    const result = snapshot(options)
    if (typeof globalThis.console?.table === 'function') globalThis.console.table(result.summary)
    return result
  }

  function clear() {
    records.fill(undefined)
    size = 0
    cursor = 0
  }

  function orderedRecords() {
    if (size < capacity) return records.slice(0, size)
    return [...records.slice(cursor), ...records.slice(0, cursor)]
  }

  return Object.freeze({ start, record, snapshot, report, clear })
}

export function exposePerformanceMonitor(monitor, target = globalThis) {
  if (!monitor || !target) return null
  const api = Object.freeze({
    snapshot: (options) => monitor.snapshot(options),
    report: (options) => monitor.report(options),
    clear: () => monitor.clear(),
  })
  Object.defineProperty(target, 'codexThreadStudioPerformance', {
    configurable: true,
    enumerable: false,
    value: api,
  })
  return api
}

function summarize(records) {
  const groups = new Map()
  for (const entry of records) {
    const durations = groups.get(entry.name) || []
    durations.push(entry.durationMs)
    groups.set(entry.name, durations)
  }
  return [...groups.entries()].map(([name, durations]) => {
    const sorted = [...durations].sort((left, right) => left - right)
    const totalMs = durations.reduce((total, duration) => total + duration, 0)
    return {
      name,
      count: durations.length,
      totalMs: roundDuration(totalMs),
      averageMs: roundDuration(totalMs / durations.length),
      p95Ms: roundDuration(sorted[Math.min(sorted.length - 1, Math.ceil(sorted.length * 0.95) - 1)]),
      maxMs: roundDuration(sorted.at(-1) || 0),
    }
  }).sort((left, right) => right.totalMs - left.totalMs)
}

function boundedDetails(details) {
  const output = {}
  for (const [key, value] of Object.entries(details || {})) {
    if (typeof value === 'string') output[key] = value.slice(0, MAX_DETAIL_STRING_LENGTH)
    else if (typeof value === 'number' && Number.isFinite(value)) output[key] = value
    else if (typeof value === 'boolean' || value == null) output[key] = value
  }
  return output
}

function roundDuration(value) {
  return Math.round(Math.max(0, Number(value) || 0) * 1_000) / 1_000
}

function defaultNow() {
  return typeof globalThis.performance?.now === 'function' ? globalThis.performance.now() : Date.now()
}
