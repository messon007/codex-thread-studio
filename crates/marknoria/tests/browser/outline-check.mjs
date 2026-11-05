import { runDomContract } from './dom-component-check.mjs'
const files = new Map([
  ...['outline.mjs', 'pdf-outline.mjs', 'translation.mjs', 'styles.css', 'vendor/virtual-core.mjs'].map(name => ['/' + name, 'web/' + name]),
  ['/outline-contract.mjs', 'tests/browser/contracts/outline.mjs']
])
console.log(JSON.stringify(await runDomContract({ files, contract: '/outline-contract.mjs', entry: 'outlineContract', moduleUrl: '/outline.mjs' })))
