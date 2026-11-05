import { runDomContract } from './dom-component-check.mjs'
console.log(
  JSON.stringify(
    await runDomContract({
      files: new Map([
        ...['table-controls.mjs', 'icons.mjs', 'translation.mjs'].map((name) => [
          '/' + name,
          'web/' + name
        ]),
        ['/table-controls-contract.mjs', 'tests/browser/contracts/table-controls.mjs']
      ]),
      contract: '/table-controls-contract.mjs',
      entry: 'tableControlsContract',
      moduleUrl: '/table-controls.mjs'
    })
  )
)
