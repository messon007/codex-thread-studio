import { runDomContract } from './dom-component-check.mjs'
const files = new Map([
  ['/table-layout.mjs', 'web/table-layout.mjs'],
  ['/translation.mjs', 'web/translation.mjs'],
  ['/layout-contract.mjs', 'tests/browser/contracts/table-layout.mjs']
])
console.log(
  JSON.stringify(
    await runDomContract({
      files,
      contract: '/layout-contract.mjs',
      entry: 'tableComponentContract',
      moduleUrl: '/table-layout.mjs'
    })
  )
)
