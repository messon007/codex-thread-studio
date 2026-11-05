// Trusted transport grants are rechecked before cached metadata and frames.
import assert from 'node:assert/strict'
import {spawn} from 'node:child_process'
import {once} from 'node:events'
import {mkdir,mkdtemp,readFile,readdir,realpath,writeFile} from 'node:fs/promises'
import {join,resolve} from 'node:path'
const directory=await mkdtemp(join(await realpath('target'),'host-image-'))
const cache=join(directory,'cache');await mkdir(cache)
const source=join(directory,'host.md'),picture=join(directory,'picture.png'),policy=join(directory,'policy.json')
// The established generated two-pixel fixture, with no external/private data.
const pixels=Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAIAAAABCAYAAAD0In+KAAAADklEQVR4nGP4z8AAQv8BD/kD/YURmXYAAAAASUVORK5CYII=','base64')
await writeFile(picture,pixels);await writeFile(source,'# Host images\n\n![host](virtual/picture.png)\n')
let grant={allowed:true,target:'virtual/picture.png',path:picture,identity:'first'}
const publish=()=>writeFile(policy,JSON.stringify(grant));await publish()
const child=spawn(resolve(process.env.MARKNORIA_EMBEDDED_BINARY||'target/debug/examples/embedded-host'),[source,'--host-image-policy='+policy],{env:{...process.env,READER_CACHE_DIR:cache},stdio:['pipe','pipe','pipe']})
let stdout='',stderr='';child.stdout.on('data',bytes=>stdout+=bytes);child.stderr.on('data',bytes=>stderr+=bytes)
const pause=ms=>new Promise(resolve=>setTimeout(resolve,ms))
try {
 const until=Date.now()+30000
 while(!stdout.includes('\n')&&child.exitCode===null&&Date.now()<until)await pause(20)
 assert.match(stdout,/http:\/\/127\.0\.0\.1:/,stderr)
 const url=new URL(stdout.trim()),headers={'x-reader-token':url.hash.slice(1)}
 const call=path=>fetch(new URL(path,url),{headers})
 let document
 while(Date.now()<until){document=await(await call('/api/document')).json();if(document.state==='ready')break;assert.notEqual(document.state,'error',document.error);await pause(20)}
 assert.equal(document.state,'ready');headers['x-reader-document']=document.documentId
 assert.equal((await(await call('/api/capabilities')).json()).referenceProfile.hostImages,true)
 const endpoint=`/api/reference-image?id=0&revision=${document.index.revision}`
 const ready=async()=>{const deadline=Date.now()+30000;while(Date.now()<deadline){const response=await call(endpoint);assert.ok(response.status===200||response.status===202,await response.clone().text());const value=await response.json();if(value.state==='ready')return value;await pause(20)}throw Error('Host image preparation timeout')}
 const first=await ready();assert.equal(first.asset.width,2)
 const frame=image=>call(endpoint+'&frame=0&imageRevision='+image.imageRevision)
 for(let i=0;i<5;i++){assert.equal((await frame(first)).status,200);assert.equal((await call(endpoint)).status,200)}
 grant.allowed=false;await publish()
 assert.equal((await call(endpoint)).status,403);assert.equal((await frame(first)).status,403)
 grant.allowed=true;await writeFile(picture,Buffer.concat([pixels,Buffer.from('changed snapshot')]))
 grant.identity='second';await publish()
 const second=await ready();assert.notEqual(second.imageRevision,first.imageRevision)
 assert.equal((await frame(first)).status,409);assert.equal((await frame(second)).status,200)
 grant.name='../escape.png';await publish();assert.equal((await call(endpoint)).status,400)
 const exit=once(child,'exit');child.stdin.end('\n');await Promise.race([exit,pause(10000)])
 assert.equal(child.exitCode,0,stderr);assert.deepEqual(await readdir(cache),[])
 assert.match(stderr,/HOST_IMAGE_SNAPSHOT_READS=2(?:\s|$)/)
 assert.equal(await readFile(source,'utf8'),'# Host images\n\n![host](virtual/picture.png)\n')
 console.log(JSON.stringify({ok:true,reauthorizedCache:true,conditionalByteTransfer:true,sourceChange:true,basenameBoundary:true,cleanup:true,artifacts:directory}))
} finally {if(child.exitCode===null&&child.signalCode===null){const exit=once(child,'exit');child.kill('SIGKILL');await exit}}
