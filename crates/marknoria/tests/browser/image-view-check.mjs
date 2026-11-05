import { runDomContract } from './dom-component-check.mjs'
const files = new Map([
  ...[
    'image-view.mjs',
    'image-view.css',
    'image-frames.mjs',
    'assets.mjs',
    'component-styles.mjs',
    'translation.mjs'
  ].map((name) => ['/' + name, 'web/' + name]),
  ['/image-view-contract.mjs', 'tests/browser/contracts/image-view.mjs']
])
console.log(
  JSON.stringify(
    await runDomContract({
      files,
      contract: '/image-view-contract.mjs',
      entry: 'imageViewContract',
      moduleUrl: '/image-view.mjs'
    })
  )
)
