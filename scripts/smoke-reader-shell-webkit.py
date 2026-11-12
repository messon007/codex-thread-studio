"""Actual Studio app in a mapped, ephemeral native Wayland WebKit window."""
import json
import re
import sys
import gi
gi.require_version('Gtk', '3.0')
gi.require_version('WebKit2', '4.1')
from gi.repository import Gtk, WebKit2, GLib, Gdk

if 'Wayland' not in Gdk.Display.get_default().__gtype__.name:
    raise RuntimeError('This check requires native Wayland')
manager = WebKit2.UserContentManager()
manager.register_script_message_handler('shellResult')
view = WebKit2.WebView(web_context=WebKit2.WebContext.new_ephemeral(), user_content_manager=manager)
window = Gtk.Window()
window.set_default_size(1500, 1000)
window.add(view)
result = None
started = False
config = json.loads(sys.argv[2])

def receive(_manager, message):
    global result
    value = json.loads(message.get_js_value().to_string())
    if 'resize' in value:
        width = value['resize']
        if not isinstance(width, int) or not 900 <= width <= 2400:
            raise RuntimeError('Invalid test window size')
        window.resize(width, 1100)
        GLib.timeout_add(250, lambda: (view.run_javascript('window.__shellResize=' + str(width), None, None, None), False)[1])
        return
    if 'snapshot' in value:
        name = value['snapshot']
        if not re.fullmatch('[a-z-]{1,40}', name):
            raise RuntimeError('Invalid snapshot name')
        def saved(webview, response):
            try:
                surface = webview.get_snapshot_finish(response)
                surface.write_to_png(config['output'] + '/webkit-' + name + '.png')
                webview.run_javascript('window.__shellShot=' + json.dumps(name), None, None, None)
            except Exception as error:
                webview.run_javascript('window.__shellShotError=' + json.dumps(str(error)), None, None, None)
        view.get_snapshot(WebKit2.SnapshotRegion.VISIBLE, WebKit2.SnapshotOptions.NONE, None, saved)
        return
    result = value
    Gtk.main_quit()

