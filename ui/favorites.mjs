export function favoriteSourceKey(value = {}) {
  return [value.backend, value.threadId, value.turnId, value.itemId]
    .map((part) => String(part || ''))
    .join(':')
}

export function questionForTurn(turn) {
  const message = (turn?.items || []).find((item) => item?.type === 'userMessage')
  return userContentText(message?.content).trim()
}

export function autoFavoriteTitle(content, fallback = '收藏的 AI 回复') {
  const firstMeaningfulLine = String(content || '')
    .split('\n')
    .map((line) => line
      .replace(/^\s{0,3}#{1,6}\s+/u, '')
      .replace(/^\s*[-*+]\s+/u, '')
      .replace(/[*_`~[\]()>]/gu, '')
      .trim())
    .find(Boolean)
  const value = (firstMeaningfulLine || fallback).replace(/\s+/gu, ' ').trim()
  return value.length > 80 ? `${[...value].slice(0, 79).join('')}…` : value
}

export function normalizeFavoriteTags(value) {
  const tags = String(value || '')
    .split(/[,，]/u)
    .map((tag) => tag.trim())
    .filter(Boolean)
  return [...new Set(tags)].slice(0, 20)
}

export function favoriteCopyText(favorite) {
  return [
    favorite?.question && `问题：\n${favorite.question}`,
    favorite?.content && `回答：\n${favorite.content}`,
    favorite?.note && `收藏笔记：\n${favorite.note}`,
  ].filter(Boolean).join('\n\n')
}

function userContentText(content) {
  if (typeof content === 'string') return content
  if (!Array.isArray(content)) return ''
  return content
    .filter((item) => item?.type === 'text' || typeof item === 'string')
    .map((item) => typeof item === 'string' ? item : item.text || '')
    .join('\n')
}
