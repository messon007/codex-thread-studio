import type { RouterCandidate } from './router-types.mjs'

/** Fast local suggestions, never an authorization to dispatch. */
export function rankRouterTargets(candidates: readonly RouterCandidate[], query: string, prompt = ''): RouterCandidate[] {
  const needle = query.trim().toLocaleLowerCase()
  const terms = (prompt.toLocaleLowerCase().match(/[\p{L}\p{N}_-]+/gu) || []).flatMap(word =>
    /[\u3400-\u9fff]/u.test(word) ? Array.from({ length: Math.max(0, word.length - 1) }, (_, i) => word.slice(i, i + 2)) : [word],
  ).filter(word => word.length > 1)
  return candidates.map((candidate, index) => {
    const title = candidate.title.toLocaleLowerCase()
    const metadata = `${title} ${candidate.cwd} ${candidate.responsibility} ${candidate.openingMessage}`.toLocaleLowerCase()
    let offset = 0
    const matches = !needle || metadata.includes(needle) || [...needle].every(char => {
      const at = title.indexOf(char, offset)
      if (at < 0) return false
      offset = at + 1
      return true
    })
    const score = (needle && title.includes(needle) ? 100 : 0) + terms.reduce((sum, term) => sum + (title.includes(term) ? 5 : metadata.includes(term) ? 1 : 0), 0)
    return { candidate, index, matches, score }
  }).filter(entry => entry.matches).sort((a, b) => b.score - a.score || a.index - b.index).map(entry => entry.candidate)
}

export function routerMention(value: string, cursor: number) {
  const prefix = value.slice(0, cursor)
  const match = /(?:^|\s)@([^\s@]*)$/u.exec(prefix)
  if (!match) return null
  return { start: cursor - match[1]!.length - 1, end: cursor, query: match[1]! }
}
