import { runDomContract } from './dom-component-check.mjs'
const files = new Map([
  ['/word-highlights.mjs', 'web/word-highlights.mjs'],
  ['/editor.mjs', 'web/vendor/editor.mjs'],
  ['/word-highlights-contract.mjs', 'tests/browser/contracts/word-highlights.mjs'],
])
console.log(JSON.stringify(await runDomContract({ files, contract: '/word-highlights-contract.mjs', entry: 'wordHighlightsContract', moduleUrl: '/word-highlights.mjs' })))
