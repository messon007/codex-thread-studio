import { TranscriptPresentationCache, presentTurn } from '../ui-src/transcript-presentation.mjs'
import { transcriptRestorePlan } from '../ui-src/transcript-scroll.mjs'
import { createTranscriptDom } from '../ui-src/transcript-dom.mjs'
const cache = new TranscriptPresentationCache()
const entry = cache.get('codex:one', { turns: [{ id: 'one', status: 'completed', items: [] }] })
const mode: 'latest' | 'fixed' = entry.windowMode
void mode
// @ts-expect-error Saved pixel offsets are numeric.
cache.setScrollState('codex:one', { scrollTop: '100' })
const plan = transcriptRestorePlan({ saved: { anchorTurnId: 'one' }, orderedTurnIds: ['one'] })
if (plan.type === 'anchor') {
  const id: string = plan.anchorTurnId
  void id
  // @ts-expect-error Anchor restoration is distinct from pixel restoration.
  plan.scrollTop
}
const presentation = presentTurn({ id: 'one', items: [] })
for (const block of presentation.blocks) {
  if (block.type === 'activity') {
    const count: number = block.summary.commands
    void count
  }
}
declare const container: HTMLElement
// @ts-expect-error DOM chunks must contain HTML strings.
createTranscriptDom().render(container, 'one', [{ id: 'one', html: 10 }])
