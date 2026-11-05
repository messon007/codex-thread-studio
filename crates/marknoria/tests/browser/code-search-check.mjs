import {runDomContract} from './dom-component-check.mjs'
const files=new Map([['/editor.mjs','web/vendor/editor.mjs'],['/translation.mjs','web/translation.mjs'],['/code-search-contract.mjs','tests/browser/contracts/code-search.mjs']])
console.log(JSON.stringify(await runDomContract({files,contract:'/code-search-contract.mjs',entry:'codeSearchContract',moduleUrl:'/editor.mjs'})))
