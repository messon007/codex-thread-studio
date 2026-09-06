interface NavigationTurn { items?: { type?: string; content?: { type?: string; text?: string }[] }[] }
export function turnPromptPreview(turn: NavigationTurn | null | undefined, maxLength = 72) {
  const userMessage = (turn?.items || []).find((item) => item?.type === 'userMessage')
  const content: { type?: string; text?: string }[] = Array.isArray(userMessage?.content) ? userMessage.content : []
  const textParts = content.filter((item) => item?.type === 'text').map((item) => item.text || '')
  return compactPromptParts(textParts, maxLength)
}

export function turnHasUserInput(turn: NavigationTurn | null | undefined) {
  return (turn?.items || []).some((item) => item?.type === 'userMessage')
}

export function navigableTurns<T extends NavigationTurn>(turns: readonly T[]) {
  return (turns || []).filter(turnHasUserInput)
}

export function activeTurnAtMarker(positions: readonly { id: string; top: number }[], marker: number, atBottom = false) {
  const available = (positions || []).filter((position) => position?.id)
  if (!available.length) return null
  if (atBottom) return available.at(-1)!.id

  let active = available[0]!
  for (const position of available) {
    if (position.top > marker) break
    active = position
  }
  return active.id
}

export function turnNavigationLabel(turn: NavigationTurn | null | undefined, index: number) {
  const preview = turnPromptPreview(turn)
  return preview ? `User input ${index + 1}: ${preview}` : `User input ${index + 1}`
}

function compactPromptParts(parts: readonly string[], limit: number) {
  const maximum = Math.max(1, Number.isFinite(limit) ? Math.floor(limit) : 72)
  const characters = []
  let pendingSpace = false
  let truncated = false
  outer: for (const part of parts) {
    if (characters.length) pendingSpace = true
    for (const character of String(part || '')) {
      if (/\s/u.test(character)) {
        if (characters.length) pendingSpace = true
        continue
      }
      if (pendingSpace) {
        if (characters.length >= maximum) {
          truncated = true
          break outer
        }
        characters.push(' ')
        pendingSpace = false
      }
      if (characters.length >= maximum) {
        truncated = true
        break outer
      }
      characters.push(character)
    }
  }
  if (!truncated) return characters.join('').trim()
  if (maximum === 1) return '…'
  return `${characters.slice(0, maximum - 1).join('').trimEnd()}…`
}

