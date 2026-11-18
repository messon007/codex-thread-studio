"""Native WebKit full-app comment editing; DOM pointer events, not OS mouse input."""
import json
import sys
import gi
gi.require_version('Gtk', '3.0')
gi.require_version('WebKit2', '4.1')
from gi.repository import Gtk, WebKit2, GLib, Gdk
config = json.loads(sys.argv[2])
manager = WebKit2.UserContentManager()
manager.register_script_message_handler('commentQa')
view = WebKit2.WebView(web_context=WebKit2.WebContext.new_ephemeral(), user_content_manager=manager)
window = Gtk.Window(); window.set_default_size(1500, 1000); window.add(view)
result = None
started = False
script = r'''(async()=>{
const cfg=CONFIG, pause=ms=>new Promise(r=>setTimeout(r,ms));
const until=async(f,label)=>{const end=performance.now()+15000;while(!f()){if(performance.now()>end)throw Error(label);await pause(25)}};
const check=(value,label)=>{if(!value)throw Error(label)};
try{
 await until(()=>window.__studioDeveloper && document.querySelector('.thread-row[data-thread-id="00000000-0000-4000-8000-000000000777"]'),'thread list');
 document.querySelector('.thread-row[data-thread-id="00000000-0000-4000-8000-000000000777"]').click();
 await until(()=>document.querySelector('#thread-title').textContent.includes('Document integration fixture'),'thread selected');
 const open=async()=>{
   await __studioDeveloper.openArtifact({root:cfg.project,path:cfg.path});
   await until(()=>document.querySelector('.artifact-large-markdown-frame')&&!document.querySelector('.artifact-large-markdown-frame').hasAttribute('aria-hidden')&&!document.querySelector('#artifact-preview').disabled,'reader');
   const frame=document.querySelector('.artifact-large-markdown-frame'),doc=frame.contentDocument;
   await until(()=>Number(doc.querySelector('[data-comment-marks]')?.dataset.commentMarks)>0,'painted comment');
   const range=[...doc.defaultView.CSS.highlights.entries()].filter(([name])=>name.startsWith('marknoria-comments-')).flatMap(([,ranges])=>[...ranges])[0];
   check(range?.toString()==='TARGET','exact comment range');
   const box=range.getBoundingClientRect(),x=box.left+box.width/2,y=box.top+box.height/2,target=doc.elementFromPoint(x,y);
   const event=(name,detail=1)=>target.dispatchEvent(new doc.defaultView[name.startsWith('pointer')?'PointerEvent':'MouseEvent'](name,{bubbles:true,cancelable:true,clientX:x,clientY:y,button:0,isPrimary:true,pointerId:1,pointerType:'mouse',detail}));
   return {doc,range,event};
 };
 let mark=await open();mark.event('pointerdown');mark.event('pointerup');mark.event('click');
 await until(()=>document.querySelector('#annotation-dialog').open,'direct click editor');
 check(document.querySelector('#annotation-comment').value==='Native WebKit comment','saved note');
 const note=document.querySelector('#annotation-comment');note.value='Edited in native WebKit';note.dispatchEvent(new Event('input',{bubbles:true}));document.querySelector('#save-annotation').click();
 await until(()=>!document.querySelector('#annotation-dialog').open,'saved');
 await pause(300);document.querySelector('#close-artifact').click();
 mark=await open();mark.event('pointerdown');mark.event('pointerup');mark.event('click');
 await until(()=>document.querySelector('#annotation-dialog').open,'reopen editor');
 check(document.querySelector('#annotation-comment').value==='Edited in native WebKit','edited note survives reopen');
 document.querySelector('#annotation-dialog').close();
 mark.event('pointerdown');mark.event('pointerup');mark.event('click');
 mark.event('pointerdown',2);mark.event('pointerup',2);mark.event('click',2);
 mark.doc.getSelection().removeAllRanges();mark.doc.getSelection().addRange(mark.range.cloneRange());
 await pause(650);check(!document.querySelector('#annotation-dialog').open,'double click does not edit');
 check(mark.doc.getSelection().toString()==='TARGET','selection retained');
 document.querySelector('#close-artifact').click();await pause(350);
 document.querySelector('#open-thread-comments').click();
 await until(()=>document.querySelector('.annotation-source'),'comment source action');
 document.querySelector('.annotation-source').click();
 await until(()=>document.querySelector('.artifact-large-markdown-frame')&&!document.querySelector('.artifact-large-markdown-frame').hasAttribute('aria-hidden'),'reopened reference');
 const frame=document.querySelector('.artifact-large-markdown-frame');
 const {createReaderClient}=await import('/reader-assets/host-client.mjs');const client=createReaderClient(frame);await client.ready();
 let selected;const deadline=performance.now()+10000;
 do{selected=await client.request('selection',{textOnly:true});if(selected?.quote==='TARGET')break;await pause(25)}while(performance.now()<deadline);
 check(selected?.quote==='TARGET'&&selected.textRange?.segments[0]?.startUtf16>20,'restored second saved occurrence');
 client.disconnect();document.querySelector('#close-artifact').click();await pause(350);
 webkit.messageHandlers.commentQa.postMessage(JSON.stringify({ok:true,fullApp:true,pointerInput:'DOM-dispatched',editSaveReopen:true,doubleClickSelection:true,exactSourceReopen:true}));
}catch(error){webkit.messageHandlers.commentQa.postMessage(JSON.stringify({error:String(error),stack:error.stack}))}
})();void 0;'''.replace('CONFIG',json.dumps(config))
def received(_manager,message):
 global result
 result=json.loads(message.get_js_value().to_string());Gtk.main_quit()
def loaded(_view,event):
 global started
 if event==WebKit2.LoadEvent.FINISHED and not started:
  started=True;view.run_javascript(script,None,None,None)
manager.connect('script-message-received::commentQa',received);view.connect('load-changed',loaded)
window.show_all();window.present();view.load_uri(sys.argv[1])
GLib.timeout_add_seconds(70,lambda:(Gtk.main_quit(),False)[1]);Gtk.main();window.destroy()
if not result or not result.get('ok'):raise RuntimeError(json.dumps(result))
result.update(nativeWayland='Wayland' in Gdk.Display.get_default().__gtype__.name,version=f'{WebKit2.get_major_version()}.{WebKit2.get_minor_version()}.{WebKit2.get_micro_version()}')
print(json.dumps(result))
