#!/usr/bin/env python3
"""Cold D-Bus Activate/Open of the real installed native app on private Wayland.

Uses the existing installer, fake providers, accessibility client and owned
Mutter input. No application hook, gateway override or host desktop is used.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import resource
import shutil
import subprocess
import sys
import tempfile
import time

sys.dont_write_bytecode = True
REPO = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO / 'scripts/test-support'))
from native_accessibility import Client, ProcessPeer
from private_wayland import OwnedMutterInput, isolated_environment

spec = importlib.util.spec_from_file_location('activation_history', REPO / 'scripts/smoke-gnome-turn-navigation-scope.py')
history = importlib.util.module_from_spec(spec)
spec.loader.exec_module(history)
APP_ID = 'io.github.messon007.codexthreadstudio.Gnome'
DRAFT = 'Desktop activation draft 中文😀 '
SOURCE = '# Desktop activation document 中文😀\r\n\r\nOriginal source with spaces and symbols.\r\n'


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(prefix, root):
    import gi
    gi.require_version('GioUnix', '2.0')
    from gi.repository import Gio, GioUnix, GLib
    assert not any(key.startswith('STUDIO_GNOME_SMOKE') for key in os.environ)
    assert 'CODEX_THREAD_STUDIO_GATEWAY_BIN' not in os.environ
    owned = OwnedMutterInput()
    bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
    native = prefix / 'bin/codex-thread-studio-gnome'
    source = root / 'project/desktop launch 中文 % $ ` " \\.md'
    originals = {path: digest(path) for path in [source, root/'codex-turns.json', root/'oc-turns.json']}
    peers = []
    phases = []

    def call(method, args, signature):
        return bus.call_sync('org.freedesktop.DBus', '/org/freedesktop/DBus',
            'org.freedesktop.DBus', method, args, GLib.VariantType.new(signature),
            Gio.DBusCallFlags.NONE, 5000, None).unpack()

    def has_owner():
        return call('NameHasOwner', GLib.Variant('(s)', (APP_ID,)), '(b)')[0]

    def until(name, predicate, seconds=30):
        end = time.monotonic() + seconds
        while time.monotonic() < end:
            while GLib.MainContext.default().pending():
                GLib.MainContext.default().iteration(False)
            value = predicate()
            if value:
                return value
            time.sleep(.04)
        raise RuntimeError(('Desktop activation timed out', name, str(root)))

    def peer():
        until('D-Bus application owner', has_owner)
        unique = call('GetNameOwner', GLib.Variant('(s)', (APP_ID,)), '(s)')[0]
        pid = call('GetConnectionUnixProcessID', GLib.Variant('(s)', (unique,)), '(u)')[0]
        process = ProcessPeer(pid, native)
        argv = Path('/proc', str(pid), 'cmdline').read_bytes().split(b'\0')
        assert b'--gapplication-service' in argv, 'Desktop launch used Exec fallback rather than D-Bus service activation'
        return process

    def keys(values):
        for key in values:
            owned.call('NotifyKeyboardKeysym', GLib.Variant('(ub)', (key, True)))
        time.sleep(.04)
        for key in reversed(values):
            owned.call('NotifyKeyboardKeysym', GLib.Variant('(ub)', (key, False)))
        time.sleep(.04)

    def provider_ids():
        return [int(path.read_text()) for path in [root/'backend.pid', root/'opencode.pid'] if path.exists()]

    def close(client, process, providers):
        owned.focus_only_window()
        client.wait('owned focused frame', lambda: client.find(lambda n:
            n.get_role_name() == 'frame' and n.get_state_set().contains(client.api.StateType.ACTIVE)))
        keys([0xffe9, 0xffc1])
        until('native exit after Close', lambda: process.poll() is not None)
        until('D-Bus name released', lambda: not has_owner())
        until('owned provider cleanup', lambda: all(not Path('/proc', str(pid)).exists() for pid in providers))

    try:
        # dbus-run-session starts before Mutter assigns WAYLAND_DISPLAY. Export
        # this private compositor/profile to service activation, as GNOME does
        # for its actual login environment. Never update the user's session bus.
        names = ['PATH', 'WAYLAND_DISPLAY', 'XDG_RUNTIME_DIR', 'XDG_DATA_HOME',
            'XDG_DATA_DIRS', 'XDG_CONFIG_HOME', 'XDG_CACHE_HOME', 'GDK_BACKEND',
            'GTK_A11Y', 'GIO_USE_VFS', 'GIO_USE_VOLUME_MONITOR', 'CODEX_HOME',
            'CODEX_THREAD_STUDIO_CODEX_BIN', 'CODEX_THREAD_STUDIO_OPENCODE_BIN',
            'CODEX_THREAD_STUDIO_BACKENDS_CONFIG', 'READER_CACHE_DIR']
        values = {name: os.environ[name] for name in names if name in os.environ}
        call('UpdateActivationEnvironment', GLib.Variant('(a{ss})', (values,)), '()')
        desktop = GioUnix.DesktopAppInfo.new_from_filename(str(prefix / ('share/applications/'+APP_ID+'.desktop')))
        assert desktop and desktop.get_boolean('DBusActivatable')
        assert not has_owner(), 'Private application already running before cold activation'
        start = time.monotonic()
        assert desktop.launch([], None)
        first = peer(); peers.append(first)
        client = Client(first)
        client.wait('cold activated conversation', lambda: client.text('Launcher response'))
        ready_ms = round((time.monotonic()-start)*1000)
        composer = client.wait('composer', lambda: client.find(lambda n:
            n.get_role_name() == 'text' and n.get_name().startswith('Write a message.')))
        client.edit(composer, DRAFT)
        providers = provider_ids()
        assert desktop.launch([Gio.File.new_for_path(str(source))], None)
        client.wait('open forwarded document', lambda: client.contains_text('Desktop activation document 中文😀'))
        assert peer().pid == first.pid, 'File activation created another application'
        assert client.contents(composer) == DRAFT
        client.click('Close document')
        client.wait('conversation restored', lambda: client.text('Launcher response'))
        assert client.contents(composer) == DRAFT
        phases.append(dict(coldActivate=True, serviceMode=True, readyMs=ready_ms, liveOpenSameProcess=True,
            draftRetained=True, pid=first.pid))
        close(client, first, providers)
        assert not has_owner()

        # Cold Open must initialize the real application and render the file,
        # rather than requiring an already registered desktop window.
        start = time.monotonic()
        assert desktop.launch([Gio.File.new_for_path(str(source))], None)
        second = peer(); peers.append(second)
        assert second.pid != first.pid or second.started != first.started
        client = Client(second)
        client.wait('cold opened document', lambda: client.contains_text('Desktop activation document 中文😀'))
        ready_ms = round((time.monotonic()-start)*1000)
        client.click('Source')
        client.wait('exact original Source', lambda: client.find(lambda n:
            client.showing(n) and n.get_role_name() == 'text' and client.contents(n) == SOURCE))
        client.click('Preview')
        client.wait('Preview restored', lambda: client.contains_text('Desktop activation document 中文😀'))
        phases.append(dict(coldOpen=True, serviceMode=True, readyMs=ready_ms, exactSource=True,
            previewSourceSwitch=True, pid=second.pid))
        providers = provider_ids()
        close(client, second, providers)
        assert all(digest(path)==value for path,value in originals.items())
        assert not history.runtime.starts(root), 'Desktop activation started model inference'
        result = dict(ok=True, installedPair=True, applicationHooks=False,
            gatewayOverride=False, coldDbusActivate=True, coldDbusOpen=True,
            specialCharacterPrefix=True, specialCharacterDocument=True,
            sourcePreserved=True, noInference=True, ownedProcessCleanup=True,
            physicalGnomeShell=False, phases=phases, artifacts=str(root))
        (root/'desktop-activation-result.json').write_text(json.dumps(result, indent=2)+'\n')
        print(json.dumps(result))
    except BaseException:
        if 'client' in locals() and client.process.poll() is None:
            (root/'failed-tree.json').write_text(json.dumps(client.describe(), ensure_ascii=False, indent=2))
        raise
    finally:
        # Only peers obtained from this owned bus and validated against the
        # installed executable may receive cleanup signals on a failed test.
        for process in peers:
            if process.poll() is None:
                os.kill(process.pid, 15)
        owned.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('bundle', type=Path)
    parser.add_argument('--private-root', type=Path)
    args = parser.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE, (0,0))
    if args.private_root:
        return run(args.bundle, args.private_root)
    base = Path(os.environ.get('STUDIO_NATIVE_QA_DIR') or REPO/'target/gnome-native-smoke')
    base.mkdir(parents=True, exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix='desktop-activation-', dir=base))
    history.runtime.create_profile(root)
    prefix = root / 'installed desktop 中文 % $ ` " \\'
    subprocess.run(['bash', str(args.bundle.resolve()/'install.sh'), str(args.bundle.resolve()), str(prefix)], check=True, stdout=subprocess.DEVNULL)
    prefs = root/'config/codex-thread-studio/settings.json'
    settings = json.loads(prefs.read_text()); settings['language']='en-US'; prefs.write_text(json.dumps(settings))
    (prefs.parent/'gnome.json').write_text(json.dumps(dict(selectedBackend='codex', selectedSession='target', windowWidth=1280, windowHeight=900)))
    source = root/'project/desktop launch 中文 % $ ` " \\.md'
    source.write_bytes(SOURCE.encode())
    (root/'codex-turns.json').write_text(json.dumps(dict(target=[dict(id='turn-1', status='completed', items=[dict(id='user-1', type='userMessage', content=[dict(type='text', text='Launcher question')]), dict(id='answer-1', type='agentMessage', text='Launcher response')])], other=[])))
    (root/'oc-turns.json').write_text('{}')
    for name, value in zip(['fake-codex.py','fake-opencode.py'], history.providers()):
        path=root/name; path.write_text(value); path.chmod(0o700)
    env = dict(os.environ, CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),
        CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),
        CODEX_THREAD_STUDIO_BACKENDS_CONFIG=str(prefs.parent/'backends.json'),
        CODEX_HOME=str(root/'codex'), XDG_CONFIG_HOME=str(root/'config'),
        XDG_DATA_HOME=str(prefix/'share'), XDG_DATA_DIRS=str(prefix/'share')+':/usr/local/share:/usr/share',
        XDG_CACHE_HOME=str(root/'cache'), READER_CACHE_DIR=str(root/'reader-cache'),
        GDK_BACKEND='wayland', GTK_A11Y='atspi', GIO_USE_VFS='local', GIO_USE_VOLUME_MONITOR='unix')
    # Keep actual profile/data private while installed D-Bus definitions remain
    # discoverable in XDG_DATA_DIRS. No user-installed desktop entry is used.
    env['XDG_DATA_HOME']=str(root/'data')
    for key in list(env):
        if key.startswith('STUDIO_GNOME_SMOKE') or key in ['CODEX_THREAD_STUDIO_GATEWAY_BIN', 'STUDIO_GNOME_CONTINUOUS_PREVIEW']:
            env.pop(key)
    with isolated_environment(env, headless=True) as private:
        subprocess.run(['dbus-run-session', '--', shutil.which('mutter') or 'mutter',
            '--headless','--wayland','--no-x11','--wayland-display','studio-test',
            '--virtual-monitor','1600x1000','--',sys.executable,str(Path(__file__).resolve()),
            str(prefix),'--private-root',str(root)], cwd=root, env=private, check=True, timeout=180)


if __name__=='__main__':
    main()
