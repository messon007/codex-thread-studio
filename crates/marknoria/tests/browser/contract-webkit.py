"""Mapped native Wayland WebKit host for public ordinary-DOM contracts."""
import json
import sys
import gi
gi.require_version('Gtk', '3.0')
gi.require_version('WebKit2', '4.1')
from gi.repository import Gtk, WebKit2, GLib

manager = WebKit2.UserContentManager()
manager.register_script_message_handler('result')
view = WebKit2.WebView(web_context=WebKit2.WebContext.new_ephemeral(), user_content_manager=manager)
window = Gtk.Window()
window.set_default_size(int(sys.argv[2]) if len(sys.argv) > 2 else 1400,
                        int(sys.argv[3]) if len(sys.argv) > 3 else 950)
window.add(view)
result = None

def receive(_manager, message):
    global result
    result = json.loads(message.get_js_value().to_string())
    Gtk.main_quit()

def loaded(_view, event):
    if event == WebKit2.LoadEvent.FINISHED:
        view.run_javascript("(async()=>{try{const {run}=await import('/contract.mjs');window.webkit.messageHandlers.result.postMessage(JSON.stringify(await run()))}catch(error){window.webkit.messageHandlers.result.postMessage(JSON.stringify({error:String(error),stack:error.stack}))}})();void 0;", None, None, None)

manager.connect('script-message-received::result', receive)
view.connect('load-changed', loaded)
window.show_all()
view.load_uri(sys.argv[1])
GLib.timeout_add_seconds(180, lambda: (Gtk.main_quit(), False)[1])
Gtk.main()
window.destroy()
if result is None or 'error' in result:
    raise RuntimeError(result or 'WebKit contract timed out')
result['browserVersion'] = f'WebKit {WebKit2.get_major_version()}.{WebKit2.get_minor_version()}.{WebKit2.get_micro_version()}'
print(json.dumps(result))
