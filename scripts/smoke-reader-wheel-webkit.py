"""Exercise engine default scrolling via native GTK input, not DOM WheelEvent."""
import json,sys,gi
gi.require_version('Gtk','3.0');gi.require_version('WebKit2','4.1')
from gi.repository import Gtk,WebKit2,Gdk,GLib
manager=WebKit2.UserContentManager();manager.register_script_message_handler('qa')
view=WebKit2.WebView(web_context=WebKit2.WebContext.new_ephemeral(),user_content_manager=manager)
win=Gtk.Window();win.set_default_size(1500,1000);win.add(view);result=None;started=False
script=r'''(async()=>{
const pause=ms=>new Promise(r=>setTimeout(r,ms)),send=v=>webkit.messageHandlers.qa.postMessage(JSON.stringify(v));
const until=async(f,label)=>{for(let i=0;i<600;i++){if(f())return;await pause(30)}throw Error(label)};
try{
await until(()=>document.querySelector('.thread-row[data-thread-id="00000000-0000-4000-8000-000000000777"]'),'thread');document.querySelector('.thread-row[data-thread-id="00000000-0000-4000-8000-000000000777"]').click();
await until(()=>document.querySelector('#thread-title').textContent.includes('Document integration fixture'),'selected');
const results=[];window.wheelDone=0;
for(let i=0;i<3;i++){
 if(i===0)document.querySelector('#open-workspace-files').click();
 await until(()=>document.querySelector('.workspace-tree-row[data-path="scroll.md"]'),'Files row');
 const row=document.querySelector('.workspace-tree-row[data-path="scroll.md"]');row.focus();row.click();
 await until(()=>document.querySelector('.artifact-large-markdown-frame')&&!document.querySelector('#artifact-preview').disabled,'ready');
 const frame=document.querySelector('.artifact-large-markdown-frame'),doc=frame.contentDocument;
 await until(()=>doc.querySelector('#viewport .page'),'page');
 const vp=doc.querySelector('#viewport'),events=[];
 doc.addEventListener('wheel',e=>events.push({trusted:e.isTrusted,target:e.target.id,delta:e.deltaY,prevented:e.defaultPrevented}),{capture:true});
 const state=()=>({top:vp.scrollTop,total:vp.scrollHeight,height:vp.clientHeight,active:doc.activeElement?.id,parentActive:document.activeElement?.id,inert:frame.inert});
 const before=state(),box=frame.getBoundingClientRect(),rect=vp.getBoundingClientRect(),ticket=wheelDone+1;
 send({wheel:{x:box.left+rect.left+rect.width/2,y:box.top+rect.top+rect.height/2,ticket}});
 await until(()=>wheelDone===ticket,'native scroll');await pause(700);
 const after=state();if(after.top<=before.top)throw Error('First native wheel failed: '+JSON.stringify({before,after,events}));if(!events.length||events.some(e=>!e.trusted||e.prevented))throw Error('Expected trusted non-cancelled native wheel events');const upTicket=wheelDone+1;send({wheel:{x:box.left+rect.left+rect.width/2,y:box.top+rect.top+rect.height/2,ticket:upTicket,up:true}});await until(()=>wheelDone===upTicket,'native up scroll');await pause(700);const up=state();if(up.top>=after.top)throw Error('Native upward wheel failed');results.push({before,after,up,events});
 document.querySelector('#close-artifact').click();await pause(100);
}
await pause(350);send({ok:true,results});
}catch(e){send({error:String(e),stack:e.stack})}
})();void 0;'''
def received(_m,message):
 global result
 value=json.loads(message.get_js_value().to_string())
 if 'wheel' in value:
  data=value['wheel']
  for i in range(3):
   ev=Gdk.Event.new(Gdk.EventType.SCROLL);ev.window=view.get_window();ev.send_event=True;ev.time=Gdk.CURRENT_TIME;ev.x=data['x'];ev.y=data['y'];ev.direction=Gdk.ScrollDirection.UP if data.get('up') else Gdk.ScrollDirection.DOWN;ev.device=Gdk.Display.get_default().get_default_seat().get_pointer()
   view.event(ev)
  GLib.timeout_add(100,lambda:(view.run_javascript('window.wheelDone='+str(data['ticket']),None,None,None),False)[1]);return
 result=value;Gtk.main_quit()
def loaded(_v,event):
 global started
 if event==WebKit2.LoadEvent.FINISHED and not started:started=True;view.run_javascript(script,None,None,None)
manager.connect('script-message-received::qa',received);view.connect('load-changed',loaded)
win.show_all();win.present();view.load_uri(sys.argv[1]);GLib.timeout_add_seconds(90,lambda:(Gtk.main_quit(),False)[1]);Gtk.main();win.destroy()
if not result or not result.get('ok'):raise RuntimeError(json.dumps(result))
result.update(nativeWayland='Wayland' in Gdk.Display.get_default().__gtype__.name,version=f'{WebKit2.get_major_version()}.{WebKit2.get_minor_version()}.{WebKit2.get_micro_version()}',input='native GTK scroll events')
print(json.dumps(result))
