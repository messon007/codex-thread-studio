export function turnPromptPreview(turn, maxLength = 72) {
  const userMessage = (turn?.items || []).find((item) => item?.type === 'userMessage')
  const text = (userMessage?.content || [])
    .filter((item) => item?.type === 'text')
    .map((item) => item.text || '')
    .join(' ')
    .replace(/\s+/gu, ' ')
    .trim()
  if (!text) return ''
  if (text.length <= maxLength) return text
  return `${text.slice(0, Math.max(1, maxLength - 1)).trimEnd()}…`
}

export function turnHasUserInput(turn) {
  return (turn?.items || []).some((item) => item?.type === 'userMessage')
}

export function navigableTurns(turns) {
  return (turns || []).filter(turnHasUserInput)
}

export function activeTurnAtMarker(positions, marker, atBottom = false) {
  const available = (positions || []).filter((position) => position?.id)
  if (!available.length) return null
  if (atBottom) return available.at(-1).id

  let active = available[0]
  for (const position of available) {
    if (position.top > marker) break
    active = position
  }
  return active.id
}

export function turnNavigationLabel(turn, index) {
  const preview = turnPromptPreview(turn)
  return preview ? `User input ${index + 1}: ${preview}` : `User input ${index + 1}`
}
