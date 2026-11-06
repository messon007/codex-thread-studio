import { runDomContract } from './dom-component-check.mjs'
const files = new Map([
  ['/comment-ranges.mjs', 'web/comment-ranges.mjs'],
  ...[
    'projection-view.mjs',
    'projection-view.css',
    'document-prose.css',
    'projection-html.mjs',
    'table-view.mjs',
    'table-view.css',
    'table-controls.mjs',
    'table-layout.mjs',
    'table-cells.mjs',
    'table-cells.css',
    'translation.mjs',
    'icons.mjs',
    'clipboard.mjs',
    'component-styles.mjs'
  ].map((name) => ['/' + name, 'web/' + name]),
  ['/vendor/purify.mjs', 'web/vendor/purify.mjs'],
  ['/projection-contract.mjs', 'tests/browser/contracts/projection-view.mjs']
])
console.log(
  JSON.stringify(
    await runDomContract({
      files,
      contract: '/projection-contract.mjs',
      entry: 'projectionViewContract',
      moduleUrl: '/projection-view.mjs'
    })
  )
)
