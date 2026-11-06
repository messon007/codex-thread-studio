import { runDomContract } from './dom-component-check.mjs'
const files = new Map([
  ...[
    'table-query.mjs', 'query-help.mjs',
    'table-query.css',
    'table-view.css',
    'table-layout.mjs',
    'table-controls.mjs','translation.mjs',
    'table-cells.mjs',
    'table-cells.css',
    'icons.mjs',
    'clipboard.mjs',
    'component-styles.mjs',
    'table-plot.mjs',
    'table-plot.css',
    'query-operation.mjs'
  ].map((name) => ['/' + name, 'web/' + name]),
  ['/query-contract.mjs', 'tests/browser/contracts/table-query.mjs']
])
console.log(
  JSON.stringify(
    await runDomContract({
      files,
      contract: '/query-contract.mjs',
      entry: 'tableQueryContract',
      moduleUrl: '/table-query.mjs'
    })
  )
)
