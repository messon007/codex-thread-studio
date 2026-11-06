import { runDomContract } from './dom-component-check.mjs'
const files = new Map([
  ['/comment-ranges.mjs', 'web/comment-ranges.mjs'],
  ['/translation.mjs', 'web/translation.mjs'],
  ['/table-cells.mjs', 'web/table-cells.mjs'],
  ['/table-cells.css', 'web/table-cells.css'],
  ['/icons.mjs', 'web/icons.mjs'],
  ['/clipboard.mjs', 'web/clipboard.mjs'],
  ['/component-styles.mjs', 'web/component-styles.mjs'],
  ['/cells-contract.mjs', 'tests/browser/contracts/table-cells.mjs']
])
console.log(
  JSON.stringify(
    await runDomContract({
      files,
      contract: '/cells-contract.mjs',
      entry: 'tableCellsContract',
      moduleUrl: '/table-cells.mjs'
    })
  )
)
