import json
import sys
import gi

gi.require_version('Gtk', '3.0')
gi.require_version('WebKit2', '4.1')
from gi.repository import Gtk, WebKit2, GLib

context = WebKit2.WebContext.new_ephemeral()
manager = WebKit2.UserContentManager()
manager.register_script_message_handler('result')
view = WebKit2.WebView(web_context=context, user_content_manager=manager)
window = Gtk.OffscreenWindow()
window.set_default_size(1400, 1000)
window.add(view)
result = None


def receive(manager, message):
    global result
    result = json.loads(message.get_js_value().to_string())
    Gtk.main_quit()


def loaded(view, event):
    if event == WebKit2.LoadEvent.FINISHED:
        view.run_javascript("(async () => { try { const config = await (await fetch('/config.json')).json(); const report = await (await import('/browser.mjs')).measure(config); window.webkit.messageHandlers.result.postMessage(JSON.stringify(report)); } catch(error) { window.webkit.messageHandlers.result.postMessage(JSON.stringify({error:String(error),stack:error.stack})); } })()", None, None, None)


manager.connect('script-message-received::result', receive)
view.connect('load-changed', loaded)
window.show_all()
view.load_uri(sys.argv[1])
GLib.timeout_add_seconds(110, lambda: (Gtk.main_quit(), False)[1])
Gtk.main()
window.destroy()
if result is None or 'error' in result:
    raise RuntimeError(result or 'WebKit benchmark timed out')
print(json.dumps(result))
