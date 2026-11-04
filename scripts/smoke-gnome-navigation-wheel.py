#!/usr/bin/env python3
"""Real Wayland wheel cancellation of the unchanged native list coordinator.

The standalone Rust example controls when its target becomes available. Only
the owned Mutter compositor generates wheel input. No application profile,
gateway or inference provider is started. This is component integration scope;
installed turn-navigation workflows have separate acceptance.
"""
import argparse
import json
import os
from pathlib import Path
import resource
import select
import subprocess
import sys
import tempfile
import time
import xml.etree.ElementTree as ET

sys.dont_write_bytecode = True
REPO = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO / 'scripts/test-support'))
from private_wayland import OwnedMutterInput, isolated_environment
from native_accessibility import Client


def run(binary, root):
    owned = OwnedMutterInput()
    log = (root / 'fixture.log').open('w')
    child = subprocess.Popen([str(binary)], stdin=subprocess.PIPE,
                             stdout=subprocess.PIPE, stderr=log, env=os.environ)
    os.set_blocking(child.stdout.fileno(), False)
    client = Client(child)
    events, buffer = [], b''

    def pump():
        nonlocal buffer
        while select.select([child.stdout], [], [], 0)[0]:
            chunk = os.read(child.stdout.fileno(), 16384)
            if not chunk:
                break
            buffer += chunk
        while b'\n' in buffer:
            line, buffer = buffer.split(b'\n', 1)
            value = json.loads(line)
            value['receivedAt'] = time.monotonic()
            events.append(value)
        assert len(events) < 500, 'Fixture telemetry exceeded its bound'

    def wait(name, predicate, seconds=5):
        end = time.monotonic() + seconds
        while time.monotonic() < end:
            pump()
            if child.poll() is not None:
                raise AssertionError((name, 'Fixture exited', child.returncode))
            value = predicate()
            if value:
                return value
            time.sleep(.002)
        raise AssertionError((name, events[-12:]))

    def command(value):
        child.stdin.write((value+'\n').encode())
        child.stdin.flush()

    def state():
        before = len(events)
        command('state')
        return wait('current fixture state', lambda: next(
            (row for row in events[before:] if row['event']=='state'), None))

    def start(identity):
        command('start')
        probe = wait('mapped pending target', lambda: next(
            (row for row in events if row['event']=='probe' and
             row['case']==identity and row['mapped']), None))
        assert probe['calls'] <= 3, ('Pending observer was not caught early', probe)
        return probe

    try:
        frame = client.wait('owned native fixture', lambda: client.find(
            lambda node: node.get_role_name()=='frame' and
            node.get_name()=='Native navigation wheel fixture'))
        owned.focus_only_window()
        client.wait('owned maximized fixture', lambda:
            client.api.Component.get_extents(frame.get_component_iface(),
                client.api.CoordType.WINDOW).width >= 1500)
        time.sleep(.4)  # Maximize bounds precede the compositor transition.
        owned.call('NotifyPointerMotionRelative', owned.GLib.Variant('(dd)', (-10000., -10000.)))
        owned.call('NotifyPointerMotionRelative', owned.GLib.Variant('(dd)', (800., 500.)))
        xml = owned.bus.call_sync(owned.target, owned.path,
            'org.freedesktop.DBus.Introspectable', 'Introspect', None,
            owned.GLib.VariantType.new('(s)'), owned.Gio.DBusCallFlags.NONE,
            3000, None).unpack()[0]
        method = ET.fromstring(xml).find(".//method[@name='NotifyPointerAxisDiscrete']")
        assert method is not None
        assert [arg.attrib['type'] for arg in method.findall('arg')
                if arg.attrib.get('direction', 'in')=='in'] == ['u', 'i']
        assert state()['mapped']

        start(1)
        time.sleep(.5)
        baseline = state()
        assert baseline['calls'] >= 5, 'Positive baseline did not run its frame observer'
        time.sleep(.2)
        assert state()['calls']==baseline['calls'], 'Frame observer did not retire within its bound'

        trials = []
        for identity, direction in [(2, 1), (3, -1), (4, 1)]:
            probe = start(identity)
            began = time.monotonic()
            owned.call('NotifyPointerAxisDiscrete', owned.GLib.Variant('(ui)', (0, direction)))
            input_ms = (time.monotonic()-began)*1000
            time.sleep(.5)  # Longer than the uncancelled baseline frame budget.
            stopped = state()
            assert 0 < stopped['calls'] < baseline['calls'], (probe, stopped, baseline)
            assert abs(stopped['offset']-probe['offset']) > 1, 'Real wheel did not scroll the view'
            command('arm')
            time.sleep(.2)
            after = state()
            assert after['calls']==stopped['calls'], 'Cancelled observer resumed when the target became available'
            assert abs(after['offset']-stopped['offset']-120) <= 1, 'Cancelled jump snapped back after user scroll'
            trials.append(dict(direction=direction, pendingCalls=probe['calls'],
                stoppedCalls=stopped['calls'], wheelRpcMs=round(input_ms, 2),
                wheelDelta=round(stopped['offset']-probe['offset'], 2)))

        start(5)
        command('arm')
        time.sleep(.5)
        aligned = state()
        assert aligned['anchorY'] is not None and abs(aligned['anchorY']-16) <= 1, aligned
        command('quit')
        child.wait(timeout=5)
        assert child.returncode==0
        runtime = (root/'fixture.log').read_text()
        assert not any(token in runtime for token in ['CRITICAL', 'panicked', 'Gtk-WARNING']), runtime
        result = dict(ok=True, actualWaylandWheel=True, componentScope=True,
            applicationHooks=False, baselineCalls=baseline['calls'],
            positiveAnchorY=aligned['anchorY'], trials=trials,
            noGateway=True, noInference=True, artifacts=str(root))
        (root/'result.json').write_text(json.dumps(result, indent=2)+'\n')
        print(json.dumps(result), flush=True)
    finally:
        if child.poll() is None:
            child.terminate()
            child.wait(timeout=5)
        pump()
        (root/'events.json').write_text(json.dumps(events, indent=2)+'\n')
        owned.close()
        log.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('fixture', type=Path)
    parser.add_argument('--private-root', type=Path)
    args = parser.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    binary = args.fixture.resolve()
    if args.private_root:
        run(binary, args.private_root)
        return
    parent = Path(os.environ.get('STUDIO_NATIVE_QA_DIR', REPO/'target/gnome-native-smoke'))
    parent.mkdir(parents=True, exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix='navigation-wheel-', dir=parent))
    env = dict(os.environ, GDK_BACKEND='wayland', GTK_A11Y='atspi',
        XDG_CONFIG_HOME=str(root/'config'), XDG_CACHE_HOME=str(root/'cache'),
        XDG_DATA_HOME=str(root/'data'), WAYLAND_DISPLAY='studio-test')
    with isolated_environment(env, headless=True) as private:
        subprocess.run(['dbus-run-session', '--', 'mutter', '--headless', '--wayland',
            '--no-x11', '--wayland-display', 'studio-test', '--virtual-monitor',
            '1600x1000', '--', sys.executable, str(Path(__file__).resolve()),
            str(binary), '--private-root', str(root)], env=private, cwd=root,
            check=True, timeout=45)


if __name__=='__main__':
    main()
