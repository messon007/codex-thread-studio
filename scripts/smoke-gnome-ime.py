#!/usr/bin/env python3
"""Real IBus pinyin input through an owned GNOME Shell and Wayland text-input.

The optional extracted Ubuntu engine is mounted only inside its own namespace.
No host package installation, login bus, model provider or application hook.
"""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import resource
import shlex
import shutil
import subprocess
import sys
import tempfile
import time
import xml.etree.ElementTree as ET

sys.dont_write_bytecode = True
REPO = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO/'scripts/test-support'))
from private_wayland import isolated_environment, owned_gnome_shell
from native_accessibility import Client, ProcessPeer
spec = importlib.util.spec_from_file_location('ime_desktop', REPO/'scripts/smoke-gnome-desktop-activation.py')
desktop = importlib.util.module_from_spec(spec)
spec.loader.exec_module(desktop)


def prepare_engine(root, engine_root):
    """Expose compiled-in dictionary paths without installing on the host."""
    engine_root = engine_root.resolve(strict=True)
    engine = engine_root/'usr/libexec/ibus-engine-libpinyin'
    assert engine.is_file()
    wrapper = root/'pinyin-engine.py'
    wrapper.write_text('''import os, sys
from pathlib import Path
engine_root=Path('''+repr(str(engine_root))+''')
root=Path('''+repr(str(root))+''')
runtime=Path(os.environ['XDG_RUNTIME_DIR'])
assert runtime.parent==Path('/tmp') and runtime.name.startswith('studio-wayland-')
assert os.environ['IBUS_ADDRESS']=='unix:path='+str(runtime/'ibus.sock')
command=['bwrap','--die-with-parent','--unshare-net','--ro-bind','/','/',
    '--dev-bind','/dev','/dev','--proc','/proc','--tmpfs','/usr/share']
for path in sorted(Path('/usr/share').iterdir()):
    if path.name!='ibus-libpinyin':command+=['--ro-bind',str(path),str(path)]
command+=['--ro-bind',str(engine_root/'usr/share/ibus-libpinyin'),'/usr/share/ibus-libpinyin',
    '--ro-bind',str(engine_root/'usr/lib/x86_64-linux-gnu/libpinyin'),'/usr/lib/x86_64-linux-gnu/libpinyin',
    '--bind',str(root),str(root),'--bind',str(runtime),str(runtime),
    '--setenv','LD_LIBRARY_PATH',str(engine_root/'usr/lib/x86_64-linux-gnu'),
    '--',str(engine_root/'usr/libexec/ibus-engine-libpinyin'),*sys.argv[1:]]
os.execvp(command[0],command)
''')
    component = ET.parse(engine_root/'usr/share/ibus/component/libpinyin.xml').getroot()
    component.find('exec').text = shlex.join([sys.executable, str(wrapper), '--ibus'])
    component.remove(component.find('observed-paths'))
    component.remove(component.find('engines'))
    component.append(ET.parse(engine_root/'usr/share/ibus-libpinyin/default.xml').getroot())
    components = root/'ibus-components'; components.mkdir()
    ET.ElementTree(component).write(components/'libpinyin.xml', encoding='utf-8', xml_declaration=True)
    shutil.copy('/usr/share/ibus/component/dconf.xml', components/'dconf.xml')
    schemas = root/'ime-schemas'; schemas.mkdir()
    shutil.copy(engine_root/'usr/share/glib-2.0/schemas/com.github.libpinyin.ibus-libpinyin.gschema.xml', schemas)
    subprocess.run(['glib-compile-schemas','--strict',str(schemas)],check=True)


