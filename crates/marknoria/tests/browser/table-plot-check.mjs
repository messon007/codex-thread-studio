import { runDomContract } from './dom-component-check.mjs'
const files = new Map([
  ['/translation.mjs', 'web/translation.mjs'],
  ['/table-plot.mjs', 'web/table-plot.mjs'],
  ['/table-plot.css', 'web/table-plot.css'],
  ['/icons.mjs', 'web/icons.mjs'],
  ['/query-operation.mjs', 'web/query-operation.mjs'],
  ['/component-styles.mjs', 'web/component-styles.mjs'],
  ['/plot-contract.mjs', 'tests/browser/contracts/table-plot.mjs']
])
console.log(
  JSON.stringify(
    await runDomContract({
      files,
      contract: '/plot-contract.mjs',
      entry: 'tablePlotContract',
      moduleUrl: '/table-plot.mjs'
    })
  )
)
