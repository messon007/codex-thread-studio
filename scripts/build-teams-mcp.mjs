// Build the GUI-independent bridge and optionally stage Tauri's target-suffixed sidecar.
import {spawnSync} from 'node:child_process'
import {mkdirSync, copyFileSync} from 'node:fs'
import {dirname, resolve, join} from 'node:path'
import {fileURLToPath} from 'node:url'
const root=resolve(dirname(fileURLToPath(import.meta.url)),'..')
const args=process.argv.slice(2)
let target='',release=false,bundle=false
for(let i=0;i<args.length;i++){
  if(args[i]==='--release')release=true
  else if(args[i]==='--bundle')bundle=true
  else if(args[i]==='--target'&&args[i+1])target=args[++i]
  else throw Error(`Unknown or incomplete argument: ${args[i]}`)
}
function run(command,args){const r=spawnSync(command,args,{cwd:root,encoding:'utf8',stdio:['ignore','pipe','inherit']});if(r.error)throw r.error;if(r.status!==0)process.exit(r.status||1);return r.stdout}
const host=run('rustc',['-vV']).match(/^host: (.+)$/m)?.[1]
if(!host)throw Error('Cannot determine Rust host triple')
const build=['build','--locked','-p','studio-teams-mcp',...(release?['--release']:[]),...(target?['--target',target]:[])]
process.stdout.write(run('cargo',build))
if(bundle){
 const output=process.env.CARGO_TARGET_DIR || JSON.parse(run('cargo',['metadata','--format-version','1','--no-deps'])).target_directory
 const triple=target||host, ext=triple.includes('windows')?'.exe':''
 const source=resolve(root,output,...(target?[target]:[]),release?'release':'debug',`studio-teams-mcp${ext}`)
 const directory=join(root,'src-tauri/binaries');mkdirSync(directory,{recursive:true})
 copyFileSync(source,join(directory,`studio-teams-mcp-${triple}${ext}`))
 console.log(`Staged Teams MCP companion for ${triple}`)
}
