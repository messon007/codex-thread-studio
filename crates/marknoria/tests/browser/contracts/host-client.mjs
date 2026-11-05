export async function hostClientContract(moduleUrl) {
  const { createReaderClient } = await import(moduleUrl)
  const frame = document.createElement('iframe')
  // The first hello is intentionally lost. Only timer-driven retries can reach
  // this listener, as happens when a real document initializes asynchronously.
  frame.srcdoc = `<script type="module">
    import { installReaderBridge } from ${JSON.stringify(new URL('./reader-bridge.mjs', new URL(moduleUrl, location.href)).href)};
    window.received=[];window.echoCalls=0;
    addEventListener('message',event=>{if(event.data?.method==='hello')received.push({id:event.data.requestId,parent:event.source===parent})});
    setTimeout(() => {
      installReaderBridge({origins:[${JSON.stringify(location.origin)}],commands:{echo:value=>{echoCalls++;return value}},state:()=>({state:'ready',bridgeVersion:1})});
      // A same-origin message from this window must still be rejected.
      setTimeout(()=>{window.postMessage({protocol:'marknoria.reader',version:1,kind:'request',requestId:crypto.randomUUID(),method:'echo',params:{}},'*')},0);
    }, 700);
  <\/script>`
  document.body.append(frame)
  const client = createReaderClient(frame, { timeout:3000 })
  let assertions = 0
  const check = (value, message) => { if (!value) throw Error(message); assertions++ }
  try {
    check((await client.ready()).state === 'ready', 'Delayed child rejected the timer-driven parent hello')
    const received = frame.contentWindow.received
    check(received.length >= 2 && new Set(received.map(value=>value.id)).size === 1, 'Test did not exercise retries of the same hello')
    check(received.every(value=>value.parent), 'Retry sender was not the parent window')
    check((await client.request('echo', {text:'literal 中文😀'})).text === 'literal 中文😀', 'Ready client lost command response')
    check(frame.contentWindow.echoCalls === 1, 'Bridge accepted a same-origin request from the wrong window')
    await client.dispose({ force:true })
    check(client.active === 0, 'Client retained pending requests after disposal')
  } finally { client.disconnect(); frame.remove() }
  const silent = document.createElement('iframe');document.body.append(silent)
  const unavailable = createReaderClient(silent, {timeout:100})
  try {
    check(await unavailable.request('hello').then(()=>false,error=>error.code==='TIMEOUT'), 'Unavailable reader did not time out')
    unavailable.disconnect()
    check(unavailable.active===0, 'Disconnected client retained timeout/retry work')
  } finally {unavailable.disconnect();silent.remove()}
  return { assertions }
}
