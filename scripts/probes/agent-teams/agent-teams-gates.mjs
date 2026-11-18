// Isolated capability probe, not a Teams implementation. Synthetic model only.
import fs from 'node:fs/promises'
import path from 'node:path'
import os from 'node:os'
import http from 'node:http'
import net from 'node:net'
import crypto from 'node:crypto'
import { spawn } from 'node:child_process'
import { createRequire } from 'node:module'
const require = createRequire(import.meta.url)
const WebSocket = require('/media/rui/codex-thread-studio-agent-teams/validation/probe-deps/node_modules/ws')
const run = process.argv[2]
if (!run?.startsWith('/media/rui/codex-thread-studio-agent-teams/validation/')) throw Error('Dedicated media run directory required')
const configRoot = process.env.CODEX_HOME || path.join(os.homedir(), '.codex')
if ((await fs.readFile(path.join(configRoot, '.teams-probe-fixture'), 'utf8')).trim() !== run) throw Error('Missing isolated config mount: refusing to run')
const probeMode = process.argv[3] || 'original'
const report = { probeMode, version: '0.170.0', syntheticModel: true, liveLlmTested: false, checks: [], errors: [], threads: [] }
const log = async (name, value) => fs.appendFile(path.join(run, name), JSON.stringify(value) + '\n')
const pause = ms => new Promise(r => setTimeout(r, ms))
async function until(fn, timeout = 12000) { const end = Date.now() + timeout; while (Date.now() < end) { const value = await fn(); if (value) return value; await pause(40) } throw Error('Condition timeout') }
let modelMode = 'text', holds = [], modelCount = 0
const modelServer = http.createServer(async (req, res) => {
  if (req.method !== 'POST') { res.writeHead(200, {'content-type':'application/json'}); res.end(JSON.stringify({data:[]})); return }
  let body = ''; for await (const b of req) body += b
  let request; try { request = JSON.parse(body) } catch { request = {unparsed:body} }
  const index = ++modelCount, mode = modelMode; modelMode = 'text'
  await log('model-requests.jsonl', {index, mode, path:req.url, request})
  res.writeHead(200, {'content-type':'text/event-stream', 'cache-control':'no-cache'})
  const emit = event => res.write(`data: ${JSON.stringify(event)}\n\n`)
  emit({type:'response.created', response:{id:`resp_${index}`,object:'response',status:'in_progress'}})
  const finish = () => {
    const item = mode === 'tool' ? {type:'function_call',id:`fc_${index}`,call_id:`native_probe_${index}`,name:'echo',namespace:'mcp__teams_probe',arguments:JSON.stringify({value:'fixture-only'})} : {type:'message',id:`msg_${index}`,role:'assistant',status:'completed',content:[{type:'output_text',text:`Synthetic fixture response ${index}`,annotations:[]}]}
    emit({type:'response.output_item.added',output_index:0,item})
    emit({type:'response.output_item.done',output_index:0,item})
    emit({type:'response.completed',response:{id:`resp_${index}`,object:'response',status:'completed',output:[item],usage:{input_tokens:1,output_tokens:1,total_tokens:2}}})
    res.end()
  }
  if (mode === 'hold') holds.push(finish); else finish()
})
await new Promise(r=>modelServer.listen(0,'127.0.0.1',r))
const modelPort = modelServer.address().port
const configBase = `model = "gpt-5.1-codex"\nmodel_provider = "teams_fixture"\napproval_policy = "never"\nsandbox_mode = "read-only"\n[features]\napps = false\nplugins = false\nhooks = false\ndaemon_auto_start = false\ncode_mode_host = false\nshell_snapshot = false\nenable_request_compression = false\n[analytics]\nenabled = false\n[model_providers.teams_fixture]\nname = "Isolated synthetic provider"\nbase_url = "http://127.0.0.1:${modelPort}/v1"\nwire_api = "responses"\nrequires_openai_auth = false\nsupports_websockets = false\n`
await fs.writeFile(path.join(configRoot,'config.toml'), configBase)
const listener = net.createServer(); await new Promise(r=>listener.listen(0,'127.0.0.1',r)); const serverPort=listener.address().port; await new Promise(r=>listener.close(r))
const token = crypto.randomBytes(24).toString('hex'), tokenPath=path.join(run,'fixture-ws-token')
await fs.writeFile(tokenPath,token,{mode:0o600})
const childEnv = {PATH:process.env.PATH, HOME:process.env.HOME, LANG:'C.UTF-8', RUST_LOG:'warn'}
if (process.env.CODEX_HOME) childEnv.CODEX_HOME=process.env.CODEX_HOME
const child = spawn('codex',['app-server','--listen',`ws://127.0.0.1:${serverPort}`,'--ws-auth','capability-token','--ws-token-file',tokenPath],{env:childEnv,cwd:run,stdio:['ignore','pipe','pipe']})
child.stdout.on('data',b=>fs.appendFile(path.join(run,'app-server.stdout'),b))
child.stderr.on('data',b=>fs.appendFile(path.join(run,'app-server.stderr'),b))
const clients=[]
class Client {
  constructor(ws,name){ this.ws=ws; this.name=name; this.next=1; this.pending=new Map(); this.events=[]; ws.on('message',raw=>{const msg=JSON.parse(raw.toString()); log('rpc.jsonl',{client:name,direction:'receive',message:msg}); if(msg.id!=null && this.pending.has(msg.id) && !msg.method){const {resolve,reject,timer}=this.pending.get(msg.id);clearTimeout(timer);this.pending.delete(msg.id);msg.error?reject(Object.assign(Error(msg.error.message),{rpcError:msg.error})):resolve(msg.result)}else{this.events.push(msg);if(msg.id!=null&&msg.method){ws.send(JSON.stringify({id:msg.id,error:{code:-32601,message:'Probe does not grant approvals'}}))}}}) }
  call(method,params,timeout=15000){ const id=this.next++;log('rpc.jsonl',{client:this.name,direction:'send',message:{id,method,params}});return new Promise((resolve,reject)=>{const timer=setTimeout(()=>{this.pending.delete(id);reject(Error(`RPC timeout: ${method}`))},timeout);this.pending.set(id,{resolve,reject,timer});this.ws.send(JSON.stringify({id,method,params}))}) }
  note(method,params){this.ws.send(JSON.stringify({method,params}))}
}
async function connect(name){ const ws=await until(()=>new Promise(resolve=>{const sock=new WebSocket(`ws://127.0.0.1:${serverPort}`,{headers:{Authorization:`Bearer ${token}`}});sock.once('open',()=>resolve(sock));sock.once('error',()=>{sock.terminate();resolve(null)})}));const c=new Client(ws,name);clients.push(c);await c.call('initialize',{clientInfo:{name:`teams_${name}`,version:'0.1'},capabilities:{experimentalApi:true}});c.note('initialized',{});return c }
async function turn(c,id,text){const start=await c.call('turn/start',{threadId:id,input:[{type:'text',text}]});await until(()=>c.events.find(e=>e.method==='turn/completed'&&e.params?.turn?.id===start.turn.id));return start}
const addCheck=(name,data)=>{report.checks.push({name,...data});process.stdout.write(`${name}: recorded\n`)}
try {
 const c1=await connect('one'),c2=await connect('two')
 for(const name of (probeMode==='thread-config'?['A','B','C']:['A','B'])){const cwd=path.join(run,`workspace-${name}`);await fs.mkdir(cwd,{recursive:true});const response=await c1.call('thread/start',{cwd,model:'gpt-5.1-codex',modelProvider:'teams_fixture',approvalPolicy:'never',sandbox:'read-only'});report.threads.push(response.thread.id);await turn(c1,response.thread.id,`Original fixture context ${name}; do not execute shell commands.`)}
 const [a,b]=report.threads
 if (probeMode==='managed-config') {
   const cfg=await c1.call('config/read',{includeLayers:true})
   const user=cfg.layers.find(l=>l.name.type==='user'&&!l.name.profile)
   if(!user)throw Error('No writable user config layer')
   addCheck('managed_config_scope',{name:user.name,version:user.version})
   const helper=path.join(path.dirname(new URL(import.meta.url).pathname),'agent-teams-echo.py')
   const callsFile=path.join(run,'mcp-calls.jsonl')
   const registration={command:'/usr/bin/python3',args:[helper,callsFile],required:false,startup_timeout_sec:5,tool_timeout_sec:10}
   const installed=await c1.call('config/batchWrite',{filePath:user.name.file,expectedVersion:user.version,reloadUserConfig:false,edits:[{keyPath:'mcp_servers.teams_probe',value:registration,mergeStrategy:'replace'}]})
   addCheck('managed_config_install',{installed})
   await c1.call('config/mcpServer/reload',null)
   const inventory=await until(async()=>{const v=await c1.call('mcpServerStatus/list',{threadId:a,detail:'toolsAndAuthOnly'});return JSON.stringify(v).includes('echo')&&v})
   modelMode='tool';await turn(c1,a,'Use the echo tool in this existing synthetic thread')
   addCheck('loaded_thread_managed_tool',{inventory,calls:(await fs.readFile(callsFile,'utf8')).trim().split('\n').filter(Boolean).map(JSON.parse)})
   const beforeDelete=await c1.call('config/read',{includeLayers:true});const layer=beforeDelete.layers.find(l=>l.name.type==='user'&&!l.name.profile)
   let stale
   try{await c1.call('config/batchWrite',{filePath:user.name.file,expectedVersion:user.version,reloadUserConfig:false,edits:[{keyPath:'mcp_servers.teams_probe',value:null,mergeStrategy:'replace'}]});stale='UNEXPECTED_ACCEPT'}catch(e){stale=e.rpcError??e.message}
   let removed
   try{removed=await c1.call('config/batchWrite',{filePath:user.name.file,expectedVersion:layer.version,reloadUserConfig:false,edits:[{keyPath:'mcp_servers.teams_probe',value:null,mergeStrategy:'replace'}]})}catch(e){removed={error:e.rpcError??e.message}}
   const after=await c1.call('config/read',{includeLayers:true})
   addCheck('managed_config_precise_delete',{stale,removed,remaining:after.config.mcp_servers??null,globalConfigText:await fs.readFile(path.join(configRoot,'config.toml'),'utf8')})
   report.verdict='PARTIAL_MANAGED_CONFIG_EVIDENCE: synthetic fixture only'
 } else if (probeMode==='thread-config') {
   const helper=path.join(path.dirname(new URL(import.meta.url).pathname),'agent-teams-echo.py')
   const callsFile=path.join(run,'mcp-calls.jsonl')
   const baseline=await c1.call('thread/read',{threadId:a,includeTurns:true})
   const resumed=await c1.call('thread/resume',{threadId:a,config:{mcp_servers:{teams_probe:{command:'/usr/bin/python3',args:[helper,callsFile],required:false,startup_timeout_sec:5,tool_timeout_sec:10}}}})
   addCheck('resume_with_member_config',{response:resumed})
   await pause(500)
   const inventories=[]
   for(const id of report.threads) inventories.push({thread:id,result:await c1.call('mcpServerStatus/list',{threadId:id,detail:'toolsAndAuthOnly'})})
   addCheck('thread_specific_inventory',{inventories})
   let available=JSON.stringify(inventories[0].result).includes('echo')
   if(!available){
     const unsubscribe=await c1.call('thread/unsubscribe',{threadId:a})
     await until(async()=>!JSON.stringify(await c1.call('thread/loaded/list',{})).includes(a))
     const reattached=await c1.call('thread/resume',{threadId:a,config:{mcp_servers:{teams_probe:{command:'/usr/bin/python3',args:[helper,callsFile],required:false,startup_timeout_sec:5,tool_timeout_sec:10}}}})
     await pause(500)
     const inventory=await c1.call('mcpServerStatus/list',{threadId:a,detail:'toolsAndAuthOnly'})
     available=JSON.stringify(inventory).includes('echo')
     addCheck('same_thread_detach_and_resume_config',{unsubscribe,reattached,inventory})
   }
   if(available){modelMode='tool';await turn(c1,a,'Synthetic native tool invocation after per-thread configuration');addCheck('native_echo_after_per_thread_config',{calls:(await fs.readFile(callsFile,'utf8')).trim().split('\n').filter(Boolean).map(JSON.parse)})}
   const after=await c1.call('thread/read',{threadId:a,includeTurns:true})
   addCheck('original_context_and_global_config',{baseline,after,globalConfigUnchanged:(await fs.readFile(path.join(configRoot,'config.toml'),'utf8'))===configBase})
   // Independent G2b samples: one queue/add, no queue/start/turn/start for dispatch.
   const idleOffset=c1.events.length
   const idle=await c1.call('thread/queue/add',{threadId:b,clientUserMessageId:'idle-add-once',input:[{type:'text',text:'Idle queued fixture'}]})
   let idleCompleted=false
   try{await until(()=>c1.events.slice(idleOffset).find(e=>e.method==='turn/completed'&&e.params.threadId===b),2000);idleCompleted=true}catch{}
   addCheck('idle_queue_add_once',{response:idle,idleCompleted,list:await c1.call('thread/queue/list',{threadId:b}),events:c1.events.slice(idleOffset)})
   modelMode='hold';const startCount=modelCount
   const busy=await c1.call('turn/start',{threadId:a,input:[{type:'text',text:'Existing synthetic work, held before queue admission'}]})
   await until(()=>modelCount>startCount&&holds.length)
   const busyOffset=c1.events.length
   const kept=await c2.call('thread/queue/add',{threadId:a,clientUserMessageId:'keep-once',input:[{type:'text',text:'Execute only after original task ends'}]})
   const withdrawn=await c2.call('thread/queue/add',{threadId:a,clientUserMessageId:'withdraw-once',input:[{type:'text',text:'This fixture must be deleted before original turn ends'}]})
   const deletion=await c2.call('thread/queue/delete',{threadId:a,queuedSubmissionId:withdrawn.queuedSubmission.id})
   await pause(200)
   const duringBusy=c1.events.slice(busyOffset)
   for(const finish of holds.splice(0))finish()
   await until(()=>c1.events.slice(busyOffset).some(e=>e.method==='turn/completed'&&e.params.threadId===a&&e.params.turn.id!==busy.turn.id))
   await pause(150)
   addCheck('busy_add_once_and_precise_delete',{busyTurn:busy.turn.id,kept,withdrawn,deletion,duringBusy,afterCompletion:c1.events.slice(busyOffset),list:await c1.call('thread/queue/list',{threadId:a})})
   report.verdict='PARTIAL_G1B_G2B_EVIDENCE: no live model or product adapter tested'
 } else {
 await c2.call('thread/resume',{threadId:a,excludeTurns:true})
 modelMode='hold';const beforeModel=modelCount
 const busy=await c1.call('turn/start',{threadId:a,input:[{type:'text',text:'Held fixture turn: keep current work active.'}]})
 await until(()=>modelCount>beforeModel&&holds.length)
 const queueInput={threadId:a,clientUserMessageId:'teams-probe-stable-delivery',input:[{type:'text',text:'Queued fixture only; never interrupt prior work.'}]}
 const q=await c1.call('thread/queue/add',queueInput)
 const qAgain=await c1.call('thread/queue/add',queueInput)
 const list=await c1.call('thread/queue/list',{threadId:a})
 addCheck('queue_duplicate_add',{first:q,second:qAgain,list})
 const eventOffset=c1.events.length
 let busyStart
 try{busyStart={accepted:await c2.call('thread/queue/start',{threadId:a,queuedSubmissionId:q.queuedSubmission.id},3500)}}catch(e){busyStart={error:e.rpcError??e.message}}
 addCheck('queue_start_while_busy',{busyTurn:busy.turn.id,outcome:busyStart,events:c1.events.slice(eventOffset)})
 for(const finish of holds.splice(0))finish()
 await until(()=>c1.events.find(e=>e.method==='turn/completed'&&e.params?.turn?.id===busy.turn.id))
 await pause(350)
 const remaining=await c1.call('thread/queue/list',{threadId:a});addCheck('queue_after_prior_turn_completed',{list:remaining})
 for(const entry of remaining.data??[]){await c1.call('thread/queue/delete',{threadId:a,queuedSubmissionId:entry.id})}
 // The two threads existed before MCP registration. No user configuration used.
 const helper=path.join(path.dirname(new URL(import.meta.url).pathname),'agent-teams-echo.py')
 const callsFile=path.join(run,'mcp-calls.jsonl')
 const mcpConfig=`\n[mcp_servers.teams_probe]\ncommand = "/usr/bin/python3"\nargs = [${JSON.stringify(helper)}, ${JSON.stringify(callsFile)}]\nrequired = false\nstartup_timeout_sec = 5\ntool_timeout_sec = 10\n`
 await fs.writeFile(path.join(configRoot,'config.toml'),configBase+mcpConfig)
 await c1.call('config/mcpServer/reload',null)
 const inventory=await until(async()=>{const v=await c1.call('mcpServerStatus/list',{threadId:a,serverName:'teams_probe',detail:'toolsAndAuthOnly'});return JSON.stringify(v).includes('echo')&&v},15000)
 addCheck('existing_thread_mcp_reload',{inventory})
 const direct=await c1.call('mcpServer/tool/call',{threadId:a,server:'teams_probe',tool:'echo',arguments:{value:'fixture-only'},_meta:{threadId:a,sessionId:a,callId:'forged-direct-only'}})
 addCheck('host_direct_call_supplied_metadata',{result:direct})
 const priorCalls=(await fs.readFile(callsFile,'utf8')).trim().split('\n').length
 modelMode='tool'
 const native=await c1.call('turn/start',{threadId:a,input:[{type:'text',text:'Synthetic provider will issue the echo tool via native model response processing.'}]})
 const nativeCall=await until(async()=>{const entries=(await fs.readFile(callsFile,'utf8')).trim().split('\n').filter(Boolean).map(JSON.parse);return entries.length>priorCalls&&entries.at(-1)},15000)
 const offset=c1.events.length
 let overlap
 try{overlap=await c2.call('mcpServer/tool/call',{threadId:a,server:'teams_probe',tool:'echo',arguments:nativeCall.params.arguments,_meta:nativeCall.params._meta})}catch(e){overlap={error:e.rpcError??e.message}}
 await until(()=>c1.events.find(e=>e.method==='turn/completed'&&e.params?.turn?.id===native.turn.id),15000)
 addCheck('native_and_overlapping_direct_call',{nativeCall,overlap,events:c1.events.slice(offset)})
 const calls=(await fs.readFile(callsFile,'utf8')).trim().split('\n').filter(Boolean).map(JSON.parse)
 const last=calls.slice(priorCalls)
 const comparable = p => { const {_meta, ...rest} = p; const {progressToken, ...identity} = _meta ?? {}; return {...rest, _meta:identity} }
 const identical=last.length>=2&&JSON.stringify(comparable(last[0].params))===JSON.stringify(comparable(last[1].params))
 addCheck('mcp_receiver_can_distinguish_origin',{identicalIdentityAndArguments:identical,calls:last,verdict:identical?'FAIL for identity-content matching: all metadata except progressToken can be copied by an authenticated direct caller; origin binding remains unproven':'UNKNOWN: further correlation proof required'})
 for(const id of report.threads){const read=await c1.call('thread/read',{threadId:id});await log('thread-metadata.jsonl',read)}
 report.verdict='NOT_APPROVED_FOR_PRODUCT: probes are partial capability evidence only'
 }
}catch(e){report.errors.push({message:e.message,rpcError:e.rpcError});report.verdict='INCOMPLETE';process.stdout.write(`Probe stopped: ${e.stack}\n`)}finally{
 for(const finish of holds.splice(0))finish()
 for(const c of clients)c.ws.close()
 child.kill('SIGTERM'); await pause(200);if(child.exitCode==null)child.kill('SIGKILL')
 modelServer.closeAllConnections();await new Promise(r=>modelServer.close(r))
 await fs.writeFile(path.join(run,'report.json'),JSON.stringify(report,null,2)+'\n')
}
