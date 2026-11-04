#!/usr/bin/env python3
"""Installed GTK jq controls; owned private Wayland, no application hooks."""
import argparse
import ast
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

sys.dont_write_bytecode=True
sys.path.insert(0,str(Path(__file__).with_name('test-support')))
from private_wayland import OwnedMutterInput, isolated_environment
from native_accessibility import Client
spec=importlib.util.spec_from_file_location('query_scope',Path(__file__).with_name('smoke-gnome-turn-navigation-scope.py'))
scope=importlib.util.module_from_spec(spec);spec.loader.exec_module(scope)
runtime=scope.runtime
DRAFT='Querying JSON keeps this conversation draft 中文😀 '


def run(native,root,language):
    import gi
    gi.require_version('GioUnix','2.0')
    from gi.repository import Gio,GioUnix
    catalog=json.loads((Path(__file__).resolve().parents[1]/'native/gnome/vendor/i18n/zh-CN.json').read_text()) if language=='zh-CN' else {}
    tr=lambda value:catalog.get(value,value)
    owned=OwnedMutterInput();owned.enable_clipboard_reader()
    log=(root/'app.log').open('w')
    child=subprocess.Popen([native],cwd=root,env=os.environ,stdout=log,stderr=log)
    client=Client(child)
    originals={path:path.read_bytes() for path in (root/'project').glob('*.json*')}
    def keys(values):
        for key in values:owned.call('NotifyKeyboardKeysym',owned.GLib.Variant('(ub)',(key,True)))
        time.sleep(.04)
        for key in reversed(values):owned.call('NotifyKeyboardKeysym',owned.GLib.Variant('(ub)',(key,False)))
        time.sleep(.04)
    def click(name):return client.click(tr(name))
    def field(name):return client.find(lambda n:client.showing(n) and n.get_role_name()=='text' and n.get_name()==tr(name))
    def output():
        node=field('Query output')
        return client.contents(node) if node else None
    def focus_query():
        # Standalone Mutter may map a non-modal window without activating it
        # after AT-SPI activation (which has no real pointer input serial).
        active=lambda:client.find(lambda n:n.get_role_name()=='frame' and n.get_name()==tr('JSON query') and n.get_state_set().contains(client.api.StateType.ACTIVE))
        for _ in range(6):
            if active():return
            keys([0xffe9,0xff09])
        client.wait('owned query window active',active)
    def query(filter):
        node=client.wait('jq filter',lambda:field('jq filter'));client.edit(node,filter);click('Query')
    try:
        client.wait('conversation ready',lambda:client.text('Original conversation answer'))
        owned.focus_only_window()
        composer=client.wait('composer',lambda:client.find(lambda n:n.get_role_name()=='text' and n.get_name()==tr('Write a message. Ctrl+Enter sends; Enter starts a new line.')))
        client.edit(composer,DRAFT)
        prefix=Path(native).parent.parent
        entry=GioUnix.DesktopAppInfo.new_from_filename(str(prefix/'share/applications/io.github.messon007.codexthreadstudio.Gnome.desktop'))
        assert entry
        source=root/'project/query array 中文.json'
        assert entry.launch([Gio.File.new_for_path(str(source))],None)
        client.wait('existing structured viewer',lambda:client.text(source.name))
        click('Query JSON with jq')
        client.wait('native query dialog',lambda:field('jq filter'))
        query('.[] | select(.active) | .name')
        client.wait('first paged result',lambda:output() and 'Name 0000' in output())
        first=output();assert '中文😀' in first
        assert not field('Query output').get_state_set().contains(client.api.StateType.EDITABLE)
        click('Copy')
        client.wait('independent Wayland result copy',lambda:owned.clipboard_text()==first)
        click('Next result page')
        client.wait('next result page',lambda:output() and output()!=first)
        click('Previous result page')
        client.wait('original result page',lambda:output()==first)
        query('.[] | .value | select(. < 3)')
        client.wait('new query replaces old result',lambda:output()=='0\n\n1\n\n2\n\n')
        query('env')
        client.wait('bounded query error',lambda:client.find(lambda n:client.showing(n) and n.get_role_name()=='label' and ('env' in n.get_name() or 'undefined' in n.get_name())))
        assert output()=='0\n\n1\n\n2\n\n'
        # File activation replaces the source through the ordinary installed
        # desktop entry and must retire the previous query window/context.
        second=root/'project/query lines 中文.jsonl'
        assert entry.launch([Gio.File.new_for_path(str(second))],None)
        client.wait('old query retired on file change',lambda:not field('jq filter'))
        client.wait('new source open',lambda:client.text(second.name))
        click('Query JSON with jq');client.wait('new query dialog',lambda:field('jq filter'))
        focus_query()
        client.toggle(tr('Slurp input into an array'),keys)
        query('map(.value) | add')
        client.wait('slurped JSONL result',lambda:output()=='6\n\n')
        # The close request is sent to the focused query window, without
        # addressing any window on the host compositor.
        client.wait('query window owns focus',lambda:client.find(lambda n:n.get_role_name()=='frame' and n.get_name()==tr('JSON query') and n.get_state_set().contains(client.api.StateType.ACTIVE)))
        keys([0xffe9,0xffc1]);client.wait('query window closed',lambda:not field('jq filter'))
        assert client.contents(composer)==DRAFT
        assert all(path.read_bytes()==data for path,data in originals.items())
        assert not runtime.starts(root)
        owned.focus_only_window()
        keys([0xffe9,0xffc1]);child.wait(timeout=25);assert child.returncode==0
        assert not any(token in (root/'app.log').read_text() for token in ('CRITICAL','Gtk-WARNING','panicked'))
        result=dict(ok=True,language=language,installedPair=True,applicationHooks=False,existingStructuredViewer=True,nativeJqControls=True,pagedResults=True,independentWaylandCopy=True,slurp=True,errorKeepsResult=True,sourceChangeRetiresQuery=True,originalSourcesUnchanged=True,draftPreserved=True,noInference=True,artifacts=str(root))
        (root/'query-ui-result.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
    except BaseException:
        (root/'failed-tree.json').write_text(json.dumps(client.describe(),ensure_ascii=False,indent=2))
        states=[]
        for node in client.tree():
            state=node.get_state_set()
            if node.get_role_name()=='frame' or 'Slurp' in node.get_name() or state.contains(client.api.StateType.FOCUSED):
                interface=node.get_action_iface()
                actions=[client.api.Action.get_action_name(interface,i) for i in range(client.api.Action.get_n_actions(interface))] if interface else []
                states.append(dict(name=node.get_name(),role=node.get_role_name(),actions=actions,states=[flag for flag in ['ACTIVE','SHOWING','SENSITIVE','FOCUSABLE','FOCUSED','CHECKED'] if state.contains(getattr(client.api.StateType,flag))]))
        (root/'failed-controls.json').write_text(json.dumps(states,ensure_ascii=False,indent=2))
        raise
    finally:
        if child.poll() is None:child.terminate();child.wait(timeout=15)
        owned.close();log.close()


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('bundle',type=Path);parser.add_argument('--private-root',type=Path);parser.add_argument('--language',choices=['en-US','zh-CN'],default='en-US');args=parser.parse_args();resource.setrlimit(resource.RLIMIT_CORE,(0,0))
    if args.private_root:return run(str(args.bundle),args.private_root,args.language)
    artifacts=Path(os.environ.get('STUDIO_NATIVE_QA_DIR') or Path(__file__).resolve().parents[1]/'target/gnome-native-smoke');artifacts.mkdir(parents=True,exist_ok=True)
    root=Path(tempfile.mkdtemp(prefix='reader-query-ui-',dir=artifacts));runtime.create_profile(root)
    bundle=args.bundle.resolve();prefix=root/'installed native 中文 % query'
    subprocess.run(['bash',str(bundle/'install.sh'),str(bundle),str(prefix)],check=True,stdout=subprocess.DEVNULL)
    settings=root/'config/codex-thread-studio/settings.json';raw=json.loads(settings.read_text());raw['language']=args.language;settings.write_text(json.dumps(raw))
    (settings.parent/'gnome.json').write_text(json.dumps(dict(selectedBackend='codex',selectedSession='target',windowWidth=1400,windowHeight=950)))
    (root/'codex-turns.json').write_text(json.dumps(dict(target=[dict(id='original',status='completed',items=[dict(id='original-answer',type='agentMessage',text='Original conversation answer')])])))
    rows=[dict(name='Name %04d 中文😀 '%i+'context '*14,value=i,active=i%2==0) for i in range(1200)]
    (root/'project/query array 中文.json').write_text(json.dumps(rows,ensure_ascii=False))
    (root/'project/query lines 中文.jsonl').write_text(''.join(json.dumps(dict(value=i),ensure_ascii=False)+'\n' for i in range(4)))
    for name,code in zip(['fake-codex.py','fake-opencode.py'],scope.providers()):ast.parse(code);path=root/name;path.write_text(code);path.chmod(0o700)
    env=dict(os.environ,CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),CODEX_THREAD_STUDIO_BACKENDS_CONFIG=str(settings.parent/'backends.json'),CODEX_HOME=str(root/'codex'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),READER_CACHE_DIR=str(root/'reader-cache'),GDK_BACKEND='wayland',GIO_USE_VFS='local',GIO_USE_VOLUME_MONITOR='unix',GTK_A11Y='atspi')
    env.pop('CODEX_THREAD_STUDIO_GATEWAY_BIN',None)
    for key in list(env):
        if key.startswith('STUDIO_GNOME_SMOKE'):env.pop(key)
    with isolated_environment(env,headless=True) as private:subprocess.run(['dbus-run-session','--',shutil.which('mutter') or 'mutter','--headless','--wayland','--no-x11','--wayland-display','studio-test','--virtual-monitor','1600x1000','--',sys.executable,str(Path(__file__).resolve()),str(prefix/'bin/codex-thread-studio-gnome'),'--private-root',str(root),'--language',args.language],cwd=root,env=private,check=True,timeout=180)


if __name__=='__main__':main()
