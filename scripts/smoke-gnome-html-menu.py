#!/usr/bin/env python3
"""Native passive-HTML menu through AT-SPI and owned Wayland keyboard input."""
import argparse,importlib.util,json,os,resource,shutil,subprocess,sys,tempfile,time
from pathlib import Path
sys.dont_write_bytecode=True
sys.path.insert(0,str(Path(__file__).with_name('test-support')))
from native_accessibility import Client
from private_wayland import OwnedMutterInput,isolated_environment
from memory_sampler import ProcessMemory
spec=importlib.util.spec_from_file_location('html_menu_profile',Path(__file__).with_name('smoke-gnome-router-archive.py'))
fixture=importlib.util.module_from_spec(spec);spec.loader.exec_module(fixture)
SOURCE='<!doctype html>\r\n<h1>Native HTML menu</h1><p>Rendered menu text 中文😀</p><script>UNTRUSTED_SCRIPT_CANARY</script>\r\n'
DRAFT='Unsent HTML menu draft 中文😀 '

def run(native,gateway,root):
    if os.environ.get('STUDIO_HTML_MENU_INSTALLED')=='1':
        assert 'CODEX_THREAD_STUDIO_GATEWAY_BIN' not in os.environ,'Installed test bypassed the sibling gateway'
    owned=OwnedMutterInput();owned.enable_clipboard_reader()
    log=(root/'app.log').open('w')
    child=subprocess.Popen([native,str(root/'project/menu.html')],cwd=root,env=os.environ,stdout=log,stderr=log)
    memory=ProcessMemory(child.pid,gateway,include_web_content=True);desktop=Client(child)
    def keys(values):
        for k in values:owned.call('NotifyKeyboardKeysym',owned.GLib.Variant('(ub)',(k,True)))
        time.sleep(.06)
        for k in reversed(values):owned.call('NotifyKeyboardKeysym',owned.GLib.Variant('(ub)',(k,False)))
    def menu_item(name):
        return desktop.find(lambda n:n.get_role_name()=='menu item' and n.get_name()==name and desktop.showing(n))
    def focus_html():
        def in_document(node):
            for _ in range(30):
                if node.get_role_name() in ['document web','document frame']:return True
                node=node.get_parent()
                if node is None:return False
            return False
        for _ in range(100):
            current=desktop.find(lambda n:n.get_state_set().contains(desktop.api.StateType.FOCUSED))
            if current and (in_document(current) or desktop.find(lambda n:n.get_role_name() in ['document web','document frame'] and desktop.within(n,current))):return
            keys([0xff09])
        (root/'focus-states.json').write_text(json.dumps([dict(name=n.get_name(),role=n.get_role_name()) for n in desktop.tree()
            if n.get_state_set().contains(desktop.api.StateType.FOCUSED)],indent=2))
        raise RuntimeError('HTML content is not keyboard reachable')
    def open_menu(shift=False):
        focus_html();keys([0xffe1,0xffc7] if shift else [0xff67])
        desktop.wait('named HTML menu',lambda:menu_item('Copy') and menu_item('Select all') and menu_item('Translate selected text'))
        assert not desktop.find(lambda n:n.get_role_name()=='menu item' and desktop.showing(n) and not n.get_name()),'Unnamed native menu item'
    def capture(name):
        if os.environ.get('STUDIO_GNOME_SMOKE_CAPTURE_ONLY')=='1':
            (root/'native-capture-request').write_text(name)
            desktop.wait('screenshot '+name,lambda:(root/(name+'.png')).is_file())
    try:
        desktop.wait('passive HTML heading',lambda:desktop.find(lambda n:n.get_role_name()=='heading' and n.get_name()=='Native HTML menu' and desktop.showing(n)))
        composer=desktop.wait('native composer',lambda:desktop.find(lambda n:n.get_role_name()=='text' and n.get_name().startswith('Write a message.')))
        desktop.edit(composer,DRAFT);owned.focus_only_window()
        open_menu()
        assert not desktop.sensitive(menu_item('Translate selected text')),'Empty HTML selection enabled translation'
        desktop.activate(menu_item('Select all'))
        desktop.wait('selection menu dismissed',lambda:not menu_item('Select all'))
        open_menu(shift=True)
        desktop.wait('selected HTML Copy enabled',lambda:desktop.sensitive(menu_item('Copy')))
        assert desktop.sensitive(menu_item('Translate selected text'))
        capture('html-native-menu')
        desktop.activate(menu_item('Copy'))
        def copied():
            try:
                text=owned.clipboard_text()
                return text if 'Native HTML menu' in text and 'Rendered menu text 中文😀' in text else None
            except desktop.glib.Error:return None
        copied=desktop.wait('independent HTML clipboard',copied)
        assert DRAFT not in copied and 'UNTRUSTED_SCRIPT_CANARY' not in copied
        assert desktop.contents(composer)==DRAFT
        if (root/'translate-menu').exists():
            open_menu();desktop.activate(menu_item('Translate selected text'))
            dialog=desktop.wait('accessible menu translation dialog',lambda:desktop.find(lambda n:
                n.get_role_name()=='dialog' and n.get_name()=='Translate to Chinese' and desktop.showing(n)))
            desktop.wait('private utility translation completed',lambda:desktop.text('Translation complete'))
            capture('html-native-translation')
            keys([0xff1b])
            desktop.wait('translation dismissed',lambda:not desktop.find(lambda n:n.get_role_name()=='dialog'
                and n.get_name()=='Translate to Chinese' and desktop.showing(n)))
        open_menu()
        keys([0xff50]);keys([0xff54]);keys([0xff1b])
        desktop.wait('Escape closes HTML menu',lambda:not menu_item('Select all'))
        # Switching away retires the owned context, rather than leaving a
        # popup/action bound to a discarded WebKit document.
        open_menu();keys([0xff1b]);desktop.click('Source code')
        desktop.wait('original HTML source',lambda:desktop.find(lambda n:n.get_role_name()=='text' and desktop.contents(n)==SOURCE))
        assert not menu_item('Copy')
        desktop.click('Preview')
        desktop.wait('reopened HTML heading',lambda:desktop.find(lambda n:n.get_role_name()=='heading' and n.get_name()=='Native HTML menu' and desktop.showing(n)))
        assert (root/'project/menu.html').read_bytes()==SOURCE.encode() and desktop.contents(composer)==DRAFT
        owned.focus_only_window();keys([0xffe9,0xffc1]);child.wait(timeout=25);assert child.returncode==0
    except BaseException:
        (root/'accessibility.json').write_text(json.dumps(desktop.describe(),indent=2,ensure_ascii=False));raise
    finally:
        if child.poll() is None:child.terminate();child.wait(timeout=10)
        measured=memory.close();owned.close();log.close()
    turns=fixture.runtime.starts(root)
    if (root/'translate-menu').exists():
        calls=fixture.runtime.requests(root)
        starts=[r for r in calls if r.get('method')=='thread/start']
        deleted=[r for r in calls if r.get('method')=='thread/delete']
        assert len(turns)==len(starts)==len(deleted)==1 and starts[0]['params']['ephemeral'] is True
        assert turns[0]['params']['threadId'].startswith('translation-') and deleted[0]['params']['threadId']==turns[0]['params']['threadId']
        evidence=turns[0]['params']['input'][0]['text']
        assert 'Native HTML menu' in evidence and 'Rendered menu text 中文😀' in evidence and DRAFT not in evidence and 'UNTRUSTED_SCRIPT_CANARY' not in evidence
    else:assert not turns,'HTML controls sent inference'
    assert not any(t in (root/'app.log').read_text() for t in ['CRITICAL','Gtk-WARNING','panicked'])
    result=dict(ok=True,namedMenuActions=True,menuKey=True,shiftF10=True,selectAll=True,
        independentCopy=True,scopedEmptySelection=True,translationEnabledAfterSelection=True,
        keyboardDismissal=True,sourceRetirement=True,originalFileUnchanged=True,draftPreserved=True,
        noInference=not bool(turns),accessibleTranslation=bool(turns),privateUtilityOnly=bool(turns),
        noApplicationHooks=not bool(os.environ.get('STUDIO_GNOME_SMOKE_CAPTURE_ONLY')),
        installedPair=os.environ.get('STUDIO_HTML_MENU_INSTALLED')=='1',
        noGatewayOverride='CODEX_THREAD_STUDIO_GATEWAY_BIN' not in os.environ,memory=measured,artifacts=str(root))
    (root/'html-menu-result.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('native');p.add_argument('gateway');p.add_argument('--private-root');p.add_argument('--capture',action='store_true');p.add_argument('--installed',action='store_true');p.add_argument('--translate',action='store_true');a=p.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE,(0,0));native,gateway=str(Path(a.native).absolute()),str(Path(a.gateway).absolute())
    if a.private_root:return run(native,gateway,Path(a.private_root))
    if a.capture and a.installed:p.error('Screenshot hooks are debug-only')
    base=Path(__file__).resolve().parents[1]/'target/gnome-native-smoke';base.mkdir(parents=True,exist_ok=True)
    root=Path(tempfile.mkdtemp(prefix='html-menu-',dir=base));fixture.create(root)
    if a.installed:
        bundle=Path(native).parent;prefix=root/'installed native 中文 % html'
        subprocess.run(['bash',str(bundle/'install.sh'),str(bundle),str(prefix)],check=True,stdout=subprocess.DEVNULL)
        native,gateway=str(prefix/'bin/codex-thread-studio-gnome'),str(prefix/'bin/codex-thread-studio-gateway')
    codex=fixture.codex()
    if a.translate:
        ts=importlib.util.spec_from_file_location('html_translation_fixture',Path(__file__).with_name('smoke-gnome-translation.py'))
        translation=importlib.util.module_from_spec(ts);ts.loader.exec_module(translation)
        codex=codex.replace("turns=json.loads(ledger.read_text()) if ledger.exists() else {}", "turns=json.loads(ledger.read_text()) if ledger.exists() else {}\nutilities={}")
        codex=codex.replace("    elif method=='thread/read':",translation.UTILITY+"\n    elif method=='thread/read':")
        (root/'translate-menu').touch()
        settings=root/'config/codex-thread-studio/settings.json';values=json.loads(settings.read_text())
        values['translation']=dict(engine='backend',ollamaModel='fixture-local',models=dict(codex='translate-codex'),efforts=dict(codex='low'))
        settings.write_text(json.dumps(values))
    for name,code in [('fake-codex.py',codex),('fake-opencode.py',fixture.opencode())]:
        path=root/name;path.write_text(code);path.chmod(0o700)
    (root/'project/menu.html').write_bytes(SOURCE.encode())
    (root/'config/codex-thread-studio/gnome.json').write_text(json.dumps(dict(selectedBackend='codex',selectedSession='target')))
    env=dict(os.environ,CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway,CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),CODEX_THREAD_STUDIO_BACKENDS_CONFIG=str(root/'config/codex-thread-studio/backends.json'),CODEX_HOME=str(root/'codex'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),READER_CACHE_DIR=str(root/'reader-cache'),GDK_BACKEND='wayland',GIO_USE_VFS='local',GIO_USE_VOLUME_MONITOR='unix',GTK_A11Y='atspi')
    for key in list(env):
        if key.startswith('STUDIO_GNOME_SMOKE'):env.pop(key)
    if a.capture:env.update(STUDIO_GNOME_SMOKE_DIR=str(root),STUDIO_GNOME_SMOKE_CAPTURE_ONLY='1')
    if a.installed:env.pop('CODEX_THREAD_STUDIO_GATEWAY_BIN',None);env['STUDIO_HTML_MENU_INSTALLED']='1'
    with isolated_environment(env,headless=True) as private:
        subprocess.run(['dbus-run-session','--',shutil.which('mutter') or 'mutter','--headless','--wayland','--no-x11','--wayland-display','studio-test','--virtual-monitor','1600x1000','--',sys.executable,str(Path(__file__).resolve()),native,gateway,'--private-root',str(root)],cwd=root,env=private,check=True,timeout=180)
if __name__=='__main__':main()
