import { runDomContract } from './dom-component-check.mjs'
const files=new Map([['/files.mjs','web/files.mjs'],['/translation.mjs','web/translation.mjs'],['/contract-file-ui.mjs','tests/browser/contracts/file-ui.mjs']])
console.log(JSON.stringify(await runDomContract({files,contract:'/contract-file-ui.mjs',entry:'fileUiContract',moduleUrl:'/files.mjs'})))
