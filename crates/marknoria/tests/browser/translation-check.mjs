import { runDomContract } from './dom-component-check.mjs'
console.log(
  JSON.stringify(
    await runDomContract({
      files: new Map([
        ['/translation.mjs', 'web/translation.mjs'],
        ['/ui-messages.mjs', 'web/ui-messages.mjs'],
        ['/translation-contract.mjs', 'tests/browser/contracts/translation.mjs']
      ]),
      contract: '/translation-contract.mjs',
      entry: 'translationContract',
      moduleUrl: '/translation.mjs'
    })
  )
)
