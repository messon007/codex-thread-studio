import { runDomContract } from './dom-component-check.mjs'
const files = new Map([
  ...['ui-messages.mjs','source-comment-marks.mjs','vendor/editor.mjs','comment-ranges.mjs','comment-marks.mjs','table-cells.mjs','table-cells.css','icons.mjs','clipboard.mjs','component-styles.mjs','translation.mjs'].map(name=>['/'+name,'web/'+name]),
  ['/comment-marks-contract.mjs','tests/browser/contracts/comment-marks.mjs']
])
console.log(JSON.stringify(await runDomContract({files,contract:'/comment-marks-contract.mjs',entry:'commentMarksContract',moduleUrl:'/comment-marks.mjs'+(process.argv.includes('--fallback')?'#fallback':'')})))
