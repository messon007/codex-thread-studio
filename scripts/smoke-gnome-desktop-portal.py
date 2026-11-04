#!/usr/bin/env python3
"""Actual Ubuntu GNOME/Nautilus choosers on an owned Wayland desktop.

No portal stub or app test hooks. Only generated/copied files and fake providers
are used. The host desktop, notification service and provider sessions are unused.
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
from native_accessibility import Client, ProcessPeer as Peer
from private_wayland import OwnedMutterInput, isolated_environment
from memory_sampler import ProcessMemory
DRAFT = 'Unsent real GNOME portal draft 中文😀 '


def module(name, filename):
    spec = importlib.util.spec_from_file_location(name, REPO / 'scripts' / filename)
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


runtime = module('desktop_portal_runtime', 'smoke-gnome-router-runtime.py')
outputs = module('desktop_portal_outputs', 'smoke-gnome-outputs.py')


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(native, gateway, root):
    from gi.repository import Gio, GLib
    bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
    controlled = OwnedMutterInput()
    log = (root / 'app.log').open('w')
    child = subprocess.Popen([native], cwd=root, env=os.environ, stdout=log, stderr=log)
    memory = ProcessMemory(child.pid, gateway, include_web_content=True)
    desktop = Client(child)
    chooser = None
    step = 'startup'
    services = {}
    cases = []
    chooser_calls = []
    original_settings = json.loads((root / "config/codex-thread-studio/settings.json").read_text())

    def wait(name, test):
        nonlocal step
        step = name
        (root / 'step.txt').write_text(name)
        return desktop.wait(name, test, 35)

    def keys(values):
        for key in values:
            controlled.call('NotifyKeyboardKeysym', GLib.Variant('(ub)', (key, True)))
        time.sleep(.06)
        for key in reversed(values):
            controlled.call('NotifyKeyboardKeysym', GLib.Variant('(ub)', (key, False)))

    def owner(name, executable):
        unique = bus.call_sync('org.freedesktop.DBus', '/org/freedesktop/DBus',
            'org.freedesktop.DBus', 'GetNameOwner', GLib.Variant('(s)', (name,)),
            GLib.VariantType.new('(s)'), Gio.DBusCallFlags.NONE, 3000, None).unpack()[0]
        pid = bus.call_sync('org.freedesktop.DBus', '/org/freedesktop/DBus',
            'org.freedesktop.DBus', 'GetConnectionUnixProcessID', GLib.Variant('(s)', (unique,)),
            GLib.VariantType.new('(u)'), Gio.DBusCallFlags.NONE, 3000, None).unpack()[0]
        peer = Peer(pid, executable)
        services[name] = dict(owner=unique, pid=pid, executable=executable)
        return peer

    def dialog(title):
        nonlocal chooser, step
        step = 'GNOME chooser: ' + title
        (root / 'step.txt').write_text(step)
        until = time.monotonic() + 30
        while time.monotonic() < until:
            try:
                peer = owner('org.gnome.Nautilus', '/usr/bin/nautilus')
                chooser = Client(peer)
                node = chooser.find(lambda n: n.get_name() == title
                                    and n.get_role_name() == 'frame' and chooser.showing(n))
                if node:
                    for name, path in [('org.freedesktop.portal.Desktop', '/usr/libexec/xdg-desktop-portal'),
                        ('org.freedesktop.impl.portal.desktop.gnome', '/usr/libexec/xdg-desktop-portal-gnome')]:
                        owner(name, path)
                    chooser_calls.append(title)
                    return node
            except GLib.Error:
                pass
            if child.poll() is not None:
                raise RuntimeError(('Native app exited while choosing', child.returncode))
            time.sleep(.05)
        raise RuntimeError(('Real GNOME chooser not found', title))

    def closed(title):
        wait('chooser closed: ' + title, lambda: not chooser.find(lambda n:
            n.get_name() == title and n.get_role_name() == 'frame' and chooser.showing(n)))

    def location(path):
        keys([0xffe3, ord('l')])
        field = chooser.wait('Nautilus location entry', lambda: chooser.find(lambda n:
            chooser.showing(n) and n.get_editable_text_iface()))
        chooser.edit(field, str(path))
        keys([0xff0d])

    def read_text(value):
        return desktop.contains_text(value)

    def activate_row(row):
        # GtkActionRow may expose keyboard activation without an AT-SPI action.
        # Use the ordinary focus chain on our verified compositor only.
        for _ in range(80):
            focused = desktop.find(lambda n: n.get_state_set().contains(desktop.api.StateType.FOCUSED))
            if focused and desktop.within(focused, row):
                keys([0xff0d])
                return
            keys([0xff09])
        raise RuntimeError(('Native preference row not keyboard reachable', row.get_name()))

    def close_preferences(title):
        current = wait(title + ' preferences', lambda: desktop.find(lambda n:
            n.get_name() == title and n.get_role_name() == 'dialog' and desktop.showing(n)))
        close = wait(title + ' close control', lambda: desktop.find(lambda n:
            n.get_name() == 'Close' and n.get_role_name() == 'button'
            and desktop.showing(n) and desktop.within(n, current)))
        desktop.activate(close)
        wait(title + ' preferences closed', lambda: not desktop.showing(current))

    def still_draft():
        assert desktop.contents(composer) == DRAFT, 'Chooser changed unsent draft'

    try:
        wait('native app', desktop.application)
        controlled.focus_only_window()
        composer = wait('native composer', lambda: desktop.find(lambda n:
            n.get_name().startswith('Write a message.') and desktop.showing(n)
            and n.get_editable_text_iface()))
        desktop.edit(composer, DRAFT)
        desktop.click('Open document')
        dialog('Open document')
        chooser.click('Close')
        closed('Open document')
        still_draft()
        cases.append('openCancel')

        desktop.click('Open document')
        dialog('Open document')
        location(root / 'project/selected 中文 #.md')
        chooser.click('Open')
        closed('Open document')
        wait('selected Markdown rendered', lambda: read_text('Real portal selection'))
        still_draft()
        cases.append('unicodeMarkdown')
        desktop.click('Close document')

        # Real folder selection must remain an in-memory edit until explicit Save.
        desktop.click('Main menu')
        desktop.click('Settings…')
        page = wait('Files preferences page', lambda: desktop.find(lambda n:
            n.get_name() == 'Files' and n.get_role_name() == 'page tab' and desktop.showing(n)))
        desktop.activate(page)
        row = wait('Shared folders preference row', lambda: desktop.find(lambda n:
            n.get_name() == 'Shared folders' and n.get_role_name() == 'list item' and desktop.showing(n)))
        activate_row(row)
        desktop.click('Choose folder')
        dialog('Add shared folder')
        chooser.click('Close')
        closed('Add shared folder')
        cases.append('folderCancel')
        desktop.click('Choose folder')
        dialog('Add shared folder')
        location(root / 'shared folder 中文 #')
        chooser.click('Select')
        closed('Add shared folder')
        settings = root / 'config/codex-thread-studio/settings.json'
        assert str(root / 'shared folder 中文 #') not in json.loads(settings.read_text()).get('sharedDocumentDirectories', [])
        desktop.click('Save')
        wait('shared folder explicitly saved', lambda: str(root / 'shared folder 中文 #') in json.loads(settings.read_text()).get('sharedDocumentDirectories', []))
        still_draft()
        cases.append('unicodeFolderExplicitSave')
        close_preferences('Shared folders')
        close_preferences('Settings')
        wait('settings closed', lambda: desktop.find(lambda n: n.get_name() == 'Open document'
            and desktop.showing(n) and desktop.sensitive(n)))

        desktop.click('Open document')
        dialog('Open document')
        location(root / 'project/source 中文 #.png')
        chooser.click('Open')
        closed('Open document')
        desktop.click('Save original image…')
        dialog('Save original image')
        chooser.click('Close')
        closed('Save original image')
        assert not list((root / 'exports').iterdir()), 'Cancelling Save wrote a file'
        cases.append('imageSaveCancel')
        desktop.click('Save original image…')
        dialog('Save original image')
        location(root / 'exports')
        chooser.wait('private export directory', lambda: chooser.find(lambda n:
            n.get_name() == 'exports' and n.get_role_name() == 'button' and chooser.showing(n)))
        # GNOME 50 represents the suggested filename as a button/popover. Retain
        # that Unicode filename; changing its editor is outside app acceptance.
        chooser.wait('suggested original filename', lambda: chooser.find(lambda n:
            n.get_name() == 'source 中文 #.png' and n.get_role_name() == 'button' and chooser.showing(n)))
        (root / 'save-controls.json').write_text(json.dumps(chooser.describe(), indent=2))
        chooser.click('Save')
        closed('Save original image')
        saved = root / 'exports/source 中文 #.png'
        wait('exact original image saved', lambda: saved.exists() and digest(saved) == digest(root / 'project/source 中文 #.png'))
        still_draft()
        cases.append('unicodeOriginalImageSave')
        desktop.click('Close image')
        updated_settings = json.loads(settings.read_text())
        assert {k:v for k,v in updated_settings.items() if k != 'sharedDocumentDirectories'} == {
            k:v for k,v in original_settings.items() if k != 'sharedDocumentDirectories'}, 'Unrelated settings changed'
        assert len(chooser_calls) == 7, chooser_calls
        assert not runtime.starts(root), 'Real chooser started a model turn'
        for relative, expected in json.loads((root / 'source-hashes.json').read_text()).items():
            assert digest(root / relative) == expected, relative
        desktop.click('Close')
        child.wait(timeout=20)
        assert child.returncode == 0
    except BaseException:
        (root / 'failure.json').write_text(json.dumps(dict(step=step,
            native=desktop.describe() if child.poll() is None else [],
            chooser=chooser.describe() if chooser else [], services=services), indent=2))
        raise
    finally:
        if child.poll() is None:
            child.terminate(); child.wait(timeout=15)
        measured = memory.close()
        controlled.close(); log.close()
    for name in ['backend.pid', 'opencode.pid']:
        if (root / name).exists():
            pid = (root / name).read_text().strip()
            deadline = time.monotonic() + 10
            while Path('/proc', pid).exists() and time.monotonic() < deadline:
                time.sleep(.05)
            assert not Path('/proc', pid).exists(), ('Owned provider survived', pid)
    assert measured['webContentProcessesObserved'] == 0
    assert not any(s in (root / 'app.log').read_text() for s in ['CRITICAL', 'panicked', 'Theme parser error'])
    result = dict(ok=True, artifacts=str(root), cases=cases, chooserCalls=chooser_calls, services=services,
        realGnomeNautilus=True, portalStub=False, applicationHooks=False,
        draftRetained=True, sourceBytesUnchanged=True, backendCleanup=True, memory=measured)
    (root / 'result.json').write_text(json.dumps(result, indent=2))
    print(json.dumps(result))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('native'); parser.add_argument('gateway')
    parser.add_argument('--private-root')
    args = parser.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    native, gateway = str(Path(args.native).resolve()), str(Path(args.gateway).resolve())
    base = Path(os.environ.get('STUDIO_NATIVE_QA_DIR') or REPO / 'target/gnome-native-smoke').resolve()
    if args.private_root:
        root = Path(args.private_root).resolve()
        assert root.parent == base and root.name.startswith('desktop-portal-')
        for key, expected in [('XDG_CONFIG_HOME', root / 'config'),
            ('CODEX_THREAD_STUDIO_CODEX_BIN', root / 'fake-codex.py'),
            ('CODEX_THREAD_STUDIO_OPENCODE_BIN', root / 'fake-opencode.py')]:
            assert os.environ.get(key) == str(expected), ('Private profile required', key)
        return run(native, gateway, root)
    for path in ['/usr/bin/nautilus', '/usr/bin/mutter', '/usr/libexec/xdg-desktop-portal-gnome']:
        assert Path(path).exists(), 'Ubuntu GNOME runtime required: ' + path
    base.mkdir(parents=True, exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix='desktop-portal-', dir=base))
    runtime.create_profile(root)
    (root / 'raw-items.json').write_text('[]')
    (root / 'project/selected 中文 #.md').write_text('# Real portal selection\n\nUnchanged text.\n')
    (root / 'shared folder 中文 #').mkdir()
    (root / 'shared folder 中文 #/notes.md').write_text('# Shared folder notes\n')
    (root / 'exports').mkdir()
    shutil.copyfile(REPO / 'src-tauri/tests/fixtures/native-images/two-frame.png', root / 'project/source 中文 #.png')
    originals = {str(p.relative_to(root)): digest(p) for p in [root / 'project/selected 中文 #.md',
        root / 'project/source 中文 #.png', root / 'shared folder 中文 #/notes.md']}
    (root / 'source-hashes.json').write_text(json.dumps(originals))
    (root / 'config/codex-thread-studio/gnome.json').write_text(json.dumps({'selectedBackend':'codex','selectedSession':'target'}))
    for name, code in zip(['fake-codex.py', 'fake-opencode.py'], outputs.providers()):
        path = root / name; path.write_text(code); path.chmod(0o700)
    environment = dict(os.environ, CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway,
        CODEX_THREAD_STUDIO_CODEX_BIN=str(root / 'fake-codex.py'),
        CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root / 'fake-opencode.py'), CODEX_HOME=str(root / 'codex'),
        XDG_CONFIG_HOME=str(root / 'config'), XDG_DATA_HOME=str(root / 'data'), XDG_CACHE_HOME=str(root / 'cache'),
        READER_CACHE_DIR=str(root / 'reader-cache'), GDK_BACKEND='wayland', GTK_USE_PORTAL='1',
        GIO_USE_VFS='local', GIO_USE_VOLUME_MONITOR='unix', GSETTINGS_BACKEND='memory',
        XDG_CURRENT_DESKTOP='GNOME', XDG_SESSION_TYPE='wayland', LC_ALL='C.UTF-8')
    for name in list(environment):
        if name.startswith('STUDIO_GNOME_SMOKE') or name == 'STUDIO_OWNED_MUTTER_INPUT':
            environment.pop(name)
    with isolated_environment(environment, headless=True) as private:
        with (root / 'session.log').open('w') as log:
            subprocess.run(['dbus-run-session', '--', '/usr/bin/mutter', '--headless', '--wayland', '--no-x11',
                '--wayland-display', 'studio-test', '--virtual-monitor', '1600x1000', '--',
                '/usr/bin/python3', str(Path(__file__).resolve()), native, gateway, '--private-root', str(root)],
                env=private, cwd=root, stdout=log, stderr=log, check=True, timeout=230)
    result = json.loads((root / 'result.json').read_text())
    (root / 'host-packages.txt').write_text(subprocess.run(['dpkg-query', '-W', 'mutter', 'nautilus',
        'xdg-desktop-portal', 'xdg-desktop-portal-gnome'], capture_output=True, text=True, check=True).stdout)
    print(json.dumps(result))


if __name__ == '__main__':
    main()
