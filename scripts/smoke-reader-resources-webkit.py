"""Actual gateway-served reader resources in native Wayland WebKitGTK."""
import gi, json, sys
gi.require_version('Gtk','3.0')
gi.require_version('WebKit2','4.1')
from gi.repository import Gtk, WebKit2, GLib
manager=WebKit2.UserContentManager()
manager.register_script_message_handler('result')
view=WebKit2.WebView(web_context=WebKit2.WebContext.new_ephemeral(),user_content_manager=manager)
window=Gtk.Window();window.set_default_size(1300,900);window.add(view)
result=None
def receive(_manager,message):
 global result
 result=json.loads(message.get_js_value().to_string());Gtk.main_quit()
def loaded(_view,event):
 if event!=WebKit2.LoadEvent.FINISHED:return
 view.run_javascript(r"""(async()=>{try{
 const {createReaderClient}=await import('./host-client.mjs');let link;
 const client=createReaderClient({src:location.href,contentWindow:window},{onEvent(type,payload){if(type==='linkRequested')link=payload}});
 const owner=await client.ready();if(!owner.capabilities.referenceProfile.dynamicRoots)throw Error('Dynamic resource API absent');
 const wait=async predicate=>{const until=performance.now()+30000;while(!predicate()){if(performance.now()>until)throw Error('Resource timeout');await new Promise(resolve=>setTimeout(resolve,30))}};
 await wait(()=>document.querySelector('[data-image-reference="0"] [data-asset-state="ready"] img'));
 await wait(()=>document.querySelector('[data-image-reference="1"] [data-asset-state="ready"] img'));
 await wait(()=>document.querySelector('[data-image-reference="2"] [data-asset-state="ready"] img'));
 let image;do{image=await client.request('referenceImageInfo',{id:2});if(image.state!=='ready')await new Promise(resolve=>setTimeout(resolve,30))}while(image.state!=='ready');
 const frame=await client.request('referenceImageFrame',{id:2,imageRevision:image.imageRevision,frame:0});
 if(!(frame.blob instanceof Blob)||frame.blob.type!=='image/png'||image.asset.width!==2)throw Error('Incorrect image transport');
 document.querySelector('[data-link-reference="4"]').click();await wait(()=>link);
 if(link.documentId!==owner.documentId||link.revision!==owner.revision||link.precision!=='exact')throw Error('Link source identity lost');
 await client.dispose();window.webkit.messageHandlers.result.postMessage(JSON.stringify({resources:true,inline:true,linkIdentity:true,cleanup:true}));
 }catch(error){window.webkit.messageHandlers.result.postMessage(JSON.stringify({error:String(error),stack:error.stack}))}})();void 0;""",None,None,None)
manager.connect('script-message-received::result',receive);view.connect('load-changed',loaded)
window.show_all();view.load_uri(sys.argv[1]);GLib.timeout_add_seconds(90,lambda:(Gtk.main_quit(),False)[1]);Gtk.main();window.destroy()
if result is None or 'error' in result:raise RuntimeError(result or 'Resource WebKit timeout')
result['browserVersion']=f'WebKit {WebKit2.get_major_version()}.{WebKit2.get_minor_version()}.{WebKit2.get_micro_version()}'
print(json.dumps(result))
