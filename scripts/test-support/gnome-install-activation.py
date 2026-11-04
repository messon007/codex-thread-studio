#!/usr/bin/python3
"""Exercise installed desktop/D-Bus Exec quoting on a private session bus.

Uses a disposable fake Gio.Application, never the real desktop profile/provider.
Requires system Python PyGObject and dbus-run-session, but no compositor.
"""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
from private_wayland import isolated_environment

APP_ID = 'io.github.messon007.codexthreadstudio.Gnome'


def private_bus(prefix, marker):
    import gi
    gi.require_version('GioUnix', '2.0')
    from gi.repository import Gio, GioUnix, GLib
    connection = Gio.bus_get_sync(Gio.BusType.SESSION, None)
    connection.call_sync('org.freedesktop.DBus', '/org/freedesktop/DBus',
                         'org.freedesktop.DBus', 'StartServiceByName',
                         GLib.Variant('(su)', (APP_ID, 0)), None,
                         Gio.DBusCallFlags.NONE, 10000, None)
    deadline = time.monotonic() + 10
    while not marker.exists():
        assert time.monotonic() < deadline, 'Installed service did not start'
        time.sleep(.025)
    result = json.loads(marker.read_text())
    assert result['executable'] == str(prefix / 'bin/codex-thread-studio-gnome'), result
    assert '--gapplication-service' in result['argv'], result
    connection.call_sync(APP_ID, '/' + APP_ID.replace('.', '/'),
                         'org.freedesktop.Application', 'ActivateAction',
                         GLib.Variant('(sava{sv})', ('quit', [], {})), None,
                         Gio.DBusCallFlags.NONE, 5000, None)
    while connection.call_sync('org.freedesktop.DBus', '/org/freedesktop/DBus',
                              'org.freedesktop.DBus', 'NameHasOwner',
                              GLib.Variant('(s)', (APP_ID,)), None,
                              Gio.DBusCallFlags.NONE, 5000, None).unpack()[0]:
        assert time.monotonic() < deadline, 'Fixture service survived quit'
        time.sleep(.025)
    # Exercise the installed Exec fallback too, through GLib's actual Desktop
    # Entry parser, with a file argument. Only the temporary copy disables D-Bus.
    marker.unlink()
    desktop = prefix / f'share/applications/{APP_ID}.desktop'
    fallback = marker.parent / 'fallback.desktop'
    fallback.write_text(desktop.read_text().replace('DBusActivatable=true', 'DBusActivatable=false'))
    source = marker.parent / 'document space % $ ` " \\.md'
    source.write_text('# Private launch fixture\n')
    info = GioUnix.DesktopAppInfo.new_from_filename(str(fallback))
    assert info is not None, 'Installed desktop entry could not be parsed'
    assert info.launch([Gio.File.new_for_path(str(source))], None)
    deadline = time.monotonic() + 10
    while not marker.exists():
        assert time.monotonic() < deadline, 'Desktop Exec fallback did not start'
        time.sleep(.025)
    result = json.loads(marker.read_text())
    assert result['executable'] == str(prefix / 'bin/codex-thread-studio-gnome'), result
    assert result['argv'][1:] == [str(source)], result
    connection.call_sync(APP_ID, '/' + APP_ID.replace('.', '/'),
                         'org.freedesktop.Application', 'ActivateAction',
                         GLib.Variant('(sava{sv})', ('quit', [], {})), None,
                         Gio.DBusCallFlags.NONE, 5000, None)
    while connection.call_sync('org.freedesktop.DBus', '/org/freedesktop/DBus',
                              'org.freedesktop.DBus', 'NameHasOwner',
                              GLib.Variant('(s)', (APP_ID,)), None,
                              Gio.DBusCallFlags.NONE, 5000, None).unpack()[0]:
        assert time.monotonic() < deadline, 'Desktop fallback survived quit'
        time.sleep(.025)


def main():
    if len(sys.argv) > 1 and sys.argv[1] == '--private':
        private_bus(Path(sys.argv[2]), Path(sys.argv[3]))
        return
    project = Path(__file__).resolve().parents[2]
    base = Path(os.environ.get('STUDIO_NATIVE_QA_DIR') or project / 'target/gnome-native-smoke')
    base.mkdir(parents=True, exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix='notification-install-', dir=base))
    bundle = root / 'bundle'
    bundle.mkdir()
    # Characters are literal path components, never shell code.
    prefix = root / 'prefix space % $ ` " \\'
    executable = bundle / 'codex-thread-studio-gnome'
    executable.write_text('''#!/usr/bin/python3
import json,os,sys
from gi.repository import Gio,GLib
app=Gio.Application(application_id="''' + APP_ID + '''",flags=Gio.ApplicationFlags.HANDLES_OPEN)
def startup(app):
    action=Gio.SimpleAction.new("quit",None)
    action.connect("activate",lambda *_:app.quit())
    app.add_action(action)
    app.hold()
    with open(os.environ["STUDIO_INSTALL_ACTIVATION_MARKER"],"w") as f:
        json.dump({"executable":os.path.abspath(__file__),"argv":sys.argv},f)
    GLib.timeout_add_seconds(15,lambda:app.quit())
app.connect("startup",startup)
app.connect("activate",lambda _:None)
app.connect("open",lambda *_:None)
app.run(sys.argv)
''')
    executable.chmod(0o755)
    for name in ['codex-thread-studio-gateway', 'codex-thread-studio-mermaid']:
        shutil.copy2(executable, bundle / name)
    for suffix in ['desktop', 'service']:
        shutil.copyfile(project / f'native/gnome/data/{APP_ID}.{suffix}', bundle / f'{APP_ID}.{suffix}')
    # The installer copies the icon; image decoding is outside this fixture.
    (bundle / f'{APP_ID}.png').write_bytes(b'private installer fixture')
    (bundle / 'licenses').mkdir()
    shutil.copyfile(project / 'LICENSE', bundle / 'licenses/LICENSE')
    (bundle / 'MERMAID-WORKER.json').write_text(json.dumps({'privateActivationFixture': True}))
    subprocess.run(['bash', str(project / 'scripts/package-gnome-checksums.sh'), str(bundle)], check=True)
    subprocess.run(['bash', str(project / 'scripts/install-gnome.sh'), str(bundle), str(prefix)], check=True)
    desktop = (prefix / f'share/applications/{APP_ID}.desktop').read_text()
    assert 'DBusActivatable=true' in desktop and 'X-GNOME-UsesNotifications=true' in desktop
    marker = root / 'activation.json'
    env = dict(os.environ, XDG_DATA_HOME=str(prefix / 'share'),
               XDG_CONFIG_HOME=str(root / 'config'), XDG_CACHE_HOME=str(root / 'cache'),
               HOME=str(root / 'home'), GIO_USE_VFS='local',
               STUDIO_INSTALL_ACTIVATION_MARKER=str(marker))
    with isolated_environment(env, headless=True) as private:
        subprocess.run(['dbus-run-session', '--', '/usr/bin/python3', str(Path(__file__).resolve()),
                        '--private', str(prefix), str(marker)], env=private, check=True, timeout=25)
    print(json.dumps({'ok': True, 'installedDbusActivation': True,
                      'desktopExecFallback': True, 'literalSpecialCharacters': True, 'artifacts': str(root)}))


if __name__ == '__main__':
    main()
