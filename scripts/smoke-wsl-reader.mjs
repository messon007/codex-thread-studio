// Linux transport substitute using the exact Studio WSL script/authority adapter.
// This is not a claim of running Windows or a real WSL distribution.
import assert from 'node:assert/strict'
import {spawn} from 'node:child_process'
import {once} from 'node:events'
import {mkdir,mkdtemp,readFile,readdir,symlink,unlink,writeFile} from 'node:fs/promises'
import {join,resolve} from 'node:path'
import {pdfFixture} from '../crates/marknoria/tests/browser/pdf-fixture.mjs'
import {Workbook} from '../ui/vendor/artifact-table.mjs'
const root=resolve(new URL('../',import.meta.url).pathname),output=join(root,'target/wsl-reader-smoke')
await mkdir(output,{recursive:true});const directory=await mkdtemp(join(output,'run-'))
const project=join(directory,'project%20literal'),shared=join(directory,'shared'),notes=join(directory,'notes')
for(const path of [project,shared])await mkdir(path)
const png=Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAIAAAABCAYAAAD0In+KAAAADklEQVR4nGP4z8AAQv8BD/kD/YURmXYAAAAASUVORK5CYII=','base64')
for(const path of [join(project,'project.png'),join(shared,'first.png')])await writeFile(path,png)
await writeFile(join(shared,'second.png'),Buffer.concat([png,Buffer.from('second snapshot')]))
const outside=join(directory,'outside.png');await writeFile(outside,png)
const alias=join(project,'alias.png');await symlink(join(shared,'first.png'),alias)
const markdown='\ufeff# WSL transport\r\n\r\n![project](project.png) ![shared](../shared/first.png) ![alias](alias.png)\r\n'
const files=[['notes.md','markdown',markdown],['plain.txt','text','Exact 中文 😀\r\nsecond line\r\n'],['data.json','text','{"name":"中文","value":42}\n'],['data.csv','csv','\ufeffName;Value\r\n001;7\r\n002;8\r\n'],['project.png','image',png],['book.pdf','pdf',pdfFixture()]]
const book=new Workbook(),sheet=book.addWorksheet('Data');sheet.addRow(['Name','Value']);sheet.addRow(['001',7]);sheet.addRow(['002',8])
files.push(['book.xlsx','xlsx',Buffer.from(await book.xlsx.writeBuffer())])
for(const [name,,bytes]of files)await writeFile(join(project,name),bytes)
const binary=process.env.STUDIO_WSL_READER_BINARY||join(root,'target/debug/examples/wsl-reader-host')
let sequence=0,savedNote,resourceId
const pause=ms=>new Promise(resolve=>setTimeout(resolve,ms))
async function session(name,distribution,run){
 const cache=join(directory,'cache-'+sequence++);await mkdir(cache)
 const child=spawn(binary,[project,join(project,name),'--shared='+shared,'--notes='+notes],{env:{PATH:'/usr/bin:/bin',LANG:'C.UTF-8',READER_CACHE_DIR:cache,WSL_DISTRO_NAME:distribution},stdio:['pipe','pipe','pipe']})
 let stdout='',stderr='';child.stdout.on('data',bytes=>stdout+=bytes);child.stderr.on('data',bytes=>stderr+=bytes)
 try{
  const until=Date.now()+30000
  while(!stdout.includes('\n')&&child.exitCode===null&&Date.now()<until)await pause(20)
  assert.match(stdout,/http:\/\/127\.0\.0\.1:/,stderr)
  const url=new URL(stdout.trim()),headers={'x-reader-token':url.hash.slice(1)}
  const call=(path,body)=>fetch(new URL(path,url),{headers:{...headers,...(body?{'Content-Type':'application/json'}:{})},method:body?'POST':'GET',...(body?{body:JSON.stringify(body)}:{})})
  let document
  while(Date.now()<until){document=await(await call('/api/document')).json();if(document.state==='ready')break;assert.notEqual(document.state,'error',document.error);await pause(20)}
  assert.equal(document.state,'ready');headers['x-reader-document']=document.documentId
  const caps=await(await call('/api/capabilities')).json();assert.equal(caps.edit,false)
  assert.equal(caps.referenceProfile.hostImages,true)
  assert.equal((await call('/api/edit')).status,403)
  await run({document,call,updateShared:values=>child.stdin.write(JSON.stringify(values)+'\n')})
  const exit=once(child,'exit');child.stdin.end('\n');await Promise.race([exit,pause(10000)])
  assert.equal(child.exitCode,0,stderr);assert.deepEqual(await readdir(cache),[])
 }finally{if(child.exitCode===null&&child.signalCode===null){const exit=once(child,'exit');child.kill('SIGKILL');await exit}}
}
const results=[]
for(const [name,format,bytes]of files)await session(name,'fixture-a',async({document,call,updateShared})=>{
 assert.equal(document.index.format,format)
 if(typeof bytes==='string')assert.equal((await(await call('/api/source?line=1')).json()).result.text,bytes)
 if(format==='image'){assert.equal(document.index.assets[0].width,2);assert.equal((await call('/api/asset?id=0&frame=0')).status,200)}
 if(format==='pdf')assert.deepEqual(Buffer.from(await(await call('/api/original?revision='+document.index.revision)).arrayBuffer()),bytes)
 if(format==='csv'||format==='xlsx'){
  const result=await(await call('/api/table-query',{table:0,revision:document.index.revision,sql:'SELECT COUNT(*),SUM(CAST(c2 AS INTEGER)) FROM t'})).json()
  assert.deepEqual(result.rows,[['2','15']])
 }
 if(format==='markdown'){
  resourceId=document.resourceId
  const endpoint=id=>`/api/reference-image?id=${id}&revision=${document.index.revision}`
  const ready=async id=>{const until=Date.now()+30000;while(Date.now()<until){const response=await call(endpoint(id));assert.ok(response.status===200||response.status===202,await response.clone().text());const value=await response.json();if(value.state==='ready')return value;await pause(20)}throw Error('WSL image preparation timed out')}
  const projectImage=await ready(0),sharedImage=await ready(1),aliased=await ready(2)
  const frame=(id,value)=>call(endpoint(id)+'&frame=0&imageRevision='+value.imageRevision)
  for(let n=0;n<3;n++)assert.equal((await frame(1,sharedImage)).status,200)
  updateShared([])
  const until=Date.now()+5000;while((await call(endpoint(1))).status!==403){assert.ok(Date.now()<until);await pause(20)}
  assert.equal((await frame(1,sharedImage)).status,403);assert.equal((await frame(0,projectImage)).status,200)
  updateShared([shared]);await pause(50)
  await unlink(alias);await symlink(outside,alias)
  assert.equal((await call(endpoint(2))).status,403)
  await unlink(alias);await symlink(join(shared,'second.png'),alias)
  const changed=await ready(2);assert.notEqual(changed.imageRevision,aliased.imageRevision)
  assert.equal((await frame(2,aliased)).status,409)
  const start=Buffer.from(markdown).indexOf(Buffer.from('WSL transport'))
  const note=await call('/api/notes',{operation:'add',kind:'favorite',quote:'WSL transport',revision:document.index.revision,anchor:{location:{kind:'source',startByte:start,endByte:start+13}}})
  assert.equal(note.status,200);savedNote=(await note.json()).id
 }
 results.push({name,format,readOnly:true,exactBytes:typeof bytes==='string'})
})
await session('notes.md','fixture-a',async({document,call})=>{
 assert.equal(document.resourceId,resourceId)
 const saved=await(await call('/api/notes',{operation:'list'})).json()
 assert.equal(saved.total,1);assert.equal(saved.items[0].id,savedNote);assert.equal(saved.items[0].stale,false)
})
await session('notes.md','fixture-b',async({document,call})=>{
 assert.notEqual(document.resourceId,resourceId);assert.equal((await(await call('/api/notes',{operation:'list'})).json()).total,0)
})
await writeFile(join(project,'notes.md'),markdown+'\r\nChanged revision.\r\n')
await session('notes.md','fixture-a',async({document,call})=>{
 assert.equal(document.resourceId,resourceId);assert.equal((await(await call('/api/notes',{operation:'list'})).json()).items[0].stale,true)
})
await writeFile(join(project,'notes.md'),markdown)
for(const [name,,bytes]of files)assert.deepEqual(await readFile(join(project,name)),Buffer.from(bytes))
console.log(JSON.stringify({ok:true,linuxTransportSubstitute:true,actualWindowsWsl:false,results,dynamicRevocation:true,canonicalRetarget:true,stableNotes:true,environmentIsolation:true,cleanup:true,artifacts:directory}))
