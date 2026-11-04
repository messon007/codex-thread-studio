#!/usr/bin/env python3
"""Packaged Map controls through AT-SPI/owned Wayland input; no application hooks."""
import argparse,importlib.util,json,os,resource,shutil,sqlite3,subprocess,sys,tempfile,time
from pathlib import Path
sys.dont_write_bytecode=True
sys.path.insert(0,str(Path(__file__).with_name('test-support')))
from private_wayland import isolated_environment,OwnedMutterInput
from memory_sampler import ProcessMemory
from native_accessibility import Client
spec=importlib.util.spec_from_file_location('installed_maps',Path(__file__).with_name('smoke-gnome-router-map.py'))
fixture=importlib.util.module_from_spec(spec);spec.loader.exec_module(fixture)
def run(bundle,root):
    native=bundle/'codex-thread-studio-gnome';gateway=bundle/'codex-thread-studio-gateway'
    controlled=OwnedMutterInput();log=(root/'installed.log').open('w')
    child=subprocess.Popen([str(native)],cwd=root,env=os.environ,stdout=log,stderr=log);memory=ProcessMemory(child.pid,gateway)
    desktop=Client(child)
    def press(value):
        controlled.call('NotifyKeyboardKeysym',controlled.GLib.Variant('(ub)',(value,True)))
        time.sleep(.06)
        controlled.call('NotifyKeyboardKeysym',controlled.GLib.Variant('(ub)',(value,False)))
        time.sleep(.06)
    try:
        desktop.wait('conversation title',lambda:desktop.text('Native target'))
        controlled.focus_only_window()
        for _ in range(80):
            focused=desktop.find(lambda n:n.get_name()=='Main menu' and n.get_state_set().contains(desktop.api.StateType.FOCUSED))
            if focused:break
            press(0xff09)
        else:raise RuntimeError('Main menu is not keyboard reachable')
        press(0x20)
        desktop.wait('named menu action',lambda:desktop.find(lambda n:n.get_role_name()=='menu item' and n.get_name()=='Session Map' and desktop.showing(n)))
        assert not desktop.find(lambda n:n.get_role_name()=='menu item' and desktop.showing(n) and not n.get_name()),'Unnamed shell menu action'
        # Verify real arrow-key navigation as well as accessible activation.
        press(0xff50)
        for _ in range(32):
            if desktop.find(lambda n:n.get_role_name()=='menu item' and n.get_name()=='Session Map'
                and n.get_state_set().contains(desktop.api.StateType.FOCUSED)):break
            press(0xff54)
        else:raise RuntimeError('Session Map is not keyboard reachable in the shell menu')
        desktop.wait('focused Map menu action',lambda:desktop.find(lambda n:n.get_name()=='Session Map' and n.get_state_set().contains(desktop.api.StateType.FOCUSED)))
        press(0xff0d)
        desktop.wait('installed goal',lambda:desktop.text('Installed Map 中文😀'))
        desktop.wait('initial progress',lambda:desktop.text('0 / 2 · rev 1'))
        desktop.click('Root topic');desktop.wait('set current',lambda:desktop.text('0 / 2 · rev 2'))
        def root_done():
            for node in desktop.tree():
                if node.get_name()!='Mark done' or not desktop.showing(node) or not desktop.sensitive(node):continue
                parent=node.get_parent()
                if parent and any(parent.get_child_at_index(i).get_name()=='Root topic' for i in range(parent.get_child_count())):return node
        desktop.activate(desktop.wait('root completion button',root_done));desktop.wait('human Done',lambda:desktop.text('1 / 2 · rev 3'))
        desktop.click('Undo Map change');desktop.wait('undo completion',lambda:desktop.text('0 / 2 · rev 4'))
        desktop.click('Close Session Map');desktop.wait('pane closed',lambda:not desktop.text('Installed Map 中文😀'))
        desktop.click('Main menu');desktop.click('Settings…')
        desktop.wait('settings opened from named action',lambda:desktop.find(lambda n:n.get_role_name()=='dialog' and n.get_name()=='Settings' and desktop.showing(n)))
        assert not desktop.find(lambda n:n.get_role_name()=='menu item' and n.get_name()=='Settings…' and desktop.showing(n)),'Menu did not dismiss after opening settings'
        press(0xff1b)
        desktop.wait('settings Escape dismissal',lambda:not desktop.find(lambda n:n.get_role_name()=='dialog' and n.get_name()=='Settings' and desktop.showing(n)))
        controlled.focus_only_window()
        for key,state in [(0xffe9,True),(0xffc1,True),(0xffc1,False),(0xffe9,False)]:controlled.call('NotifyKeyboardKeysym',controlled.GLib.Variant('(ub)',(key,state)))
        child.wait(timeout=20);assert child.returncode==0
    except BaseException:
        (root/'accessibility.json').write_text(json.dumps([dict(name=n.get_name(),role=n.get_role_name()) for n in desktop.tree()],ensure_ascii=False));raise
    finally:
        if child.poll() is None:child.terminate();child.wait(timeout=10)
        measured=memory.close();controlled.close();log.close()
    with sqlite3.connect(root/'config/codex-thread-studio/session-maps.sqlite3') as database:
        assert database.execute("SELECT revision,current_item_id FROM session_maps WHERE backend='codex' AND thread_id='target'").fetchone()==(4,'root')
        assert database.execute("SELECT COUNT(*) FROM session_map_items WHERE state='done'").fetchone()==(0,)
    assert not fixture.runtime.starts(root),'Manual installed Map controls started a model turn'
    assert not any(token in (root/'installed.log').read_text() for token in ['CRITICAL','panicked','Gtk-WARNING'])
    for name in ['backend.pid','opencode.pid']:
        if (root/name).exists():fixture.runtime.until(lambda:not Path('/proc',(root/name).read_text().strip()).exists(),seconds=10)
    result=dict(ok=True,installedPair=True,noApplicationHooks=True,noGatewayOverride=True,mainMenu=True,namedMenuActions=True,menuKeyboardNavigation=True,menuDialogFocus=True,currentTopic=True,humanDone=True,undo=True,closePane=True,persistedRevision=4,modelTurns=0,memory=measured,artifacts=str(root))
    (root/'installed-map-result.json').write_text(json.dumps(result,indent=2));print(json.dumps(result))
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('bundle');parser.add_argument('--private-root');args=parser.parse_args();resource.setrlimit(resource.RLIMIT_CORE,(0,0));bundle=Path(args.bundle).resolve()
    if args.private_root:return run(bundle,Path(args.private_root))
    for name in ['codex-thread-studio-gnome','codex-thread-studio-gateway']:assert (bundle/name).is_file(),name
    base=Path(os.environ.get('STUDIO_NATIVE_QA_DIR') or Path(__file__).resolve().parents[1]/'target/gnome-native-smoke');base.mkdir(parents=True,exist_ok=True);root=Path(tempfile.mkdtemp(prefix='release-map-',dir=base));fixture.runtime.create_profile(root)
    fixture.runtime.fixture.BACKEND=fixture.codex();oc=root/'fake-opencode.py';oc.write_text(fixture.runtime.OPENCODE);oc.chmod(0o700)
    api=fixture.runtime.fixture.Gateway(bundle/'codex-thread-studio-gateway',root,'codex',oc)
    try:fixture.create_map(api,goal='Installed Map 中文😀')
    finally:api.close()
    code=root/'fake-codex.py';code.write_text(fixture.codex());code.chmod(0o700)
    (root/'config/codex-thread-studio/gnome.json').write_text(json.dumps(dict(selectedBackend='codex',selectedSession='target')))
    env=dict(os.environ,CODEX_THREAD_STUDIO_BACKENDS_CONFIG=str(root/'config/codex-thread-studio/backends.json'),CODEX_THREAD_STUDIO_CODEX_BIN=str(code),CODEX_THREAD_STUDIO_OPENCODE_BIN=str(oc),CODEX_HOME=str(root/'codex'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),READER_CACHE_DIR=str(root/'reader-cache'),GDK_BACKEND='wayland',GIO_USE_VFS='local',GIO_USE_VOLUME_MONITOR='unix',GTK_A11Y='atspi')
    for key in list(env):
        if key.startswith('STUDIO_GNOME_SMOKE') or key=='CODEX_THREAD_STUDIO_GATEWAY_BIN':env.pop(key)
    prefix=root/'installed native 中文 % map'
    subprocess.run(['bash',str(bundle/'install.sh'),str(bundle),str(prefix)],check=True,stdout=subprocess.DEVNULL)
    bundle=prefix/'bin'
    with isolated_environment(env,headless=True) as private:
        subprocess.run(['dbus-run-session','--',shutil.which('mutter') or 'mutter','--headless','--wayland','--no-x11','--wayland-display','studio-test','--virtual-monitor','1600x1000','--',sys.executable,str(Path(__file__).resolve()),str(bundle),'--private-root',str(root)],cwd=root,env=private,check=True,timeout=160)
if __name__=='__main__':main()
