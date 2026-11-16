"""Cold history recovery in the actual app using ephemeral native Wayland WebKit."""
import json, sys, gi
gi.require_version('Gtk','3.0');gi.require_version('WebKit2','4.1')
from gi.repository import Gtk, WebKit2, GLib, Gdk
if 'Wayland' not in Gdk.Display.get_default().__gtype__.name: raise RuntimeError('Native Wayland required')
manager=WebKit2.UserContentManager();manager.register_script_message_handler('historyQa')
view=WebKit2.WebView(web_context=WebKit2.WebContext.new_ephemeral(),user_content_manager=manager)
window=Gtk.Window();window.set_default_size(1600,1000);window.add(view)
results=[];started=False
script=r'''(async()=>{
const until=async(f,label)=>{const end=performance.now()+30000;while(!f()){if(performance.now()>end)throw Error(label);await new Promise(r=>setTimeout(r,25))}};
try{
await until(()=>document.querySelector('.thread-row[data-thread-id="00000000-0000-4000-8000-000000000777"]'),'thread list');
const start=performance.now();document.querySelector('.thread-row[data-thread-id="00000000-0000-4000-8000-000000000777"]').click();
await until(()=>document.querySelectorAll('#turn-navigator-list [data-turn-nav-id]').length===402,'complete history');
const elapsedMs=performance.now()-start;
const ids=[...document.querySelectorAll('#turn-navigator-list [data-turn-nav-id]')].map(n=>n.dataset.turnNavId);
if(ids.some((id,i)=>id!==`turn-${i+1}`))throw Error('turn order');
for(const i of [1,402]){document.querySelector(`[data-turn-nav-id="turn-${i}"]`).click();await until(()=>document.querySelector(`.turn[data-turn-id="turn-${i}"]`)?.textContent.includes(`Fixture answer ${i}`),'endpoint navigation')}
webkit.messageHandlers.historyQa.postMessage(JSON.stringify({ok:true,turns:ids.length,elapsedMs}));
}catch(e){webkit.messageHandlers.historyQa.postMessage(JSON.stringify({error:String(e)}))}
})();void 0;'''
def received(_manager,message):
 global started
 result=json.loads(message.get_js_value().to_string());results.append(result)
 if result.get('error') or len(results)==2:Gtk.main_quit()
 else:started=False;view.reload()
def loaded(_view,event):
 global started
 if event==WebKit2.LoadEvent.FINISHED and not started:
  started=True;view.run_javascript(script,None,None,None)
manager.connect('script-message-received::historyQa',received);view.connect('load-changed',loaded)
window.show_all();window.present();view.load_uri(sys.argv[1])
GLib.timeout_add_seconds(90,lambda:(Gtk.main_quit(),False)[1]);Gtk.main();window.destroy()
if len(results)!=2 or any(not r.get('ok') for r in results):raise RuntimeError(json.dumps(results))
print(json.dumps({'ok':True,'nativeWayland':True,'version':f'{WebKit2.get_major_version()}.{WebKit2.get_minor_version()}.{WebKit2.get_micro_version()}','runs':results}))
