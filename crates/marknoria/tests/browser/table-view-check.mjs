import { runDomContract } from './dom-component-check.mjs'
const files = new Map([
  ['/comment-ranges.mjs', 'web/comment-ranges.mjs'],
  ...[
    'table-view.mjs',
    'table-view.css',
    'table-controls.mjs','translation.mjs',
    'table-layout.mjs',
    'table-cells.mjs',
    'table-cells.css',
    'icons.mjs',
    'clipboard.mjs',
    'component-styles.mjs'
  ].map((name) => ['/' + name, 'web/' + name]),
  ['/vendor/purify.mjs', 'web/vendor/purify.mjs'],
  ['/view-contract.mjs', 'tests/browser/contracts/table-view.mjs']
])
console.log(
  JSON.stringify(
    await runDomContract({
      files,
      contract: '/view-contract.mjs',
      entry: 'tableViewContract',
      moduleUrl: '/table-view.mjs'
    })
  )
)
