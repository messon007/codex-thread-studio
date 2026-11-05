import { runDomContract } from './dom-component-check.mjs'
const files = new Map([
  ...['notes.mjs', 'translation.mjs', 'notes.css', 'icons.mjs', 'clipboard.mjs', 'component-styles.mjs'].map(
    (name) => ['/' + name, 'web/' + name]
  ),
  ['/notes-contract.mjs', 'tests/browser/contracts/notes.mjs']
])
console.log(
  JSON.stringify(
    await runDomContract({
      files,
      contract: '/notes-contract.mjs',
      entry: 'notesContract',
      moduleUrl: '/notes.mjs'
    })
  )
)
