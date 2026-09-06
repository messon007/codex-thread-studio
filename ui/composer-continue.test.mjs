import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const app = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
const html = readFileSync(new URL('./index.html', import.meta.url), 'utf8')
const main = readFileSync(new URL('../src-tauri/src/main.rs', import.meta.url), 'utf8')
const ollama = readFileSync(new URL('../src-tauri/src/ollama.rs', import.meta.url), 'utf8')

test('Continue behavior is persisted and defaults to a human-reviewed session-model draft', () => {
  assert.match(html, /id="continue-thread"[^>]*title="Draft the next message with the current session model · Ctrl\/Cmd\+Shift\+Enter"[^>]*>Continue<\/button>/u)
  assert.match(html, /id="continue-behavior"[\s\S]{0,420}value="sessionModelDraft"[\s\S]{0,220}value="ollamaDraft"[\s\S]{0,220}value="quickSend"/u)
  assert.match(app, /continueBehavior: 'sessionModelDraft'/u)
  assert.match(app, /state\.continueBehavior = normalizeContinueBehavior\(saved\.continueBehavior\)/u)
  assert.match(readFileSync(new URL('./preferences-snapshot.mjs', import.meta.url), 'utf8'), /continueBehavior: state\.continueBehavior/u)
  assert.match(app, /#continue-behavior'\)\.value = state\.continueBehavior/u)
  assert.match(app, /state\.continueBehavior = normalizeContinueBehavior\(\$\('#continue-behavior'\)\.value\)/u)
  assert.match(app, /normalizeContinueBehavior[\s\S]*from '\.\/preference-normalization\.mjs'/u)
  assert.match(app, /#continue-thread'\)\.addEventListener\('click', handleContinueAction\)/u)
  assert.match(app, /event\.key === 'Enter' && event\.shiftKey && \(event\.ctrlKey \|\| event\.metaKey\)[\s\S]{0,160}handleContinueAction\(\)/u)
})

test('session-model Continue runs in an isolated hidden task and fills Chat without sending', () => {
  const start = app.indexOf('async function draftContinueWithSessionModel')
  const end = app.indexOf('\nfunction ensureContinuationBackend', start)
  const implementation = app.slice(start, end)
  assert.ok(start >= 0 && end > start)
  assert.match(app, /function handleContinueAction\(\)[\s\S]{0,220}state\.continueBehavior === 'quickSend'[\s\S]{0,160}draftContinueMessage\(\)/u)
  const runner = readFileSync(new URL('./hidden-utility-session.mjs', import.meta.url), 'utf8')
  assert.match(implementation, /runHiddenUtilitySession\(state,[\s\S]*instructions: CONTINUATION_DRAFT_INSTRUCTIONS/u)
  assert.match(runner, /ephemeral: true,[\s\S]*developerInstructions: options\.instructions/u)
  assert.match(implementation, /outputSchema: CONTINUATION_DRAFT_SCHEMA[\s\S]*parse: continuationDraftTurnState/u)
  assert.match(runner, /waitForUtilityResult/u)
  assert.match(implementation, /dispatchBackendRpc\(targetBackend, 'thread\/delete'/u)
  assert.match(app, /structuredUtilityTasks: new Map\(\)/u)
  assert.match(main, /"\/continuation-draft\.mjs", get\(continuation_draft_js\)/u)
  assert.match(app, /setComposerDraftValue\(key, prompt\)[\s\S]{0,180}input\.focus\(\)/u)
  assert.doesNotMatch(app, /function draftContinueMessage\(\)[\s\S]{0,1200}requestSubmit\(\)/u)
})

test('Ollama Continue remains an optional local draft mode', () => {
  assert.match(app, /behavior === 'ollamaDraft'[\s\S]{0,100}draftContinueWithOllama\(source\)/u)
  assert.match(app, /gatewayFetch\('\/studio\/ollama\/continue-draft',[\s\S]{0,320}model: state\.translation\.ollamaModel \|\| 'gemma3:4b',[\s\S]{0,120}assistantResponse: truncateCharacters\(source, 32_000\)/u)
  assert.match(html, /id="translation-ollama-model-field" class="field full"[\s\S]{0,500}Lists every model managed by local Ollama\./u)
})

test('Quick send mode submits one randomized short Continue prompt', () => {
  assert.match(app, /function quickSendContinueMessage\(\)[\s\S]{0,320}setCurrentComposerValue\(randomContinuePrompt\(\)\)[\s\S]{0,120}requestSubmit\(\)/u)
  assert.match(app, /state\.continueBehavior === 'quickSend'[\s\S]{0,120}quickSendContinueMessage\(\)/u)
})

test('Continue discards drafts made stale by session, turn, reply, or composer changes', () => {
  assert.match(app, /state\.continuationDraftLoads\.add\(key\)/u)
  assert.match(app, /if \(selectedStateKey\(\) !== key\) return/u)
  assert.match(app, /state\.model\.activeTurnId \|\| latestAgentResponseText\(\)\.trim\(\) !== source \|\| composerHasPendingContent\(key\)/u)
  assert.match(app, /state\.continuationDraftLoads\.delete\(key\)/u)
})

test('the gateway registers a guarded local-only Ollama continuation route', () => {
  assert.match(main, /"\/studio\/ollama\/continue-draft",\s*axum::routing::post\(ollama::continue_draft\)/u)
  assert.match(ollama, /const OLLAMA_ORIGIN: &str = "http:\/\/127\.0\.0\.1:11434"/u)
  assert.match(ollama, /pub async fn continue_draft\(body: String\)/u)
  assert.match(ollama, /MAX_ASSISTANT_RESPONSE_CHARS: usize = 32_000/u)
  assert.match(ollama, /CONTINUATION_INSTRUCTIONS[\s\S]{0,900}untrusted quoted data/u)
})
