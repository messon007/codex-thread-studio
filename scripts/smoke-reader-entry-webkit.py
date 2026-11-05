"""Common ordinary-document reader contract via the actual gateway on Wayland."""
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
 const {createReaderClient}=await import('./host-client.mjs');
 const client=createReaderClient({src:location.href,contentWindow:window});
 const owner=await client.ready();
 const check=(value,message)=>{if(!value)throw Error(message)};
 check(owner.view==='preview','Fresh default must be Preview');
 check(owner.capabilities.notes,'Shared Notes unavailable');
 const expected=EXPECTED;
 check(owner.capabilities.source===(expected.text!==null),'Incorrect source capability');
 if(expected.text!==null) {
  check((await client.request('sourceWindow',{line:1})).result.text===expected.text,'Original source differs');
  check((await client.request('setView',{view:'source'})).view==='source','Source unavailable');
  check((await client.request('setView',{view:'preview'})).view==='preview','Preview unavailable');
 }
 if(expected.name==='small.md') {
  const until=performance.now()+30000;
  while(!document.querySelector('[data-image-reference="0"] [data-asset-state="ready"] img')) {
   if(performance.now()>until)throw Error('Raw image timeout');
   await new Promise(resolve=>setTimeout(resolve,30));
  }
  check((await client.request('tableInfo',{table:0})).rows===1,'Markdown table missing');
 }
 if(expected.name==='book.pdf') {
  await client.request('settings',{pdf:{zoom:1,continuous:false}});
  await client.request('navigate',{location:{kind:'pdf',pages:[{page:7,rects:[{x:0,y:0,width:1,height:0.000001}]}]}});
  const current=await client.request('state');
  check(current.formatState.page===7&&current.formatState.zoom===1&&current.formatState.continuous===false,'PDF settings/navigation lost');
 }
 await client.dispose();window.webkit.messageHandlers.result.postMessage(JSON.stringify({commonEntry:true,sourceExact:expected.text!==null,cleanup:true}));
 }catch(error){window.webkit.messageHandlers.result.postMessage(JSON.stringify({error:String(error),stack:error.stack}))}})();void 0;""".replace("EXPECTED",json.dumps(json.loads(sys.argv[2]),ensure_ascii=True)),None,None,None)
manager.connect('script-message-received::result',receive);view.connect('load-changed',loaded)
window.show_all();view.load_uri(sys.argv[1]);GLib.timeout_add_seconds(90,lambda:(Gtk.main_quit(),False)[1]);Gtk.main();window.destroy()
if result is None or 'error' in result:raise RuntimeError(result or 'Entry WebKit timeout')
result['browserVersion']=f'WebKit {WebKit2.get_major_version()}.{WebKit2.get_minor_version()}.{WebKit2.get_micro_version()}'
print(json.dumps(result))
