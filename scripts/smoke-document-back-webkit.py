"""Native WebKit full-app linked-document history with public reader navigation."""
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
const cfg=CONFIG,pause=ms=>new Promise(r=>setTimeout(r,ms)),check=(value,label)=>{if(!value)throw Error(label)};
const until=async(f,label)=>{const end=performance.now()+15000;while(!f()){if(performance.now()>end)throw Error(label);await pause(25)}};
let client,doc;
try{
 await until(()=>document.querySelector('.thread-row[data-thread-id="00000000-0000-4000-8000-000000000777"]'),'thread');
 document.querySelector('.thread-row[data-thread-id="00000000-0000-4000-8000-000000000777"]').click();
 await until(()=>document.querySelector('#thread-title').textContent.includes('Document integration fixture'),'selected');
 const {createReaderClient}=await import('/reader-assets/host-client.mjs');
 const ready=async name=>{
  await until(()=>document.querySelector('#artifact-title').textContent===name&&document.querySelector('.artifact-large-markdown-frame')&&!document.querySelector('.artifact-large-markdown-frame').hasAttribute('aria-hidden')&&!document.querySelector('#artifact-preview').disabled,'ready '+name);
  client?.disconnect();const frame=document.querySelector('.artifact-large-markdown-frame');doc=frame.contentDocument;client=createReaderClient(frame);await client.ready();
 };
 const label=text=>check(document.querySelector('#close-artifact').getAttribute('aria-label')===text,'Back label '+text);
 const link=async text=>{await until(()=>[...doc.querySelectorAll('a')].some(a=>a.textContent===text),'link '+text);const a=[...doc.querySelectorAll('a')].find(a=>a.textContent===text);a.scrollIntoView({block:'nearest'});await pause(100);const before=(await client.request('state')).position;a.click();return before};
 document.querySelector('#open-workspace-files').click();await until(()=>document.querySelector('.workspace-tree-row[data-path="A.md"]'),'Files');document.querySelector('.workspace-tree-row[data-path="A.md"]').click();await ready('A.md');label('Back to Files');
 await client.request('navigateFragment',{fragment:'a-target'});
 const visits=[];let current='A.md';
 for(const [action,next] of [['Open B','B.md'],['Open C','C.md'],['Open D','D.md'],['Return B','B.md'],['Open C again','C.md'],['Open D','D.md'],['Return A','A.md']]){
  visits.push({name:current,position:await link(action)});await ready(next);current=next;
 }
 const expected=visits.slice(-4).reverse();check(expected.map(v=>v.name).join(',')==='D.md,C.md,B.md,D.md','expected visit order');
 for(const visit of expected){label('Back to '+visit.name);document.querySelector('#close-artifact').click();await ready(visit.name);check(Math.abs((await client.request('state')).position.location.line-visit.position.location.line)<=3,'restore '+visit.name)}
 label('Back to Files');document.querySelector('#close-artifact').click();client.disconnect();await pause(350);
 webkit.messageHandlers.commentQa.postMessage(JSON.stringify({ok:true,fullApp:true,cycle:'A-B-C-D-B-C-D-A',backOrder:expected.map(v=>v.name),positionsRestored:true}));
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
