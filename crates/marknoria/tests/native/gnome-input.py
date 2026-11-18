#!/usr/bin/env python3
"""Send ordinary input only to the owned GNOME check session.

The public test host writes input-request.json and observes input-done.json.
Coordinates are compositor logical coordinates; key values are XKB keysyms.
The D-Bus connection stays alive for the lifetime of the virtual input session.
"""
import json
import os
from pathlib import Path
import sys
import time

from gi.repository import Gio, GLib

runtime = Path(os.environ.get('XDG_RUNTIME_DIR', ''))
if (runtime.parent != Path('/tmp') or not runtime.name.startswith('mn-wayland-')
        or os.environ.get('WAYLAND_DISPLAY') != 'marknoria-check'
        or not os.environ.get('MARKNORIA_TEST_COMPOSITOR')):
    raise RuntimeError('Input requires tests/native/owned-wayland-run.py; never use a login session')

output = Path(sys.argv[1]).resolve(strict=True)
bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
destination = 'org.gnome.Mutter.RemoteDesktop'
interface = destination + '.Session'
expected_pid = int(os.environ.get('MARKNORIA_TEST_COMPOSITOR_PID', '0'))
owner = bus.call_sync('org.freedesktop.DBus', '/org/freedesktop/DBus',
    'org.freedesktop.DBus', 'GetNameOwner', GLib.Variant('(s)', (destination,)),
    None, Gio.DBusCallFlags.NONE, 5000, None).unpack()[0]
actual_pid = bus.call_sync('org.freedesktop.DBus', '/org/freedesktop/DBus',
    'org.freedesktop.DBus', 'GetConnectionUnixProcessID', GLib.Variant('(s)', (owner,)),
    None, Gio.DBusCallFlags.NONE, 5000, None).unpack()[0]
if expected_pid <= 0 or actual_pid != expected_pid:
    raise RuntimeError('Remote input service is not the compositor started by this test')


def call(path, iface, method, signature=None, values=()):
    return bus.call_sync(owner, path, iface, method,
        GLib.Variant(signature, values) if signature else None,
        None, Gio.DBusCallFlags.NONE, 5000, None)


session = call('/org/gnome/Mutter/RemoteDesktop', destination, 'CreateSession').unpack()[0]
call(session, interface, 'Start')
(output / 'input-ready.json').write_text(json.dumps({'ok': True}) + '\n')
previous = None
try:
    while not (output / 'input-stop').exists():
        request_path = output / 'input-request.json'
        if not request_path.exists():
            time.sleep(.025)
            continue
        try:
            request = json.loads(request_path.read_text())
        except json.JSONDecodeError:
            time.sleep(.025)
            continue
        sequence = request['sequence']
        if sequence == previous:
            time.sleep(.025)
            continue
        previous = sequence
        error = None
        try:
            actions = request['actions']
            if len(actions) > 64:
                raise ValueError('Input batches are capped at 64 actions')
            for action in actions:
                kind = action['kind']
                if kind == 'key':
                    keysym = int(action['keysym'])
                    if 'pressed' in action:
                        call(session, interface, 'NotifyKeyboardKeysym', '(ub)', (keysym, bool(action['pressed'])))
                    else:
                        call(session, interface, 'NotifyKeyboardKeysym', '(ub)', (keysym, True))
                        time.sleep(.04)
                        call(session, interface, 'NotifyKeyboardKeysym', '(ub)', (keysym, False))
                elif kind == 'motion':
                    call(session, interface, 'NotifyPointerMotionRelative', '(dd)', (float(action['dx']), float(action['dy'])))
                elif kind == 'click':
                    call(session, interface, 'NotifyPointerButton', '(ib)', (272, True))
                    call(session, interface, 'NotifyPointerButton', '(ib)', (272, False))
                elif kind == 'context-click':
                    # Fixed right button, under the same private compositor/PID
                    # checks as left-click; no arbitrary device/button injection.
                    call(session, interface, 'NotifyPointerButton', '(ib)', (273, True))
                    call(session, interface, 'NotifyPointerButton', '(ib)', (273, False))
                elif kind == 'button':
                    # Owned test-session drag only; never accepts an arbitrary
                    # button/device ID or reaches the user's login session.
                    if not isinstance(action.get('pressed'), bool):
                        raise ValueError('button requires a boolean pressed state')
                    call(session, interface, 'NotifyPointerButton', '(ib)', (272, action['pressed']))
                elif kind == 'scroll':
                    call(session, interface, 'NotifyPointerAxis', '(ddu)', (float(action.get('dx', 0)), float(action.get('dy', 0)), 0))
                else:
                    raise ValueError(f'Unknown input action {kind}')
                pause_ms = int(action.get('delayMs', 60))
                if not 0 <= pause_ms <= 1000:
                    raise ValueError('Input pause must be between 0 and 1000 ms')
                time.sleep(pause_ms / 1000)
        except Exception as failure:
            error = str(failure)
        temporary = output / 'input-done.tmp'
        temporary.write_text(json.dumps({'sequence': sequence, 'ok': error is None, 'error': error}) + '\n')
        temporary.replace(output / 'input-done.json')
finally:
    call(session, interface, 'Stop')
