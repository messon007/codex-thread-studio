#!/usr/bin/env python3
"""Real pointer/keyboard contract on the private, PID-verified GNOME test seat."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import gi
gi.require_version('Gtk', '3.0')
gi.require_version('WebKit2', '4.1')
from gi.repository import Gtk, WebKit2, GLib, Gdk

if not os.environ.get('MARKNORIA_TEST_COMPOSITOR_PID'):
    raise RuntimeError('Run through tests/native/owned-wayland-run.py')
base = Path('target').resolve()
base.mkdir(exist_ok=True)
output = Path(tempfile.mkdtemp(prefix='comment-pointer-', dir=base))
helper = subprocess.Popen(['/usr/bin/python3', 'tests/native/gnome-input.py', str(output)])
manager = WebKit2.UserContentManager()
manager.register_script_message_handler('result')
manager.register_script_message_handler('pointer')
manager.add_script(WebKit2.UserScript.new('''
window.marknoriaObservedPointer=null;window.addEventListener('pointermove',event=>{if(event.isTrusted)window.marknoriaObservedPointer=[event.clientX,event.clientY]});
let pointerSequence=0;const pointerPending=new Map();
window.marknoriaPointerInput=value=>new Promise((resolve,reject)=>{
 const id=++pointerSequence;pointerPending.set(id,{resolve,reject});
 window.webkit.messageHandlers.pointer.postMessage(JSON.stringify({id,value}));
});window.marknoriaPointerDone=(id,error)=>{const entry=pointerPending.get(id);pointerPending.delete(id);if(error)entry.reject(Error(error));else entry.resolve()};
''', WebKit2.UserContentInjectedFrames.TOP_FRAME, WebKit2.UserScriptInjectionTime.START, None, None))
view = WebKit2.WebView(web_context=WebKit2.WebContext.new_ephemeral(), user_content_manager=manager)
window = Gtk.Window()
window.set_default_size(1400, 950)
window.add(view)
window.fullscreen()
result = None
sequence = 0
pointer = [0.0, 0.0]
busy = False
observed_pointer = None
view.add_events(Gdk.EventMask.POINTER_MOTION_MASK)
def motion(_view, event):
    global observed_pointer
    observed_pointer = [event.x, event.y]
    return False
view.connect('motion-notify-event', motion)

def send(actions, callback):
    global sequence, busy
    if busy:
        raise RuntimeError('Input contract must serialize its requests')
    busy = True
    sequence += 1
    expected = sequence
    temporary = output / 'input-request.tmp'
    temporary.write_text(json.dumps({'sequence': sequence, 'actions': actions}))
    temporary.replace(output / 'input-request.json')
    deadline = time.monotonic() + 8
    def poll():
        global busy
        path = output / 'input-done.json'
        if path.exists():
            data = json.loads(path.read_text())
            if data['sequence'] == expected:
                busy = False
                callback(data.get('error'))
                return False
        if helper.poll() is not None or time.monotonic() > deadline:
            busy = False
            callback('Owned pointer input timed out')
            return False
        return True
    GLib.timeout_add(15, poll)

def receive_input(_manager, message):
    request = json.loads(message.get_js_value().to_string())
    value, actions = request['value'], []
    kind = value['type']
    if kind == 'screenshot':
        import re
        label = value['label']
        if not re.fullmatch('[a-z-]{1,40}', label): raise ValueError('Invalid screenshot label')
        def captured(_view, task):
            error = None
            try: view.get_snapshot_finish(task).write_to_png(str(output / (label + '.png')))
            except Exception as failure: error = str(failure)
            view.run_javascript('window.marknoriaPointerDone(%d,%s);void 0' % (request['id'], json.dumps(error)), None, None, None)
        view.get_snapshot(WebKit2.SnapshotRegion.VISIBLE, WebKit2.SnapshotOptions.NONE, None, captured)
        return
    if kind == 'text':
        text = value['text']
        if len(text) > 64 or any(not 32 <= ord(c) <= 126 for c in text):
            raise ValueError('This input fixture accepts at most 64 ASCII characters')
        actions = [{'kind': 'key', 'keysym': ord(c), 'delayMs': 5} for c in text]
    else:
        x, y = float(value['x']), float(value['y'])
        if not 0 <= x < view.get_allocated_width() or not 0 <= y < view.get_allocated_height():
            raise ValueError('Pointer outside the actual fullscreen WebView')
        actions.append({'kind': 'motion', 'dx': x-pointer[0], 'dy': y-pointer[1], 'delayMs': 30})
        pointer[:] = [x, y]
        if kind in ('click', 'double'):
            actions.append({'kind': 'click', 'delayMs': 60})
            if kind == 'double': actions.append({'kind': 'click', 'delayMs': 60})
        elif kind in ('down', 'up'):
            actions.append({'kind': 'button', 'pressed': kind == 'down', 'delayMs': 30})
        elif kind != 'move': raise ValueError('Unknown pointer action')
    send(actions, lambda error: view.run_javascript('window.marknoriaPointerDone(%d,%s);void 0' % (request['id'], json.dumps(error)), None, None, None))

def receive(_manager, message):
    global result
    result = json.loads(message.get_js_value().to_string())
    if 'error' in result:
        def failure_snapshot(_view, task):
            try: view.get_snapshot_finish(task).write_to_png(str(output / 'failure.png'))
            finally: Gtk.main_quit()
        view.get_snapshot(WebKit2.SnapshotRegion.VISIBLE, WebKit2.SnapshotOptions.NONE, None, failure_snapshot)
    else: Gtk.main_quit()

def fail(error):
    global result
    result = {'error': str(error)}
    Gtk.main_quit()

def run(error=None):
    if error: return fail(error)
    def calibrated(_view, task):
        observed = json.loads(view.run_javascript_finish(task).get_js_value().to_string())
        state = {'screen': [900,600], 'observedLocal': observed,
                 'gtkObserved': observed_pointer, 'active': window.is_active(),
                 'viewSize': [view.get_allocated_width(),view.get_allocated_height()]}
        (output / 'pointer-origin.json').write_text(json.dumps(state))
        if observed is None: return fail('Real calibration motion did not enter WebKit: ' + json.dumps(state))
        pointer[:] = observed
        if not window.is_active() or view.get_allocated_width() < 1000:
            return fail('Owned fullscreen WebKit host is not active: ' + json.dumps(state))
        view.run_javascript("(async()=>{try{const {run}=await import('/contract.mjs');window.webkit.messageHandlers.result.postMessage(JSON.stringify(await run()))}catch(error){window.webkit.messageHandlers.result.postMessage(JSON.stringify({error:String(error),stack:error.stack}))}})();void 0;", None, None, None)
    view.run_javascript('JSON.stringify(window.marknoriaObservedPointer)', None, calibrated)

def loaded(_view, event):
    if event != WebKit2.LoadEvent.FINISHED: return
    def activate():
        if helper.poll() is not None: fail('Owned input broker exited during initialization'); return False
        if not (output / 'input-ready.json').exists(): return True
        window.present()
        # GNOME initially shows Overview. These ordinary keys target only the
        # freshly owned compositor; they run before the document fixture exists.
        send([{'kind':'key','keysym':65307}, {'kind':'key','keysym':65513,'pressed':True},
              {'kind':'key','keysym':65289}, {'kind':'key','keysym':65513,'pressed':False},
              {'kind':'motion','dx':-10000,'dy':-10000}, {'kind':'motion','dx':900,'dy':600,'delayMs':250}], run)
        return False
    GLib.timeout_add(50, activate)

manager.connect('script-message-received::result', receive)
def guarded_input(*args):
    try: receive_input(*args)
    except Exception as error: fail(error)
manager.connect('script-message-received::pointer', guarded_input)
view.connect('load-changed', loaded)
try:
    window.show_all()
    view.load_uri(sys.argv[1])
    GLib.timeout_add_seconds(150, lambda: (Gtk.main_quit(), False)[1])
    Gtk.main()
    if result is None or 'error' in result: raise RuntimeError(result or 'Pointer contract timed out')
    result.update(nativeDisplay=window.get_display().__gtype__.name, realWaylandInput=True, artifacts=str(output), browserVersion=f'WebKit {WebKit2.get_major_version()}.{WebKit2.get_minor_version()}.{WebKit2.get_micro_version()}')
    print(json.dumps(result))
finally:
    (output / 'input-stop').touch()
    try: helper.wait(timeout=5)
    except subprocess.TimeoutExpired: helper.terminate(); helper.wait(timeout=5)
    window.destroy()
