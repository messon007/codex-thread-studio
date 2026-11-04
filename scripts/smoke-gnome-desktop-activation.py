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
from private_wayland import OwnedMutterInput, isolated_environment, owned_gnome_shell

spec = importlib.util.spec_from_file_location('activation_history', REPO / 'scripts/smoke-gnome-turn-navigation-scope.py')
history = importlib.util.module_from_spec(spec)
spec.loader.exec_module(history)
APP_ID = 'io.github.messon007.codexthreadstudio.Gnome'
DRAFT = 'Desktop activation draft 中文😀 '
SOURCE = '# Desktop activation document 中文😀\r\n\r\nOriginal source with spaces and symbols.\r\n'


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(prefix, root, controlled=None, shell_peer=None, interactions=False):
    import gi
    gi.require_version('GioUnix', '2.0')
    from gi.repository import Gio, GioUnix, GLib
    assert not any(key.startswith('STUDIO_GNOME_SMOKE') for key in os.environ)
    assert 'CODEX_THREAD_STUDIO_GATEWAY_BIN' not in os.environ
    owned = controlled if controlled is not None else OwnedMutterInput()
    bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
    native = prefix / 'bin/codex-thread-studio-gnome'
    source = root / 'project/desktop launch 中文 % $ ` " \\.md'
    originals = {path: digest(path) for path in [source, root/'codex-turns.json', root/'oc-turns.json']}
    peers = []
    phases = []
    focus_states = []
    shell_client = Client(shell_peer) if interactions else None
    monitor = None
    monitor_log = None
    shell_results = {}

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

    def overview():
        return bus.call_sync(owned.target, '/org/gnome/Shell',
            'org.freedesktop.DBus.Properties', 'Get',
            GLib.Variant('(ss)', ('org.gnome.Shell', 'OverviewActive')),
            GLib.VariantType.new('(v)'), Gio.DBusCallFlags.NONE, 3000, None).unpack()[0]

    def leave_overview():
        before = overview()
        if before:
            keys([0xff1b])
            until('private Shell overview closed', lambda: not overview())
        return before

    def close(client, process, providers):
        if controlled is not None:
            before = leave_overview()
            focus_states.append(dict(pid=process.pid, overviewBeforeClose=before, overviewAfterEscape=overview()))
            (root/'shell-focus.json').write_text(json.dumps(focus_states, indent=2))
        owned.focus_only_window()
        client.wait('owned focused frame', lambda: client.find(lambda n:
            n.get_role_name() == 'frame' and n.get_state_set().contains(client.api.StateType.ACTIVE)))
        keys([0xffe9, 0xffc1])
        until('native exit after Close', lambda: process.poll() is not None)
        until('D-Bus name released', lambda: not has_owner())
        until('owned provider cleanup', lambda: all(not Path('/proc', str(pid)).exists() for pid in providers))

    def launcher_search():
        if not overview():
            keys([0xffeb])
            until('Shell overview for launcher', overview)
        for char in 'codex thread studio':
            keys([ord(char)])
        shell_client.wait('installed app in Shell search', lambda: shell_client.contains_text('Codex Thread Studio (GNOME)'))
        keys([0xff0d])
        until('Shell launcher exits overview', lambda: not overview())
        shell_results['launcherSearchKeyboardActivate'] = True

    def activate_notification_center():
        keys([0xffeb,ord('v')])
        def center_notice():
            nodes=shell_client.tree()
            labels=[node for node in nodes if node.get_name()=='Work completed' and shell_client.showing(node)]
            return next((node for node in nodes if node.get_role_name()=='notification'
                and shell_client.showing(node) and any(shell_client.within(label,node) for label in labels)),None)
        target=shell_client.wait('completion in notification center',center_notice)
        assert target.get_application().get_process_id()==shell_peer.pid
        for _ in range(90):
            target.clear_cache()
            if target.get_state_set().contains(shell_client.api.StateType.FOCUSED):
                break
            keys([0xff09])
        else:
            raise RuntimeError('Notification center entry was not keyboard reachable')
        keys([0xff0d])

    def notification_controls(client, composer):
        def active():
            return client.find(lambda n:n.get_role_name()=='frame' and n.get_state_set().contains(client.api.StateType.ACTIVE))
        def counts():
            content=(root/'notification-bus.log').read_text()
            return {name:content.count('member='+name) for name in ['AddNotification','RemoveNotification']}
        def publish(turn, started=True):
            (root/'navigation-events-emitted').unlink(missing_ok=True)
            events=[]
            if started:
                events.append(dict(method='turn/started',params=dict(threadId='target',turn=dict(id=turn,status='inProgress',items=[]))))
            events.append(dict(method='turn/completed',params=dict(threadId='target',turn=dict(id=turn,status='completed',items=[]))))
            path=root/'navigation-events.pending';path.write_text(json.dumps(events));path.replace(root/'navigation-events.json')
            until('owned fake completion emitted',lambda:(root/'navigation-events-emitted').exists())
        def background():
            leave_overview()
            owned.focus_only_window()
            client.wait('native foreground before minimizing',active)
            keys([0xffeb,ord('h')])
            until('native minimized into background',lambda:not active())
            leave_overview()
        until('initial notification monitor',lambda:monitor.poll() is None)
        assert counts()['AddNotification']==0, 'History load emitted a desktop notification'
        background()
        publish('desktop-notice-1')
        until('real GIO AddNotification',lambda:counts()['AddNotification']==1)
        shell_client.wait('real Shell completion banner',lambda:shell_client.contains_text('Work completed'))
        shell_client.wait('safe session name in notification',lambda:shell_client.contains_text('Native target'))
        assert not shell_client.contains_text('Launcher response') and not shell_client.contains_text('Launcher question')
        activate_notification_center()
        client.wait('notification activation focuses native window',active)
        until('actual notification withdrawn',lambda:counts()['RemoveNotification']>=1)
        assert client.contents(composer)==DRAFT
        background()
        before=counts()['AddNotification']
        publish('desktop-notice-1',started=False)
        time.sleep(.7)
        assert counts()['AddNotification']==before, 'Duplicate completion emitted a notification'
        publish('desktop-notice-2')
        until('fresh completion still notifies',lambda:counts()['AddNotification']==before+1)
        shell_client.wait('second real Shell banner',lambda:shell_client.contains_text('Work completed'))
        leave_overview();owned.focus_only_window()
        client.wait('native restored from background',active)
        assert client.contents(composer)==DRAFT
        shell_results.update(realCompletionBanner=True,notificationKeyboardActivation=True,notificationCenterActivation=True,notificationActivationFocus=True,
            notificationWithdrawn=True,duplicateSuppressed=True,freshCompletionAfterDuplicate=True,
            notificationPrivacy=True,notificationCounts=counts())

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
        if interactions and os.environ.get('STUDIO_TEST_WAYLAND_TRACE')=='1':
            values['WAYLAND_DEBUG']='client'
        call('UpdateActivationEnvironment', GLib.Variant('(a{ss})', (values,)), '()')
        desktop = GioUnix.DesktopAppInfo.new_from_filename(str(prefix / ('share/applications/'+APP_ID+'.desktop')))
        assert desktop and desktop.get_boolean('DBusActivatable')
        assert not has_owner(), 'Private application already running before cold activation'
        if interactions:
            monitor_log=(root/'notification-bus.log').open('w')
            monitor=subprocess.Popen(['stdbuf','-oL','-eL','dbus-monitor','--session',"interface='org.gtk.Notifications'","interface='org.gtk.Actions'","interface='org.freedesktop.Application'"],stdout=monitor_log,stderr=monitor_log)
        start = time.monotonic()
        if interactions:
            launcher_search()
        else:
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
        if interactions:
            notification_controls(client,composer)
        phases.append(dict(coldActivate=True, serviceMode=True, readyMs=ready_ms, liveOpenSameProcess=True,
            draftRetained=True, pid=first.pid))
        close(client, first, providers)
        assert not has_owner()
        if interactions:
            leave_overview()
            activate_notification_center()
            restarted=peer();peers.append(restarted)
            restarted_client=Client(restarted)
            restarted_client.wait('cold notification conversation',lambda:restarted_client.text('Launcher response'))
            restarted_client.wait('cold notification window focus',lambda:restarted_client.find(lambda n:
                n.get_role_name()=='frame' and n.get_state_set().contains(restarted_client.api.StateType.ACTIVE)))
            assert restarted.pid!=first.pid or restarted.started!=first.started
            shell_results['coldNotificationActivation']=True
            phases.append(dict(coldNotificationAction=True,serviceMode=True,pid=restarted.pid))
            close(restarted_client,restarted,provider_ids())

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
        if interactions:
            final_bus=(root/'notification-bus.log').read_text()
            assert final_bus.count('member=AddNotification')==2, 'Notification count changed after the complete workflow'
            shell_results['finalAddNotificationCount']=2
        assert all(digest(path)==value for path,value in originals.items())
        assert not history.runtime.starts(root), 'Desktop activation started model inference'
        result = dict(ok=True, installedPair=True, applicationHooks=False,
            gatewayOverride=False, coldDbusActivate=True, coldDbusOpen=True,
            specialCharacterPrefix=True, specialCharacterDocument=True,
            sourcePreserved=True, noInference=True, ownedProcessCleanup=True,
            physicalGnomeShell=False, realGnomeShell=controlled is not None,
            shellOverviewHandling=focus_states, shellInteractions=shell_results, phases=phases, artifacts=str(root))
        (root/'desktop-activation-result.json').write_text(json.dumps(result, indent=2)+'\n')
        print(json.dumps(result))
    except BaseException:
        if shell_client is not None and shell_peer.poll() is None:
            (root/'failed-shell-tree.json').write_text(json.dumps(shell_client.describe(),ensure_ascii=False,indent=2))
        if 'client' in locals() and client.process.poll() is None:
            (root/'failed-tree.json').write_text(json.dumps(client.describe(), ensure_ascii=False, indent=2))
            (root/'failed-window-state.json').write_text(json.dumps([
                dict(name=n.get_name(), role=n.get_role_name(), active=n.get_state_set().contains(client.api.StateType.ACTIVE), focused=n.get_state_set().contains(client.api.StateType.FOCUSED), showing=client.showing(n), iconified=n.get_state_set().contains(client.api.StateType.ICONIFIED))
                for n in client.tree() if n.get_role_name() in ('frame', 'dialog')], ensure_ascii=False, indent=2))
        raise
    finally:
        # Only peers obtained from this owned bus and validated against the
        # installed executable may receive cleanup signals on a failed test.
        for process in peers:
            if process.poll() is None:
                os.kill(process.pid, 15)
        if monitor is not None:
            if monitor.poll() is None:
                monitor.terminate();monitor.wait(timeout=5)
            monitor_log.close()
        if controlled is None:
            owned.close()


