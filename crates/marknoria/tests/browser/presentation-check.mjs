import { runDomContract } from './dom-component-check.mjs'
const files = new Map([
  ...['presentation.mjs', 'reader-ui.mjs', 'translation.mjs'].map((name) => [
    '/' + name,
    'web/' + name
  ]),
  ['/presentation-contract.mjs', 'tests/browser/contracts/presentation.mjs']
])
console.log(
  JSON.stringify(
    await runDomContract({
      files,
      contract: '/presentation-contract.mjs',
      entry: 'presentationContract',
      moduleUrl: '/presentation.mjs'
    })
  )
)
