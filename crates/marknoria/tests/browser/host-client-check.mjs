import { runDomContract } from './dom-component-check.mjs'
console.log(JSON.stringify(await runDomContract({
  files: new Map([
    ['/host-client.mjs','web/host-client.mjs'],
    ['/reader-bridge.mjs','web/reader-bridge.mjs'],
    ['/host-client-contract.mjs','tests/browser/contracts/host-client.mjs'],
  ]), contract:'/host-client-contract.mjs', entry:'hostClientContract', moduleUrl:'/host-client.mjs',
})))
