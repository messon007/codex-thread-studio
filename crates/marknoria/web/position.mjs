export function readPosition(token, id) {
  try {
    const value = JSON.parse(sessionStorage.getItem(`reader-position:${token}:${id}`))
    return { line: Math.max(1, Math.trunc(Number(value?.line)) || 1), view: ['source', 'edit'].includes(value?.view) ? value.view : 'preview' }
  } catch { return { line: 1, view: 'preview' } }
}

export function writePosition(token, id, position) {
  if (!id || !position) return
  try { sessionStorage.setItem(`reader-position:${token}:${id}`, JSON.stringify(position)) } catch {}
}
