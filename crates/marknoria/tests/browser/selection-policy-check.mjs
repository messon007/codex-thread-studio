import { runDomContract } from './dom-component-check.mjs'
const files = new Map([
  ...['notes.mjs', 'notes.css', 'translation.mjs', 'table-cells.mjs', 'table-cells.css', 'icons.mjs', 'clipboard.mjs', 'component-styles.mjs'].map(name => ['/' + name, 'web/' + name]),
  ['/selection-policy-contract.mjs', 'tests/browser/contracts/selection-policy.mjs']
])
console.log(JSON.stringify(await runDomContract({ files, contract: '/selection-policy-contract.mjs', entry: 'selectionPolicyContract', moduleUrl: '/notes.mjs' })))
