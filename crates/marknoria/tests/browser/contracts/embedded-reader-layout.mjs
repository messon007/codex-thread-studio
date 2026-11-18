import { createReaderClient } from '/host-client.mjs'
import { layoutAssertions } from '/layout-contract.mjs'

export async function embeddedLayoutContract(urls, token) {
  const test=layoutAssertions(), pending=new Map(), panes=[]
  const receive=event=>{
    const item=pending.get(event.data?.id)
    if(event.data?.readerLayoutTest!==token || !item || item.frame.contentWindow!==event.source || new URL(item.frame.src).origin!==event.origin) return
    clearTimeout(item.timer);pending.delete(event.data.id)
    event.data.error?item.reject(Error(event.data.error)):item.resolve(event.data.result)
  }
  addEventListener('message',receive)
  const probe=(pane,action='sample',params={})=>new Promise((resolve,reject)=>{
    const id=crypto.randomUUID(),timer=setTimeout(()=>{pending.delete(id);reject(Error(`Layout probe ${action} timed out`))},20000)
    pending.set(id,{frame:pane.frame,timer,resolve,reject})
    pane.frame.contentWindow.postMessage({readerLayoutTest:token,id,action,params},new URL(pane.frame.src).origin)
  })
  try {
    for(const [i,url] of urls.entries()) {
      const frame=document.createElement('iframe');frame.src=url;frame.title=`Layout ${i}`
      frame.style.cssText='display:block;width:1320px;height:850px;border:0'
      document.body.append(frame)
      const pane={frame,client:createReaderClient(frame)};panes.push(pane)
      const client=pane.client, initial=await client.ready()
      test.check(initial.capabilities.edit === (i===1),'Public write grant changed')
      test.geometry(await probe(pane),`embedded-${i}-first`)
      const typography=Object.fromEntries(['contentFontSize','codeFontSize','contentFontFamily','codeFontFamily','contentFontWeight','fontSize','fontFamily'].map(key=>[key,initial.presentation[key]]))
      for(const [label,patch] of [
        ['identical-typography',typography],
        ['theme-and-identical-fonts',{...typography,theme:'dark'}],
        ['locale-and-identical-fonts',{...typography,locale:'zh-CN'}],
        ['code-only-no-code',{codeFontSize:14}],
        ['code-reset-auto',{codeFontSize:null}],
        ['body-size',{contentFontSize:19}],
        ['body-font',{contentFontFamily:'serif',contentFontWeight:600}],
        ['ui-font',{fontFamily:'sans-serif',fontSize:16}],
        ['host-toolbar',{toolbar:'host'}],
        ['reader-toolbar',{toolbar:'reader'}],
        ['font-restore',{...typography,locale:'en',theme:'light'}],
      ]) {
        const before=await probe(pane,'geometry')
        await client.request('settings',patch)
        const after=test.geometry(await probe(pane),`embedded-${i}-${label}`)
        test.check(after.position.line===before.position.line,`${label}: source reading anchor changed ${before.position.line}→${after.position.line}`)
      }
      await client.request('settings',{contentFontFamily:'LayoutLoaded'})
      test.geometry(await probe(pane,'fontLoad'),`embedded-${i}-loaded-font`)
      await client.request('settings',typography)
      frame.style.width='760px';test.geometry(await probe(pane),`embedded-${i}-narrow`)
      frame.style.width='1320px';test.geometry(await probe(pane),`embedded-${i}-wide`)
      test.geometry(await probe(pane,'scroll',{top:-1}),`embedded-${i}-out`)
      test.geometry(await probe(pane,'scroll',{top:0}),`embedded-${i}-back`)
      const moving=client.request('navigate',{location:{kind:'cell',table:0,row:13,column:2}})
      await client.request('settings',{codeFontSize:16})
      await moving
      const navigated=test.geometry(await probe(pane),`embedded-${i}-pending-navigation`)
      const selection=await client.request('selection')
      test.check(selection?.location?.row===13 && selection.location.column===2,'Settings interrupted table navigation/selection')
      test.check(selection.viewportRect.y >= navigated.viewport.top-1 && selection.viewportRect.y+selection.viewportRect.height <= navigated.viewport.top+navigated.viewport.height+1,'Settings left the selected table cell outside the viewport')
      await client.request('settings',{...typography})
      const retained=await client.request('selection')
      test.check(retained?.location?.row===13,'Typography cleared table selection')
      test.check(Math.abs(retained.viewportRect.y-selection.viewportRect.y)<2,'Code-only settings moved the selected table row')
      const found=await client.request('search',{query:'Example question 13'})
      await client.request('searchActivate',{searchId:found.searchId,index:0})
      await client.request('settings',{codeFontSize:15})
      test.check((await client.request('state')).search.current===0,'Typography lost active search occurrence')
      await client.request('setView',{view:'source'});await probe(pane,'select')
      const source=(await probe(pane,'geometry')).code
      await client.request('settings',{codeFontSize:22})
      const sourceAfter=(await probe(pane)).at(-1).code
      test.check(source.text===sourceAfter.text && source.from===sourceAfter.from && source.to===sourceAfter.to,'Source settings changed text/selection')
      if(i===0) test.check(await client.request('setView',{view:'edit'}).then(()=>false,()=>true),'Readonly document accepted Edit')
      else {
        await client.request('setView',{view:'edit'});await probe(pane,'draft')
        const draft=(await probe(pane,'geometry')).code
        await client.request('settings',{codeFontSize:18,contentFontFamily:'serif'})
        const after=(await probe(pane)).at(-1)
        test.check(after.dirty && after.code.text===draft.text && after.code.from===draft.from && after.code.to===draft.to,'Embedded settings lost draft/caret')
      }
      await client.request('dispose',{force:true});frame.remove()
    }
    return {ok:true,mode:'public-embedded',...test.report()}
  } finally {
    removeEventListener('message',receive)
    for(const item of pending.values()){clearTimeout(item.timer);item.reject(Error('Test disposed'))}pending.clear()
    for(const pane of panes){await pane.client.request('dispose',{force:true}).catch(()=>{});pane.frame.remove()}
  }
}