def run(prefix, root):
    # Refuse a direct invocation on the login bus before starting even IBus or
    # writing its dconf settings. Shell's shared guard verifies this again.
    if (Path('/proc',str(os.getppid()),'exe').resolve()!=Path('/usr/bin/dbus-run-session').resolve()
            or os.environ.get('XDG_CONFIG_HOME')!=str(root/'config')
            or os.environ.get('HOME')!=str(root/'home')
            or os.environ.get('XDG_CACHE_HOME')!=str(root/'cache')):
        raise RuntimeError('IME acceptance requires its owned session bus and private profile')
    import gi
    gi.require_version('IBus','1.0')
    from gi.repository import IBus, Gio, GLib
    assert os.environ['IBUS_ADDRESS']=='unix:path='+str(Path(os.environ['XDG_RUNTIME_DIR'])/'ibus.sock')
    assert not os.environ.get('GTK_IM_MODULE'), 'Test the normal Wayland IM path, not a forced GTK module'
    assert not any(k.startswith('STUDIO_GNOME_SMOKE') for k in os.environ)
    assert 'CODEX_THREAD_STUDIO_GATEWAY_BIN' not in os.environ
    IBus.init()
    log=(root/'ibus.log').open('w')
    daemon=subprocess.Popen(['/usr/bin/ibus-daemon','--panel','disable','--emoji-extension','disable',
        '--cache','none','--address',os.environ['IBUS_ADDRESS'],'--desktop','gnome'],stdout=log,stderr=log)
    def wait(name, test, seconds=30):
        end=time.monotonic()+seconds
        while time.monotonic()<end:
            while GLib.MainContext.default().pending(): GLib.MainContext.default().iteration(False)
            value=test()
            if value:return value
            time.sleep(.04)
        raise RuntimeError(('IME timed out',name,str(root)))
    try:
        ibus=IBus.Bus.new()
        wait('owned IBus connected',ibus.is_connected)
        assert any(e.get_name()=='libpinyin' for e in ibus.list_engines()), 'Private pinyin component was not registered'
        settings=Gio.Settings.new('org.gnome.desktop.input-sources')
        assert settings.set_value('sources',GLib.Variant('a(ss)',[('ibus','libpinyin')]))
        Gio.Settings.sync()
        with owned_gnome_shell(root) as (shell,owned):
            def keys(values):
                for key in values:owned.call('NotifyKeyboardKeysym',GLib.Variant('(ub)',(key,True)))
                time.sleep(.05)
                for key in reversed(values):owned.call('NotifyKeyboardKeysym',GLib.Variant('(ub)',(key,False)))
                time.sleep(.05)
            bus=Gio.bus_get_sync(Gio.BusType.SESSION,None)
            def overview():
                return bus.call_sync(owned.target,'/org/gnome/Shell','org.freedesktop.DBus.Properties','Get',
                    GLib.Variant('(ss)',('org.gnome.Shell','OverviewActive')),GLib.VariantType.new('(v)'),Gio.DBusCallFlags.NONE,3000,None).unpack()[0]
            if overview():keys([0xff1b]);wait('overview closed',lambda:not overview())
            native=prefix/'bin/codex-thread-studio-gnome'
            trace=(root/'native-wayland.log').open('w')
            process=subprocess.Popen([str(native)],env=dict(os.environ,WAYLAND_DEBUG='client'),stdout=trace,stderr=trace)
            client=Client(process)
            try:
                client.wait('owned conversation',lambda:client.text('Launcher response'))
                composer=client.wait('composer',lambda:client.find(lambda n:n.get_role_name()=='text' and n.get_name().startswith('Write a message.')))
                owned.focus_only_window()
                client.wait('native foreground',lambda:client.find(lambda n:n.get_role_name()=='frame' and n.get_state_set().contains(client.api.StateType.ACTIVE)))
                for _ in range(60):
                    composer.clear_cache()
                    if composer.get_state_set().contains(client.api.StateType.FOCUSED):break
                    keys([0xff09])
                else:raise RuntimeError('Composer was not keyboard reachable')
                wait('pinyin global engine',lambda:ibus.get_global_engine() and ibus.get_global_engine().get_name()=='libpinyin')
                for c in 'nihao':keys([ord(c)])
                shell_client=Client(ProcessPeer(shell.pid,'/usr/bin/gnome-shell'))
                shell_client.wait('real Shell candidate popup',lambda:shell_client.contains_text('你好'))
                (root/'composing-tree.json').write_text(json.dumps(client.describe(),ensure_ascii=False,indent=2))
                keys([0x20])
                client.wait('committed Chinese in composer',lambda:'你好' in client.contents(composer))
                assert not desktop.history.runtime.starts(root),'Confirming an IME candidate sent a model turn'
                assert client.contents(composer)=='你好',repr(client.contents(composer))
                # Escape discards only the next composition, preserving committed text.
                for c in 'zaijian':keys([ord(c)])
                keys([0xff1b]);time.sleep(.3)
                assert client.contents(composer)=='你好',repr(client.contents(composer))
                assert not desktop.history.runtime.starts(root)
                client.edit(composer,'/')
                client.wait('command completion visible',lambda:client.contains_text('Tab or Enter to run'))
                keys([ord('n')])
                client.wait('completion yields to real preedit',lambda:not client.contains_text('Tab or Enter to run'))
                for c in 'ihao':keys([ord(c)])
                keys([0x20])
                client.wait('IME commit preserves slash draft',lambda:client.contents(composer)=='/你好')
                assert not desktop.history.runtime.starts(root)
                client.edit(composer,'你好')
                for c in 'nihao':keys([ord(c)])
                keys([0xff0d]);time.sleep(.3)
                assert not desktop.history.runtime.starts(root),'Enter during composition sent a model turn'
                enter_text=client.contents(composer)
                assert enter_text.startswith('你好') and '\n' not in enter_text,repr(enter_text)
                client.edit(composer,'Committed prefix ')
                for c in 'nihao':keys([ord(c)])
                keys([0xffe3,0xff0d]);time.sleep(.4)
                assert not desktop.history.runtime.starts(root),'Ctrl+Enter during composition sent a partial draft'
                assert client.contents(composer)=='Committed prefix '
                keys([0x20])
                client.wait('composition retained after shortcut',lambda:client.contents(composer)=='Committed prefix 你好')
                keys([0xffe3,0xff0d])
                sent=wait('explicit committed fake turn',lambda:desktop.history.runtime.starts(root))
                assert len(sent)==1 and sent[0]['params']['threadId']=='target'
                assert sent[0]['params']['input']==[dict(type='text',text='Committed prefix 你好')],sent[0]['params']['input']
                client.wait('composer cleared after explicit send',lambda:client.contents(composer)=='')
                # The shared Router fake persists completion but deliberately
                # returns a stale in-progress ACK. Publish its stored reply
                # through its existing event lane, as live provider streaming.
                turn=wait('stored fake completion',lambda:next((t for t in json.loads((root/'codex-turns.json').read_text())['target'] if t['id']!='turn-1'),None))
                events=[dict(method='turn/started',params=dict(threadId='target',turn=dict(id=turn['id'],status='inProgress'))),
                    dict(method='item/completed',params=dict(threadId='target',turnId=turn['id'],item=turn['items'][0])),
                    dict(method='turn/completed',params=dict(threadId='target',turn=dict(id=turn['id'],status='completed')))]
                pending=root/'navigation-events.pending';pending.write_text(json.dumps(events));pending.replace(root/'navigation-events.json')
                client.wait('acknowledged fake reply',lambda:client.contains_text('Native routed target response 中文😀'))
                assert len(desktop.history.runtime.starts(root))==1
                text=(root/'native-wayland.log').read_text()
                assert 'zwp_text_input_v3' in text and 'preedit_string' in text and 'commit_string("你好")' in text
                result=dict(ok=True,realGnomeShell=True,realIBus=True,realCandidatePopup=True,engine='libpinyin',normalWaylandInput=True,
                    forcedGtkIMModule=False,applicationHooks=False,gatewayOverride=False,candidateCommit=True,completionYieldsToComposition=True,
                    compositionCancel=True,enterDuringCompositionNoSend=True,enterCommittedText=enter_text,
                    controlEnterDuringCompositionNoSend=True,compositionRetainedAfterShortcut=True,
                    committedExplicitSend=True,fakeModelTurnCount=1,noRealInference=True,physicalDesktop=False,artifacts=str(root))
                (root/'ime-result.json').write_text(json.dumps(result,indent=2)+'\n')
                print(json.dumps(result),flush=True)
            except BaseException:
                if process.poll() is None:(root/'failed-tree.json').write_text(json.dumps(client.describe(),ensure_ascii=False,indent=2))
                raise
            finally:
                if process.poll() is None:process.terminate()
                try:process.wait(timeout=15)
                except subprocess.TimeoutExpired:process.kill();process.wait(timeout=5)
                trace.close()
                pids=[int(p.read_text()) for p in [root/'backend.pid',root/'opencode.pid'] if p.exists()]
                wait('owned provider cleanup',lambda:all(not Path('/proc',str(pid)).exists() for pid in pids))
    finally:
        if daemon.poll() is None:daemon.terminate()
        try:daemon.wait(timeout=10)
        except subprocess.TimeoutExpired:daemon.kill();daemon.wait(timeout=5)
        log.close()


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('bundle',type=Path)
    parser.add_argument('--engine-root',type=Path)
    parser.add_argument('--private-root',type=Path)
    args=parser.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE,(0,0))
    if args.private_root:return run(args.bundle,args.private_root)
    if not args.engine_root:parser.error('--engine-root is required (extracted ibus-libpinyin, libpinyin15 and libpinyin-data packages)')
    base=Path(os.environ.get('STUDIO_NATIVE_QA_DIR') or REPO/'target/gnome-native-smoke');base.mkdir(parents=True,exist_ok=True)
    root=Path(tempfile.mkdtemp(prefix='ime-',dir=base)).resolve()
    desktop.history.runtime.create_profile(root)
    prefix=root/'installed 中文';(root/'home').mkdir()
    subprocess.run(['bash',str(args.bundle.resolve()/'install.sh'),str(args.bundle.resolve()),str(prefix)],check=True,stdout=subprocess.DEVNULL)
    prepare_engine(root,args.engine_root)
    prefs=root/'config/codex-thread-studio/settings.json'
    settings=json.loads(prefs.read_text());settings.update(language='en-US',desktopNotifications=False);prefs.write_text(json.dumps(settings))
    (prefs.parent/'gnome.json').write_text(json.dumps(dict(selectedBackend='codex',selectedSession='target',windowWidth=1280,windowHeight=900)))
    (root/'codex-turns.json').write_text(json.dumps(dict(target=[dict(id='turn-1',status='completed',items=[dict(id='answer-1',type='agentMessage',text='Launcher response')])],other=[])))
    (root/'oc-turns.json').write_text('{}')
    for name,value in zip(['fake-codex.py','fake-opencode.py'],desktop.history.providers()):
        p=root/name;p.write_text(value);p.chmod(0o700)
    env=dict(os.environ,HOME=str(root/'home'),CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),
        CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),CODEX_THREAD_STUDIO_BACKENDS_CONFIG=str(prefs.parent/'backends.json'),
        CODEX_HOME=str(root/'codex'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),
        XDG_DATA_DIRS=str(prefix/'share')+':/usr/local/share:/usr/share',XDG_CACHE_HOME=str(root/'cache'),
        READER_CACHE_DIR=str(root/'reader-cache'),GDK_BACKEND='wayland',GTK_A11Y='atspi',GIO_USE_VFS='local',GIO_USE_VOLUME_MONITOR='unix',
        XDG_CURRENT_DESKTOP='GNOME',XDG_SESSION_TYPE='wayland',GSETTINGS_BACKEND='dconf',GNOME_SHELL_SESSION_MODE='user',LC_ALL='C.UTF-8',
        IBUS_COMPONENT_PATH=str(root/'ibus-components'),GSETTINGS_SCHEMA_DIR=str(root/'ime-schemas'))
    for k in list(env):
        if k.startswith('STUDIO_GNOME_SMOKE') or k in ['GTK_IM_MODULE','QT_IM_MODULE','XMODIFIERS','IBUS_ADDRESS','IBUS_ADDRESS_FILE',
            'CODEX_THREAD_STUDIO_GATEWAY_BIN','STUDIO_GNOME_CONTINUOUS_PREVIEW','XDG_SESSION_ID','XDG_SEAT','XDG_VTNR',
            'GNOME_SESSION_ID','SESSION_MANAGER','DESKTOP_SESSION']:env.pop(k,None)
    with isolated_environment(env,headless=True) as private:
        private['IBUS_ADDRESS']='unix:path='+str(Path(private['XDG_RUNTIME_DIR'])/'ibus.sock')
        result=subprocess.run(['dbus-run-session','--',sys.executable,str(Path(__file__).resolve()),str(prefix),
            '--private-root',str(root)],cwd=root,env=private,timeout=180)
        print('IME artifacts:',root,flush=True)
        result.check_returncode()


if __name__=='__main__':main()
