import { createReaderClient } from '/host-client.mjs'

export async function tableNavigationContract(urls, token) {
  let assertions = 0
  const check = (value, message) => { if (!value) throw Error(message); assertions++ }
  const pause = ms => new Promise(resolve => setTimeout(resolve, ms))
  const evidence = [], panes = [], pending = new Map()
  const receive = event => {
    if (event.data?.tableNavigationTest !== token) return
    const operation = pending.get(event.data.id)
    if (!operation || operation.frame.contentWindow !== event.source || new URL(operation.frame.src).origin !== event.origin) return
    clearTimeout(operation.timer); pending.delete(event.data.id)
    event.data.error ? operation.reject(Error(event.data.error)) : operation.resolve(event.data.result)
  }
  addEventListener('message', receive)
  const probe = (pane, action = 'geometry', row = 119, column = 1) => new Promise((resolve, reject) => {
    const id = crypto.randomUUID(), timer = setTimeout(() => { pending.delete(id); reject(Error('Test geometry timed out')) }, 5000)
    pending.set(id, { frame:pane.frame, resolve, reject, timer })
    pane.frame.contentWindow.postMessage({tableNavigationTest:token,id,action,row,column},new URL(pane.frame.src).origin)
  })
  const settled = async (pane, row, column, label) => {
    const selection = await pane.client.request('selection')
    const {samples} = await probe(pane, 'sample', row, column)
    for (const {checkpoint,geometry} of samples) {
      evidence.push({label,checkpoint,selection,geometry})
      check(selection?.quote === String(column ? row : `row ${row}`), 'Wrong cell text: '+JSON.stringify(evidence))
      check(selection.location.kind === 'cell' && selection.location.table === 0 && selection.location.row === row && selection.location.column === column, 'Wrong original cell location')
      const cell = geometry.cell
      check(cell && cell.y >= geometry.top-1 && cell.y+cell.height <= geometry.top+geometry.height+1 &&
        cell.x >= Math.max(geometry.left,geometry.content.left)-1 &&
        cell.x+cell.width <= Math.min(geometry.left+geometry.width,geometry.content.left+geometry.content.width)+1,
        'Cell clipped after navigation: '+JSON.stringify(evidence))
    }
    const final = await pane.client.request('selection'), actual = (await probe(pane,'geometry',row,column)).cell
    check(final.quote === selection.quote && final.location.row === row && final.location.column === column, 'Selection changed after settled frames')
    check(actual && Math.abs(actual.y-final.viewportRect.y)<1 && Math.abs(actual.x-final.viewportRect.x)<1, 'Public rectangle differs from actual cell')
  }
  try {
    for (const [i, raw] of urls.entries()) {
      const url = new URL(raw); url.searchParams.set('notes','host');url.searchParams.set('selectionMenu','host')
      const frame = document.createElement('iframe');frame.title=`Navigation ${i}`;frame.src=url.href
      frame.style.cssText='width:740px;height:850px;border:0;display:inline-block;vertical-align:top'
      document.body.append(frame)
      const pane = {frame,client:createReaderClient(frame)};panes.push(pane)
      await pane.client.ready()
      await pane.client.request('settings',{toolbar:'reader',notes:'host',selectionMenu:'host'})
      check((await probe(pane)).notes === 0,'Host-owned Notes became visible')
      await pane.client.request('navigate',{location:{kind:'cell',table:0,row:0,column:0}})
      await probe(pane,'hideColumn')
      const deadline = performance.now()+5000
      while ((await probe(pane)).columns.length !== 1) { if(performance.now()>deadline) throw Error('Column projection timed out');await pause(20) }
      await pane.client.request('setView',{view:'source'})
      check((await probe(pane)).sourceReady === 'true','Source did not finish opening')
      await pane.client.request('navigate',{location:{kind:'cell',table:0,row:119,column:1}})
      check((await pane.client.request('state')).view === 'preview','Cell target did not open Preview')
      check((await probe(pane)).columns.join(',') === '0,1','Hidden original column did not return')
      await settled(pane,119,1,`source-hidden-column-${i}`)
      await pane.client.request('settings',{toolbar:'host'})
      await pane.client.request('navigate',{location:{kind:'cell',table:0,row:119,column:1}})
      check((await pane.client.request('state')).presentation.toolbar === 'host','Navigation changed host toolbar policy')
      await settled(pane,119,1,`host-toolbar-${i}`)
      await pane.client.request('navigate',{location:{kind:'cell',table:0,row:51,column:0}})
      await settled(pane,51,0,`preview-existing-column-${i}`)
    }
    // A different public intent cancels an older location before its reveal.
    const pane = panes[0]
    await pane.client.request('setView',{view:'source'})
    const old = pane.client.request('navigate',{location:{kind:'cell',table:0,row:119,column:1}}).then(()=>null,error=>error.code)
    await pane.client.request('navigate',{location:{kind:'cell',table:0,row:51,column:0}})
    check(await old === 'CANCELLED','Older navigation was not superseded')
    await settled(pane,51,0,'latest-request-wins')
    const controller = new AbortController()
    const cancelled = pane.client.request('navigate',{location:{kind:'cell',table:0,row:119,column:1}},{signal:controller.signal}).then(()=>false,error=>error.name === 'AbortError')
    controller.abort();check(await cancelled,'Host cancellation did not reject')
    await pause(250)
    check((await pane.client.request('selection'))?.location.row !== 119,'Cancelled navigation selected its target')
    const moving = pane.client.request('navigate',{location:{kind:'cell',table:0,row:119,column:1}}).then(()=>null,error=>error.code)
    await pause(25); const user = await probe(pane,'wheel')
    check(await moving === 'CANCELLED','User scrolling did not cancel navigation')
    await pause(200)
    check(Math.abs((await probe(pane)).scrollTop-user.scrollTop)<1,'Old navigation recaptured user scroll')
    const stale = await pane.client.request('navigate',{revision:'0'.repeat(64),location:{kind:'cell',table:0,row:119,column:1}}).then(()=>null,error=>error.code)
    check(stale === 'REVISION','Stale navigation bypassed revision ownership')
    const closing = panes[1].client.request('navigate',{location:{kind:'cell',table:0,row:119,column:1}}).then(()=>false,()=>true)
    await panes[1].client.dispose();check(await closing,'Disposal acknowledged an active navigation')
    await pause(200);check((await probe(panes[1])).cell === null,'Disposed navigation resurrected table content')
    return {assertions,checkpoints:evidence.length,publicNavigation:true,actualViewport:true,cancellation:true,hostNotes:true,evidence}
  } finally {
    removeEventListener('message',receive)
    for (const operation of pending.values()) {clearTimeout(operation.timer);operation.reject(Error('Test disposed'))}
    pending.clear()
    for (const pane of panes) { await pane.client.dispose().catch(()=>{});pane.frame.remove() }
  }
}
