#!/usr/bin/env python3
"""Native turn navigation through owned Wayland/AT-SPI input, without app hooks."""
import argparse, importlib.util, json, os, resource, shutil, subprocess, sys, tempfile, time
from pathlib import Path
sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).with_name('test-support')))
from private_wayland import OwnedMutterInput, isolated_environment
from native_accessibility import Client
spec = importlib.util.spec_from_file_location('navigation_continue', Path(__file__).with_name('smoke-gnome-continuation.py'))
fixture = importlib.util.module_from_spec(spec); spec.loader.exec_module(fixture)
runtime = fixture.runtime
DRAFT = 'Turn navigator retains my draft 中文😀 '

def run(native, root):
    capture = os.environ.get('STUDIO_NAVIGATION_CAPTURE')=='1'
    assert (capture and os.environ.get('STUDIO_GNOME_SMOKE_CAPTURE_ONLY')=='1' and os.environ.get('STUDIO_GNOME_SMOKE_DIR')==str(root)) or not any(k.startswith('STUDIO_GNOME_SMOKE') for k in os.environ)
    owned = OwnedMutterInput()
    def keys(values):
        for k in values: owned.call('NotifyKeyboardKeysym', owned.GLib.Variant('(ub)', (k, True)))
        time.sleep(.04)
        for k in reversed(values): owned.call('NotifyKeyboardKeysym', owned.GLib.Variant('(ub)', (k, False)))
        time.sleep(.04)
    log = (root/'app.log').open('w')
    child = subprocess.Popen([native], cwd=root, env=os.environ, stdout=log, stderr=log)
    client = Client(child)
    original_source = (root/'codex-turns.json').read_bytes()
    try:
        client.wait('latest answer', lambda: client.text('Answer 15'))
        owned.focus_only_window()
        client.wait('active owned window', lambda: client.find(lambda n: n.get_role_name()=='frame' and n.get_state_set().contains(client.api.StateType.ACTIVE)))
        composer = client.wait('composer', lambda: client.find(lambda n: n.get_role_name()=='text' and n.get_name().startswith('Write a message.')))
        client.edit(composer, DRAFT)
        def open_menu():
            client.click('Turn navigation')
            return client.wait('turn menu', lambda: client.text('Turns in this page'))
        def selected_name():
            # GtkListView rows have no accessible name of their own; the bound
            # prompt Label is their named child. Ignore the sidebar selection.
            for label in client.tree():
                if not client.showing(label) or not label.get_name().startswith('User input '): continue
                node=label
                for _ in range(4):
                    if node.get_role_name()=='list item' and node.get_state_set().contains(client.api.StateType.SELECTED): return label.get_name()
                    node=node.get_parent()
                    if node is None: break
            return ''
        def prompt_visible(index):
            text='Prompt '+str(index).zfill(2)+' 中文😀 literal {index}'
            for node in client.tree():
                if not client.showing(node) or node.get_role_name() not in ('label','text'): continue
                if node.get_name()!=text and client.contents(node)!=text: continue
                parent=node.get_parent()
                while parent and parent.get_role_name()!='scroll pane': parent=parent.get_parent()
                if parent is None: continue
                bounds=client.api.Component.get_extents(node.get_component_iface(),client.api.CoordType.WINDOW)
                viewport=client.api.Component.get_extents(parent.get_component_iface(),client.api.CoordType.WINDOW)
                if bounds.width>0 and bounds.height>0 and bounds.y+bounds.height>viewport.y and bounds.y<viewport.y+viewport.height: return node
        def choose(index):
            (root/'phase').write_text('choose '+str(index))
            prompt=client.wait('mapped prompt',lambda:client.find(lambda n:client.showing(n) and n.get_name().startswith('User input ')))
            target=prompt
            while target.get_role_name()!='list': target=target.get_parent()
            before=client.find(lambda n:n.get_state_set().contains(client.api.StateType.FOCUSED))
            (root/'focus-before.json').write_text(json.dumps(dict(name=before.get_name() if before else None,role=before.get_role_name() if before else None)))
            for _ in range(20):
                if client.find(lambda n:n.get_state_set().contains(client.api.StateType.FOCUSED) and client.within(n,target)): break
                keys([0xff09])
            else: raise RuntimeError('Prompt list is not keyboard reachable')
            # Materialize a virtualized prompt row through normal list keys.
            keys([0xffe3,0xff50])
            for _ in range(index-1): keys([0xff54])
            name = 'User input '+str(index)+': Prompt '+str(index).zfill(2)+' 中文😀 literal {index}'
            node = client.wait('prompt '+str(index), lambda: client.find(lambda n: client.showing(n) and n.get_name()==name))
            for _ in range(5):
                interface=node.get_action_iface()
                if interface and any(client.api.Action.get_action_name(interface,i) in ('activate','click') for i in range(client.api.Action.get_n_actions(interface))):
                    client.activate(node); break
                node=node.get_parent()
            else: raise RuntimeError('Prompt row has no native action')
            client.wait('turn menu dismissed', lambda: not client.text('Turns in this page'))
            client.wait('target prompt visible', lambda: prompt_visible(index))
            assert client.contents(composer)==DRAFT
        open_menu()
        client.wait('last turn is current', lambda: 'User input 15:' in selected_name())
        layout={}
        if capture:
            heading=client.text('Turns in this page');content=heading.get_parent()
            bounds=client.api.Component.get_extents(content.get_component_iface(),client.api.CoordType.WINDOW)
            frame=client.find(lambda n:n.get_role_name()=='frame')
            window=client.api.Component.get_extents(frame.get_component_iface(),client.api.CoordType.WINDOW)
            assert 0<bounds.width<=window.width and 0<bounds.height<=window.height,(bounds.width,bounds.height,window.width,window.height)
            layout=dict(popupWidth=bounds.width,popupHeight=bounds.height,windowWidth=window.width,windowHeight=window.height)
            heading_bounds=client.api.Component.get_extents(heading.get_component_iface(),client.api.CoordType.WINDOW)
            prompt_label=client.find(lambda n:client.showing(n) and n.get_role_name()=='label' and n.get_name().startswith('User input '))
            prompt_bounds=client.api.Component.get_extents(prompt_label.get_component_iface(),client.api.CoordType.WINDOW)
            layout.update(headingHeight=heading_bounds.height,promptLabelHeight=prompt_bounds.height)
            for request,name in [('native-turn-capture-request','turn-popup'),('native-capture-request','turn-window')]:
                (root/request).write_text(name)
                client.wait('mapped capture '+name,lambda name=name:(root/(name+'.png')).exists() or (root/(name+'.error')).exists())
                assert not (root/(name+'.error')).exists(),(root/(name+'.error')).read_text() if (root/(name+'.error')).exists() else ''
        choose(1)
        open_menu()
        client.wait('first turn is current', lambda: 'User input 1:' in selected_name())
        choose(8)
        open_menu()
        choose(15)
        # Keyboard activation follows the same scoped native list action.
        open_menu()
        keys([0xffe3,0xff50]); keys([0xff0d])
        client.wait('keyboard navigation dismissed menu', lambda: not client.text('Turns in this page'))
        client.wait('keyboard first prompt', lambda: prompt_visible(1))
        assert client.contents(composer)==DRAFT
        assert not runtime.starts(root), 'Reading navigation sent inference'
        assert (root/'codex-turns.json').read_bytes()==original_source, 'Reading navigation changed source history'
        (root/'accessible-final.json').write_text(json.dumps(client.describe(), ensure_ascii=False, indent=2))
        keys([0xffe9,0xffc1]); child.wait(timeout=20); assert child.returncode==0
        for name in ['backend.pid','opencode.pid']:
            path=root/name
            if path.exists(): runtime.until(lambda: not Path('/proc',path.read_text().strip()).exists(), seconds=10)
        raw=(root/'app.log').read_text()
        assert not any(token in raw for token in ['CRITICAL','Gtk-WARNING','panicked']), raw[-1500:]
        result=dict(ok=True,noApplicationHooks=not capture,passiveCapture=capture,layout=layout,installedPair=os.environ.get('STUDIO_NAVIGATION_INSTALLED')=='1',gatewayOverride='CODEX_THREAD_STUDIO_GATEWAY_BIN' in os.environ,promptPreviews=True,
                    directJumps=True,currentFirstAndLast=True,keyboardActivation=True,
                    literalPromptPreserved=True,draftPreserved=True,noInference=True,
                    sourceHistoryUnchanged=True,
                    providerCleanup=True,artifacts=str(root))
        (root/'navigation-result.json').write_text(json.dumps(result,indent=2)+'\n')
        print(json.dumps(result))
    except BaseException:
        (root/'failed-tree.json').write_text(json.dumps(client.describe(),ensure_ascii=False,indent=2))
        raise
    finally:
        if child.poll() is None: child.terminate(); child.wait(timeout=15)
        owned.close()
        log.close()

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('native');p.add_argument('gateway');p.add_argument('--private-root',type=Path);p.add_argument('--capture',action='store_true');p.add_argument('--narrow',action='store_true');p.add_argument('--large-type',action='store_true');p.add_argument('--installed',action='store_true');a=p.parse_args()
    if a.installed and a.capture:p.error('Passive captures require the debug build; use a separate installed acceptance')
    resource.setrlimit(resource.RLIMIT_CORE,(0,0))
    native,gateway=str(Path(a.native).resolve()),str(Path(a.gateway).resolve())
    if a.private_root: return run(native,a.private_root)
    base=Path(os.environ.get('STUDIO_NATIVE_QA_DIR') or Path(__file__).resolve().parents[1]/'target/gnome-native-smoke');base.mkdir(parents=True,exist_ok=True)
    root=Path(tempfile.mkdtemp(prefix='turn-navigation-',dir=base));runtime.create_profile(root)
    if a.installed:
        bundle=Path(native).parent;prefix=root/'installed native 中文 % turns'
        subprocess.run(['bash',str(bundle/'install.sh'),str(bundle),str(prefix)],check=True,stdout=subprocess.DEVNULL)
        native,gateway=str(prefix/'bin/codex-thread-studio-gnome'),str(prefix/'bin/codex-thread-studio-gateway')
    raw=json.loads((root/'config/codex-thread-studio/settings.json').read_text());raw['language']='en-US';(root/'config/codex-thread-studio/settings.json').write_text(json.dumps(raw))
    if a.large_type:
        raw['typography']=dict(uiFontFamily='DejaVu Sans',uiFontSize=20,uiFontWeight=400,contentFontFamily='DejaVu Sans',contentFontSize=24,contentFontWeight=400,workspaceFontFamily='DejaVu Sans',workspaceFontSize=20,codeFontFamily='monospace',codeFontSize=20,codeFontWeight=400,highContrast=False)
        (root/'config/codex-thread-studio/settings.json').write_text(json.dumps(raw))
    preferences=dict(selectedBackend='codex',selectedSession='target')
    if a.narrow:
        preferences.update(windowWidth=640,windowHeight=900)
    (root/'config/codex-thread-studio/gnome.json').write_text(json.dumps(preferences))
    turns=[dict(id='turn-'+str(i),status='completed',items=[dict(id='user-'+str(i),type='userMessage',content=[dict(type='text',text='Prompt '+str(i).zfill(2)+' 中文😀 literal {index}')]),dict(id='answer-'+str(i),type='agentMessage',text='\n\n'.join('Answer '+str(i)+' paragraph '+str(j)+' '+'Readable context. '*12 for j in range(8)))]) for i in range(1,16)]
    (root/'codex-turns.json').write_text(json.dumps(dict(target=turns)))
    for name,value in [('fake-codex.py',fixture.codex()),('fake-opencode.py',fixture.opencode())]:
        path=root/name;path.write_text(value);path.chmod(0o700)
    env=dict(os.environ,CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway,CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),CODEX_THREAD_STUDIO_BACKENDS_CONFIG=str(root/'config/codex-thread-studio/backends.json'),CODEX_HOME=str(root/'codex'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),READER_CACHE_DIR=str(root/'reader-cache'),GDK_BACKEND='wayland',GIO_USE_VFS='local',GIO_USE_VOLUME_MONITOR='unix',GTK_A11Y='atspi')
    for key in list(env):
        if key.startswith('STUDIO_GNOME_SMOKE'): env.pop(key)
    env.pop('STUDIO_NAVIGATION_CAPTURE',None);env.pop('STUDIO_NAVIGATION_INSTALLED',None)
    if a.capture:env.update(STUDIO_NAVIGATION_CAPTURE='1',STUDIO_GNOME_SMOKE_DIR=str(root),STUDIO_GNOME_SMOKE_CAPTURE_ONLY='1')
    if a.installed:
        env.pop('CODEX_THREAD_STUDIO_GATEWAY_BIN',None);env['STUDIO_NAVIGATION_INSTALLED']='1'
    with isolated_environment(env,headless=True) as private:
        subprocess.run(['dbus-run-session','--',shutil.which('mutter') or 'mutter','--headless','--wayland','--no-x11','--wayland-display','studio-test','--virtual-monitor','1600x1000','--',sys.executable,str(Path(__file__).resolve()),native,gateway,'--private-root',str(root)],cwd=root,env=private,check=True,timeout=180)
if __name__=='__main__': main()
