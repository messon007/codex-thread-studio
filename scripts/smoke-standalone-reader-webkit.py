"""Mapped native Wayland standalone check; synthetic fixtures and test diagnostics."""
import json
import sys
import time
from pathlib import Path
from urllib.request import Request, urlopen
from urllib.parse import urlsplit, quote
import gi
gi.require_version('Gtk', '3.0')
gi.require_version('WebKit2', '4.1')
from gi.repository import Gtk, WebKit2, GLib, Gdk
if 'Wayland' not in Gdk.Display.get_default().__gtype__.name:
    raise RuntimeError('Native Wayland is required')
url, cfg = sys.argv[1], json.loads(sys.argv[2])
parts = urlsplit(url)
manager = WebKit2.UserContentManager()
manager.register_script_message_handler('result')
view = WebKit2.WebView(web_context=WebKit2.WebContext.new_ephemeral(), user_content_manager=manager)
window = Gtk.Window()
window.set_default_size(1500, 1000)
window.add(view)
queue = [(phase, fmt, path) for phase in ['fresh-application-cache', 'reopen-browser-cache'] for fmt, path in cfg['cases']]
results, failure = [], None
current = None
started_at = None

def advance():
    global current, started_at
    if not queue:
        Gtk.main_quit()
        return False
    current = queue.pop(0)
    started_at = int(time.time()*1000)
    _, _, path = current
    request = Request(parts.scheme+'://'+parts.netloc+'/api/open?path='+quote(Path(path).name), method='POST', headers={'x-reader-token': parts.fragment})
    with urlopen(request, timeout=10) as response:
        if response.status != 200:
            raise RuntimeError('Standalone open failed')
    view.load_uri(url)
    return False

def received(_manager, message):
    global failure
    result = json.loads(message.get_js_value().to_string())
    if 'error' in result:
        failure = result
        Gtk.main_quit()
        return
    result['phase'] = current[0]
    result['hostFirstUsefulMs'] = result['initial']['observedAt']-started_at
    results.append(result)
    if current[0] == 'fresh-application-cache':
        def saved(webview, response):
            global failure
            try:
                webview.get_snapshot_finish(response).write_to_png(cfg['output']+'/webkit-'+current[1]+'-dark.png')
                GLib.idle_add(advance)
            except Exception as error:
                failure = {'error': str(error)}
                Gtk.main_quit()
        view.get_snapshot(WebKit2.SnapshotRegion.VISIBLE, WebKit2.SnapshotOptions.NONE, None, saved)
    else:
        GLib.idle_add(advance)

def loaded(webview, event):
    if event != WebKit2.LoadEvent.FINISHED:
        return
    args = {'format': current[1], 'name': Path(current[2]).name}
    function = Path(cfg['script']).read_text()
    script = '(async()=>{try{const result=await '+function+'('+json.dumps(args)+');webkit.messageHandlers.result.postMessage(JSON.stringify(result))}catch(error){webkit.messageHandlers.result.postMessage(JSON.stringify({error:String(error),stack:error.stack}))}})();void 0;'
    webview.run_javascript(script, None, None, None)
manager.connect('script-message-received::result', received)
view.connect('load-changed', loaded)
window.show_all()
GLib.idle_add(advance)
GLib.timeout_add_seconds(170, lambda: (Gtk.main_quit(), False)[1])
Gtk.main()
window.destroy()
if failure or len(results) != 8:
    raise RuntimeError(json.dumps(failure or {'error':'Standalone check timed out'}))
print(json.dumps({'ok':True,'nativeWayland':True,'results':results,'browserVersion':f'WebKit {WebKit2.get_major_version()}.{WebKit2.get_minor_version()}.{WebKit2.get_micro_version()}'}))