def loaded(_view, event):
    global started
    if event != WebKit2.LoadEvent.FINISHED or started:
        return
    started = True
    script = r"""(async()=>{
      const cfg=CONFIG, pause=()=>new Promise(resolve=>setTimeout(resolve,25));
      const until=async(predicate,label)=>{const end=performance.now()+25000;while(!predicate()){if(performance.now()>end)throw Error('Timed out: '+label);await pause()}};
      const send=value=>webkit.messageHandlers.shellResult.postMessage(JSON.stringify(value));
      try {
        await until(()=>window.__studioDeveloper&&document.documentElement.style.getPropertyValue('--ui-font-size'),'actual app preferences');
        const {createReaderClient}=await import('/reader-assets/host-client.mjs');
        const {setLanguage,t}=await import('/i18n.mjs');
        const results=[];
        if(cfg.layout){
          const samples=[];let frame,doc,client,pages;
          const open=async()=>{
            await __studioDeveloper.openArtifact({root:cfg.project,path:cfg.cases[0][1]});
            await until(()=>document.querySelector('.artifact-large-markdown-frame')&&!document.querySelector('#artifact-preview').disabled,'layout reader initialized');
            frame=document.querySelector('.artifact-large-markdown-frame');doc=frame.contentDocument;
            client=createReaderClient(frame);await client.ready();pages=(await client.request('state')).document.projectionPages;
            if(pages<50)throw Error('Layout fixture must exceed the mounted window');
          };
          const sample=async name=>{
            let geometry,stable=0;const end=performance.now()+5000;
            while(performance.now()<end){
              await new Promise(resolve=>setTimeout(resolve,100));
              const nodes=[...doc.querySelectorAll('#space > article[data-ready="true"]')].map(node=>{const rect=node.getBoundingClientRect();return {index:Number(node.dataset.index),top:rect.top,bottom:rect.bottom}}).sort((a,b)=>a.index-b.index);
              geometry={mounted:doc.querySelectorAll('#space > article').length,dom:doc.querySelectorAll('#space *').length,gaps:nodes.slice(1).flatMap((node,i)=>node.index===nodes[i].index+1?[node.top-nodes[i].bottom]:[])};
              if(geometry.gaps.length&&geometry.gaps.every(gap=>Math.abs(gap)<1)){if(++stable===3)break}else stable=0;
            }
            if(stable!==3||geometry.mounted>30||geometry.mounted>=pages/2||geometry.dom>=15000)throw Error(name+': invalid geometry '+JSON.stringify(geometry));
            samples.push({name,...geometry});
          };
          const scroll=async name=>{for(const offset of [0,400,1200,2400,5000,0]){doc.querySelector('#viewport').scrollTop=offset;await sample(name+'-'+offset)}};
          const settings=async(theme,locale,typography)=>{
            __studioDeveloper.click('#settings-button');
            document.querySelector('#theme-select').value=theme;document.querySelector('#language-select').value=locale;
            if(typography){document.querySelector('#content-font-size').value=typography.content;document.querySelector('#code-font-size').value=typography.code;document.querySelector('#reader-automatic-code-size').checked=typography.auto}
            document.querySelector('#settings-form').requestSubmit();await until(()=>!document.querySelector('#settings-dialog').open,'layout settings close');
            for(let attempt=0;attempt<200;attempt++){const state=await client.request('state');if(state.presentation.theme===theme&&state.presentation.locale===locale&&(!typography||state.presentation.contentFontSize===typography.content&&state.presentation.codeFontSize===(typography.auto?null:typography.code)))return;await pause()}
            throw Error('Layout presentation update did not complete');
          };
          await open();await scroll('initial');
          await settings('light','en-US');await scroll('theme');await settings('light','zh-CN');await scroll('locale');
          await settings('light','zh-CN',{content:15,code:14,auto:true});await scroll('code-auto');
          await settings('light','zh-CN',{content:15,code:14,auto:false});await scroll('code-only');
          await settings('light','zh-CN',{content:18,code:24,auto:false});await scroll('actual-font');
          for(const width of [1200,2100,1800]){send({resize:width});await until(()=>window.__shellResize===width,'native window resize');await scroll('resize-'+width)}
          client.disconnect();document.querySelector('#close-artifact').click();await until(()=>!document.querySelector('.artifact-large-markdown-frame'),'layout close');
          await open();await scroll('reopen');client.disconnect();document.querySelector('#close-artifact').click();
          send({ok:true,fullApp:true,nativeWayland:true,layout:true,samples,projectionPages:pages,browserVersion:'WebKit '+VERSION});return;
        }
        for(const [format,path] of cfg.cases){
          const started=performance.now();
          __studioDeveloper.openArtifact({root:cfg.project,path});
          await until(()=>document.querySelector('.artifact-large-markdown-frame'),format+' mounted');
          await until(()=>document.querySelector('.artifact-large-markdown-frame')&&document.querySelector('#artifact-title').textContent.trim()===path.split('/').pop()&&!document.querySelector('#artifact-preview').disabled,format+' initialized');
          const frame=document.querySelector('.artifact-large-markdown-frame'), doc=frame.contentDocument;
          const client=createReaderClient(frame);await client.ready();
          const state=()=>client.request('state'),stages=[];
          const stage=async name=>{await Promise.all([window,doc.defaultView].map(win=>new Promise(resolve=>win.requestAnimationFrame(()=>win.requestAnimationFrame(resolve)))));await pause();stages.push({name,timings:(await state()).timings})};
          const first=await state(),initial={...first.timings,hostFirstUsefulMs:performance.now()-started,mounted:doc.querySelectorAll('#space *').length};
          if((await state()).presentation.toolbar!=='host'||getComputedStyle(doc.querySelector('.reader-header')).display!=='none')throw Error('Duplicate child toolbar');
          for(const theme of ['light','dark']){
            __studioDeveloper.click('#settings-button');
            document.querySelector('#theme-select').value=theme;
            document.querySelector('#language-select').value='en-US';
            document.querySelector('#settings-form').requestSubmit();
            await until(()=>!document.querySelector('#settings-dialog').open,'settings close');
            for(let attempt=0;attempt<200&&(await state()).presentation.theme!==theme;attempt++)await pause();
            if((await state()).presentation.theme!==theme)throw Error('Theme did not reach reader');
            await stage('theme-'+theme);
          }
          let interactionProfile=null;
          if(format==='markdown'){
            const restorers=[],calls=[],sync=[];
            if(cfg.profileInteractions)for(const [realm,win] of [['parent',window],['child',doc.defaultView]]){
              for(const [target,key] of [[win,'getComputedStyle'],[win.HTMLElement.prototype,'focus'],[win.Element.prototype,'getBoundingClientRect']]){
                const original=target[key],record={realm,method:key,count:0,totalMs:0,maxMs:0};calls.push(record);
                target[key]=function(...args){const start=performance.now();try{return Reflect.apply(original,this,args)}finally{const duration=performance.now()-start;record.count++;record.totalMs+=duration;record.maxMs=Math.max(record.maxMs,duration)}};
                restorers.push(()=>{target[key]=original});
              }
            }
            try{
            if(document.querySelector('#artifact-notes-tools button')||doc.querySelector('.notes-open,.notes-selection'))throw Error('Unexpected reader Notes/Favorite entry');
            if((await state()).presentation.notes!=='host'||(await state()).presentation.selectionMenu!=='host')throw Error('Reader selection ownership was not delegated');
            await stage('host-menu-ownership');
            const localeStart=performance.now();setLanguage('zh-CN');sync.push({locale:'zh-CN',ms:performance.now()-localeStart});
            for(let attempt=0;attempt<200&&(await state()).presentation.locale!=='zh-CN';attempt++)await pause();
            if((await state()).presentation.locale!=='zh-CN')throw Error('Child locale did not update');
            await stage('Reader-zh-CN');
            if(cfg.profileInteractions){
              for(const locale of ['en-US','zh-CN']){const start=performance.now();setLanguage(locale);sync.push({locale,ms:performance.now()-start});
                for(let attempt=0;attempt<200&&(await state()).presentation.locale!==locale;attempt++)await pause();
                if((await state()).presentation.locale!==locale)throw Error('Repeated child locale did not update');
                await stage('Reader-repeat-'+locale);
              }
            }
            document.querySelector('#artifact-outline-toggle').click();
            await until(()=>!doc.querySelector('#reader-outline').hidden&&doc.querySelector('#toc-filter').getAttribute('aria-label')===t('Filter contents'),'live Contents locale');
            if(!(await state()).outline.open)throw Error('Public Contents state did not open');
            await stage('Contents-zh-CN');
            setLanguage('en-US');doc.querySelector('#contents-close').click();
            await stage('Contents-en-US-closed');
            }finally{for(const restore of restorers.reverse())restore()}
            if(cfg.profileInteractions)interactionProfile={calls,sync,nestedTimings:true};
          }
          if(format==='markdown'){
            document.querySelector('#artifact-preview').focus();document.activeElement.dispatchEvent(new KeyboardEvent('keydown',{key:'f',ctrlKey:true,bubbles:true,cancelable:true}));
            if(document.querySelector('#artifact-search-panel').classList.contains('hidden'))throw Error('Native shell search shortcut unavailable');
            document.querySelector('#artifact-search-input').dispatchEvent(new KeyboardEvent('keydown',{key:'Escape',bubbles:true,cancelable:true}));
          }
          if(format==='table'){doc.querySelector('.table-query-open').click();await until(()=>doc.querySelector('.table-query-dialog[open]'),'SQL dialog')}
          const visible=node=>node.getClientRects().length&&getComputedStyle(node).visibility!=='hidden';
          for(const owner of [document,doc]){
            const unnamed=[...owner.querySelectorAll('button,input:not([type=hidden]),select,textarea')].filter(node=>visible(node)&&!node.disabled&&!node.getAttribute('aria-label')&&!node.getAttribute('title')&&!node.textContent.trim()&&!node.labels?.length&&!node.getAttribute('placeholder'));
            if(unnamed.length)throw Error('Unnamed shell control: '+unnamed[0].outerHTML.slice(0,160));
          }
          for(const dialog of doc.querySelectorAll('dialog[open]')){
            const r=dialog.getBoundingClientRect();if(r.x<0||r.y<0||r.right>frame.clientWidth+1||r.bottom>frame.clientHeight+1)throw Error('Clipped dialog');
          }
          const afterProductInteractions=(await state()).timings;
          send({snapshot:format});
          await until(()=>window.__shellShot===format||window.__shellShotError,'snapshot');
          if(window.__shellShotError)throw Error(window.__shellShotError);
          results.push({format,phase:'first-open-private-cache',initial,afterProductInteractions,stages,interactionProfile,afterQaSnapshot:(await state()).timings,theme:true,initialized:true,accessibleNames:true,keyboard:format==='markdown'});client.disconnect();
          document.querySelector('#close-artifact').click();
          await until(()=>!document.querySelector('.artifact-large-markdown-frame'),'close');
        }
        for(const [format,path] of cfg.cases){
          const started=performance.now();__studioDeveloper.openArtifact({root:cfg.project,path});
          await until(()=>document.querySelector('.artifact-large-markdown-frame')&&!document.querySelector('#artifact-preview').disabled,'warm initialized');
          const frame=document.querySelector('.artifact-large-markdown-frame'),client=createReaderClient(frame);await client.ready();
          const state=await client.request('state');results.push({format,phase:'reopen-browser-cache',initial:{...state.timings,hostFirstUsefulMs:performance.now()-started,mounted:frame.contentDocument.querySelectorAll('#space *').length}});
          client.disconnect();document.querySelector('#close-artifact').click();await until(()=>!document.querySelector('.artifact-large-markdown-frame'),'warm close');
        }
        send({ok:true,fullApp:true,nativeWayland:true,results,browserVersion:'WebKit '+VERSION});
      } catch(error){send({error:String(error),stack:error.stack,debug:{title:document.querySelector('#artifact-title')?.textContent,error:document.querySelector('#artifact-error-message')?.textContent,loading:document.querySelector('#artifact-loading')?.className,childStatus:document.querySelector('.artifact-large-markdown-frame')?.contentDocument?.querySelector('#status')?.textContent,frames:document.querySelectorAll('.artifact-large-markdown-frame').length,lang:document.documentElement.lang}})}
    })();void 0;""".replace('CONFIG', json.dumps(config)).replace('VERSION', json.dumps(f'{WebKit2.get_major_version()}.{WebKit2.get_minor_version()}.{WebKit2.get_micro_version()}'))
    view.run_javascript(script, None, None, None)

manager.connect('script-message-received::shellResult', receive)
view.connect('load-changed', loaded)
window.show_all()
window.present()
view.load_uri(sys.argv[1])
GLib.timeout_add_seconds(160, lambda: (Gtk.main_quit(), False)[1])
Gtk.main()
window.destroy()
if not result or 'error' in result:
    raise RuntimeError(json.dumps(result or {'error': 'Native shell timeout'}))
print(json.dumps(result))
