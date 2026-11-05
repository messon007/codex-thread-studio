"""Actual Studio + shared jq in a mapped native Wayland WebKit window."""
import json, sys, gi
gi.require_version('Gtk', '3.0')
gi.require_version('WebKit2', '4.1')
from gi.repository import Gtk, WebKit2, GLib, Gdk
if 'Wayland' not in Gdk.Display.get_default().__gtype__.name:
    raise RuntimeError('Native Wayland required')
config = json.loads(sys.argv[2])
manager = WebKit2.UserContentManager()
manager.register_script_message_handler('issueResult')
view = WebKit2.WebView(web_context=WebKit2.WebContext.new_ephemeral(), user_content_manager=manager)
window = Gtk.Window(); window.set_default_size(1440, 1000); window.add(view)
result = None
started = False

def received(_manager, message):
    global result
    result = json.loads(message.get_js_value().to_string()); Gtk.main_quit()

script = r'''(async()=>{
 const cfg=CONFIG,pause=()=>new Promise(resolve=>setTimeout(resolve,25));
 const until=async(fn,label)=>{const end=performance.now()+30000;while(!fn()){if(performance.now()>end)throw Error('Timeout: '+label);await pause()}};
 try{
  await until(()=>window.__studioDeveloper,'Studio app');
  const {createReaderClient}=await import('/reader-assets/host-client.mjs');const results=[];
  for(const item of cfg.cases){
   __studioDeveloper.openArtifact({root:cfg.project,path:cfg.project+'/'+item.name});
   await until(()=>document.querySelector('#artifact-title')?.textContent.trim()===item.name&&document.querySelector('.artifact-large-markdown-frame')&&!document.querySelector('#artifact-preview').disabled,'open '+item.name);
   const frame=document.querySelector('.artifact-large-markdown-frame'),client=createReaderClient(frame),owner=await client.ready();
   if(owner.view!=='preview')throw Error('Fresh document did not open in Preview');
   await until(()=>frame.contentDocument?.querySelector('pre[data-document-code]')?.getClientRects().length,'visible Preview');
   const source=(await client.request('sourceWindow',{line:1})).result;
   if(!source.text.startsWith(item.prefix))throw Error('Source prefix changed');
   if(item.filter){
    const button=document.querySelector('#artifact-query-tools button[aria-label="jq"]');
    if(!button?.getClientRects().length)throw Error('jq toolbar entry hidden');button.click();
    await until(()=>document.querySelector('.jq-query-dialog'),'jq dialog');
    const dialog=document.querySelector('.jq-query-dialog'),rect=dialog.getBoundingClientRect();
    if(rect.x<0||rect.y<0||rect.right>innerWidth+1||rect.bottom>innerHeight+1)throw Error('Clipped jq dialog');
    await until(()=>!dialog.querySelector('.jq-run').disabled,'initial jq');
    dialog.querySelector('textarea').value=item.filter;dialog.querySelector('.jq-run').click();
    await until(()=>dialog.querySelector('.jq-results').textContent.trim()===item.expected,'complete jq input');
    dialog.querySelector('button[aria-label="Close"]').click();
   }else if(document.querySelector('#artifact-query-tools button'))throw Error('jq shown for XML');
   await client.request('setView',{view:'source'});client.disconnect();document.querySelector('#close-artifact').click();
   await until(()=>!document.querySelector('.artifact-large-markdown-frame'),'close');results.push({name:item.name,preview:true,sourcePrefix:true,jq:Boolean(item.filter)});
  }
  const controls=document.querySelector('#composer-images');for(let node=controls;node&&node!==document.body;node=node.parentElement){node.classList.remove('hidden');node.hidden=false}controls.innerHTML=cfg.attachmentMarkup;
  const button=controls.querySelector('button'),icon=button.querySelector('svg'),b=button.getBoundingClientRect(),i=icon.getBoundingClientRect();
  if(!b.width||Math.abs(b.x+b.width/2-i.x-i.width/2)>=1||Math.abs(b.y+b.height/2-i.y-i.height/2)>=1)throw Error('Attachment icon not centered');
  webkit.messageHandlers.issueResult.postMessage(JSON.stringify({ok:true,nativeWayland:true,fullApp:true,results,attachmentCentered:true}));
 }catch(error){webkit.messageHandlers.issueResult.postMessage(JSON.stringify({error:String(error),stack:error.stack,status:document.querySelector('#artifact-error-message')?.textContent}))}
})();void 0;'''.replace('CONFIG',json.dumps(config))

def loaded(_view,event):
    global started
    if event == WebKit2.LoadEvent.FINISHED and not started:
        started=True; view.run_javascript(script,None,None,None)
manager.connect('script-message-received::issueResult',received)
view.connect('load-changed',loaded)
window.show_all();view.load_uri(sys.argv[1])
GLib.timeout_add_seconds(160,lambda:(Gtk.main_quit(),False)[1]);Gtk.main();window.destroy()
if not result or 'error' in result: raise RuntimeError(json.dumps(result or {'error':'Wayland issue regression timed out'}))
print(json.dumps(result))