def run_with_shell(prefix, root, interactions=False):
    from gi.repository import Gio
    with owned_gnome_shell(root) as (shell, controlled):
        if interactions:
            providers=Gio.Settings.new('org.gnome.desktop.search-providers')
            assert providers.set_boolean('disable-external',True)
            Gio.Settings.sync()
            assert providers.get_boolean('disable-external')
        run(prefix, root, controlled, ProcessPeer(shell.pid, '/usr/bin/gnome-shell'), interactions)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('bundle', type=Path)
    parser.add_argument('--private-root', type=Path)
    parser.add_argument('--gnome-shell', action='store_true', help='use an owned headless GNOME Shell rather than standalone Mutter')
    parser.add_argument('--shell-interactions', action='store_true', help='verify real Shell launcher search and notification controls; requires --gnome-shell')
    args = parser.parse_args()
    if args.shell_interactions and not args.gnome_shell:
        parser.error('--shell-interactions requires --gnome-shell')
    resource.setrlimit(resource.RLIMIT_CORE, (0,0))
    if args.private_root:
        return run_with_shell(args.bundle, args.private_root, args.shell_interactions) if args.gnome_shell else run(args.bundle, args.private_root)
    base = Path(os.environ.get('STUDIO_NATIVE_QA_DIR') or REPO/'target/gnome-native-smoke')
    base.mkdir(parents=True, exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix='desktop-activation-', dir=base))
    history.runtime.create_profile(root)
    prefix = root / 'installed desktop 中文 % $ ` " \\'
    subprocess.run(['bash', str(args.bundle.resolve()/'install.sh'), str(args.bundle.resolve()), str(prefix)], check=True, stdout=subprocess.DEVNULL)
    prefs = root/'config/codex-thread-studio/settings.json'
    settings = json.loads(prefs.read_text()); settings['language']='en-US'; settings['desktopNotifications']=args.shell_interactions; prefs.write_text(json.dumps(settings))
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
    if args.gnome_shell:
        env.update(XDG_CURRENT_DESKTOP='GNOME', XDG_SESSION_TYPE='wayland',
            GSETTINGS_BACKEND='memory', GNOME_SHELL_SESSION_MODE='user', LC_ALL='C.UTF-8')
        if args.shell_interactions:
            env['GSETTINGS_BACKEND']='dconf'
        for name in ['XDG_SESSION_ID', 'XDG_SEAT', 'XDG_VTNR', 'GNOME_SESSION_ID', 'SESSION_MANAGER', 'DESKTOP_SESSION']:
            env.pop(name, None)
    with isolated_environment(env, headless=True) as private:
        command = ['dbus-run-session', '--']
        if not args.gnome_shell:
            command += [shutil.which('mutter') or 'mutter', '--headless', '--wayland',
                '--no-x11', '--wayland-display', 'studio-test', '--virtual-monitor', '1600x1000', '--']
        command += [sys.executable, str(Path(__file__).resolve()), str(prefix), '--private-root', str(root)]
        if args.gnome_shell:
            command += ['--gnome-shell']
        if args.shell_interactions:
            command += ['--shell-interactions']
        subprocess.run(command, cwd=root, env=private, check=True, timeout=180)


if __name__=='__main__':
    main()
