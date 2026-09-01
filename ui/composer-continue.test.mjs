import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const app = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
const html = readFileSync(new URL('./index.html', import.meta.url), 'utf8')

test('composer provides a guarded one-step Continue action', () => {
  assert.match(html, /id="continue-thread"[^>]*title="Send a short continue prompt · Ctrl\/Cmd\+Shift\+Enter"[^>]*>Continue<\/button>/u)
  assert.match(app, /#continue-thread'\)\.addEventListener\('click', sendContinueMessage\)/u)
  assert.match(app, /event\.key === 'Enter' && event\.shiftKey && \(event\.ctrlKey \|\| event\.metaKey\)[\s\S]{0,160}sendContinueMessage\(\)/u)
  assert.match(app, /const continueAvailable = !active && !shellMode && !isRouterThread\(\)/u)
  assert.match(app, /#continue-thread'\)\.disabled = !state\.ready \|\| !state\.selectedId \|\| hasComposerContent \|\| Boolean\(queue\.length\)/u)
  assert.match(app, /function sendContinueMessage\(\)[\s\S]{0,260}setCurrentComposerValue\(randomContinuePrompt\(\)\)[\s\S]{0,120}requestSubmit\(\)/u)
})
