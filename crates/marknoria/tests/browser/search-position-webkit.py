"""Real mapped native Wayland WebKit validation of the reader's search UI."""
import json
import sys
from pathlib import Path
import gi

gi.require_version('Gtk', '3.0')
gi.require_version('WebKit2', '4.1')
from gi.repository import Gtk, WebKit2, GLib

manager = WebKit2.UserContentManager()
manager.register_script_message_handler('result')
if '--fallback' in sys.argv:
    manager.add_script(WebKit2.UserScript.new('window.Highlight = undefined;', WebKit2.UserContentInjectedFrames.ALL_FRAMES, WebKit2.UserScriptInjectionTime.START, None, None))
view = WebKit2.WebView(web_context=WebKit2.WebContext.new_ephemeral(), user_content_manager=manager)
window = Gtk.Window()
window.set_default_size(1440, 850)
window.add(view)
result = None


def receive(_manager, message):
    global result
    result = json.loads(message.get_js_value().to_string())
    Gtk.main_quit()


def loaded(_view, event):
    if event != WebKit2.LoadEvent.FINISHED:
        return
    contract = sys.argv[sys.argv.index('--contract') + 1] if '--contract' in sys.argv else 'search-position.mjs'
    if contract not in ('search-position.mjs', 'go-to-line.mjs'):
        raise ValueError('Unknown reader contract')
    script = Path(__file__).with_name('contracts').joinpath(contract).read_text().replace('export async function run()', 'async function run()')
    view.run_javascript(script + ';run().then(result=>window.webkit.messageHandlers.result.postMessage(JSON.stringify(result))).catch(error=>window.webkit.messageHandlers.result.postMessage(JSON.stringify({error:String(error),stack:error.stack})));void 0;', None, None, None)


manager.connect('script-message-received::result', receive)
view.connect('load-changed', loaded)
# Present the mapped native Wayland surface so compositor throttling cannot
# stall animation-frame-based layout/activation behind another test window.
window.show_all()
window.present()
view.load_uri(sys.argv[1])
GLib.timeout_add_seconds(90, lambda: (Gtk.main_quit(), False)[1])
Gtk.main()
window.destroy()
if result is None or 'error' in result:
    raise RuntimeError(result or 'WebKit search contract timed out')
result['browserVersion'] = f'WebKit {WebKit2.get_major_version()}.{WebKit2.get_minor_version()}.{WebKit2.get_micro_version()}'
print(json.dumps(result))
