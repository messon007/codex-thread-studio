import { runDomContract } from './dom-component-check.mjs'
const files = new Map([
  ...[
    'jq-query.mjs', 'query-help.mjs',
    'jq-query.css',
    'translation.mjs',
    'query-operation.mjs',
    'icons.mjs',
    'clipboard.mjs',
    'component-styles.mjs',
    'table-query.css'
  ].map((name) => ['/' + name, 'web/' + name]),
  ['/jq-contract.mjs', 'tests/browser/contracts/jq-query.mjs']
])
console.log(
  JSON.stringify(
    await runDomContract({
      files,
      contract: '/jq-contract.mjs',
      entry: 'jqQueryContract',
      moduleUrl: '/jq-query.mjs'
    })
  )
)
