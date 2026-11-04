#!/usr/bin/env python3
"""Unread prompt cues and real read receipts in a private native Router profile."""
import argparse, importlib.util, json, os, resource, shutil, sqlite3, subprocess, sys, tempfile, time, uuid
from pathlib import Path
sys.dont_write_bytecode=True
sys.path.insert(0,str(Path(__file__).with_name('test-support')))
from private_wayland import OwnedMutterInput, isolated_environment
from native_accessibility import Client
spec=importlib.util.spec_from_file_location('unread_archive',Path(__file__).with_name('smoke-gnome-router-archive.py'))
archive=importlib.util.module_from_spec(spec);spec.loader.exec_module(archive)
runtime=archive.runtime
def question(i):return 'Unread response {index} literal 中文😀 request '+str(i)
def connection(root):return sqlite3.connect(root/'config/codex-thread-studio/studio.sqlite3')
def records(root):
    with connection(root) as db:return db.execute('SELECT controller,turn_key,dispatch FROM router_dispatches ORDER BY controller,turn_key').fetchall()
def unread(root,identity):
    with connection(root) as db:return db.execute('SELECT unread FROM native_router_jobs WHERE id=?',(identity,)).fetchone()[0]
def codex():return archive.codex().replace("    elif method=='thread/list':","    elif method=='thread/searchOccurrences':answer(identity,error='Method not found: thread/searchOccurrences')\n    elif method=='thread/list':",1)

def seed(gateway,root):
    runtime.fixture.BACKEND=codex();runtime.fixture.OPENCODE_BACKEND=archive.opencode()
    service=runtime.fixture.Gateway(Path(gateway),root,'codex')
    try:
        assert service.api('/studio/native/router/jobs?controller=codex%3Arouter')[0]==200
        ids={str(i):str(uuid.uuid4()) for i in range(1,26)};ids['foreign']=str(uuid.uuid4())
        with connection(root) as db:
            for number,identity in ids.items():
                foreign=number=='foreign';i=100 if foreign else int(number);controller='opencode:oc-router' if foreign else 'codex:router'
                body='\n\n'+'\n\n'.join('Clarification '+str(i)+' paragraph '+str(p)+'. '+'Bounded native reading context. '*12 for p in range(16))+'\n\nEND CUE READ '+str(i)
                job=dict(id=identity,controller=controller,createdAt=i,prompt=question(i),status='clarify',controllerTurnId='',target=None,targetTurnId='',decision=None,response=body,error='',unread=True)
                db.execute('INSERT INTO native_router_jobs(id,controller,created,owner,phase,json,receipt,unread) VALUES(?,?,?,?,?,?,?,?)',(identity,controller,i,'private-unread-fixture','clarify',json.dumps(job),uuid.uuid4().hex,1))
        for n in range(2):
            value=dict(status='clarify',unread=True,directTurn=dict(id='router-turn-'+str(n)),decision=dict(action='clarify',targetSessionKey='',forwardedPrompt='',reason='',message='Existing browser clarification'))
            assert service.api('/studio/router-history',dict(controller='codex:router',turnKey='codex:router-turn-'+str(n),dispatch=value),method='PUT')[0]==204
        (root/'unread-identities.json').write_text(json.dumps(ids))
        (root/'original-browser-records.json').write_text(json.dumps(records(root)))
    finally:service.close()

def run(native,root):
    capture=os.environ.get('STUDIO_UNREAD_CAPTURE')=='1'
    assert capture or not any(k.startswith('STUDIO_GNOME_SMOKE') for k in os.environ)
    ids=json.loads((root/'unread-identities.json').read_text());original=records(root)
    owned=OwnedMutterInput()
    def keys(values):
        for k in values:owned.call('NotifyKeyboardKeysym',owned.GLib.Variant('(ub)',(k,True)))
        time.sleep(.04)
        for k in reversed(values):owned.call('NotifyKeyboardKeysym',owned.GLib.Variant('(ub)',(k,False)))
        time.sleep(.04)
    log=(root/'app.log').open('w');child=subprocess.Popen([native],cwd=root,env=os.environ,stdout=log,stderr=log);client=Client(child)
    draft='Preserve unread-navigation draft 中文😀 '
    def cue(i,marked=True,chinese=False):
        suffix=' · 未读回复' if chinese else ' · Unread response'
        return client.find(lambda n:n.get_role_name()=='button' and client.showing(n) and question(i) in n.get_name() and n.get_name().endswith(suffix)==marked)
    def menu(chinese=False):
        client.click('对话轮次导航' if chinese else 'Turn navigation')
        client.wait('mapped turn menu',lambda:client.text('当前页的对话轮次' if chinese else 'Turns in this page'))
        label=client.wait('mapped turn row',lambda:client.find(lambda n:n.get_role_name()=='button' and client.showing(n) and ('用户输入' if chinese else 'User input ') in n.get_name()))
        target=label
        while target.get_role_name()!='list':target=target.get_parent()
        for _ in range(40):
            if client.find(lambda n:n.get_state_set().contains(client.api.StateType.FOCUSED) and client.within(n,target)):break
            keys([0xff09])
        else:raise RuntimeError('Turn list is not keyboard reachable')
        keys([0xffe3,0xff50])
    def close_menu(chinese=False):
        keys([0xff1b]);client.wait('closed turn menu',lambda:not client.text('当前页的对话轮次' if chinese else 'Turns in this page'))
    def capture_menu(name):
        if not capture:return
        (root/'native-turn-capture-request').write_text(name)
        client.wait('painted '+name,lambda:(root/(name+'.png')).exists() or (root/(name+'.error')).exists())
        assert not (root/(name+'.error')).exists()
    def bounds(node):
        box=client.api.Component.get_extents(node.get_component_iface(),client.api.CoordType.WINDOW)
        return box.x,box.y,box.width,box.height
    def choose_source(index):
        # GtkDropDown exposes its selected option as the accessible name.
        combo=client.wait('Router source picker',lambda:client.find(lambda n:n.get_role_name()=='combo box' and n.get_name() in ['Current requests','Earlier browser requests'] and client.showing(n)))
        for _ in range(100):
            current=client.find(lambda n:n.get_state_set().contains(client.api.StateType.FOCUSED))
            if current and client.within(current,combo):break
            keys([0xff09])
        else:raise RuntimeError('Router source picker is not reachable')
        keys([0x20]);client.wait('Router source options',lambda:client.text('Current requests') and client.text('Earlier browser requests'))
        keys([0xff50] if index==0 else [0xff57]);keys([0xff0d])
        client.wait('selected Router source',lambda:client.find(lambda n:n.get_role_name()=='combo box' and n.get_name()==['Current requests','Earlier browser requests'][index] and client.showing(n)))
    def question_visible(i):
        for node in client.tree():
            if not client.showing(node) or node.get_role_name() not in ['label','text'] or node.get_name()!=question(i):continue
            parent=node.get_parent()
            while parent and parent.get_role_name()!='scroll pane':parent=parent.get_parent()
            if parent is None:continue
            box=client.api.Component.get_extents(node.get_component_iface(),client.api.CoordType.WINDOW)
            viewport=client.api.Component.get_extents(parent.get_component_iface(),client.api.CoordType.WINDOW)
            if box.width>0 and box.height>0 and box.y+box.height>viewport.y and box.y<viewport.y+viewport.height:return node
    def select_session(title,sidebar=None):
        label=client.wait('session '+title,lambda:client.find(lambda n:client.showing(n) and n.get_role_name()=='label' and n.get_name().endswith(title) and (sidebar is None or client.within(n,sidebar))))
        row=label
        while row.get_role_name()!='list item':row=row.get_parent()
        parent=row.get_parent();selection=parent.get_selection_iface()
        assert selection and client.api.Selection.select_child(selection,row.get_index_in_parent()),'Native session selection unavailable'
        return parent
    def language(chinese):
        keys([0xffe3,ord(',')]);client.wait('language settings',lambda:client.text('Apply language') or client.text('应用语言'))
        combo=client.wait('language picker',lambda:client.find(lambda n:n.get_role_name()=='combo box' and n.get_name() in ['Language','语言'] and client.showing(n)))
        for _ in range(100):
            current=client.find(lambda n:n.get_state_set().contains(client.api.StateType.FOCUSED));parent=combo.get_parent()
            if current and (client.within(current,combo) or (parent and parent.get_role_name()=='list item' and client.within(current,parent))):break
            keys([0xff09])
        else:raise RuntimeError('Language picker is not reachable')
        keys([0x20]);client.wait('language choices',lambda:client.text('简体中文') and client.text('English'))
        keys([0xff50])
        for _ in range(1 if chinese else 2):keys([0xff54])
        keys([0xff0d]);client.click('Apply language' if chinese else '应用语言')
        client.wait('saved locale',lambda:json.loads((root/'config/codex-thread-studio/settings.json').read_text()).get('language')==('zh-CN' if chinese else 'en-US'))
        dialog=client.wait('Settings dialog',lambda:client.find(lambda n:n.get_role_name()=='dialog' and n.get_name() in ['Settings','设置'] and client.showing(n)))
        client.activate(client.wait('Settings close',lambda:client.find(lambda n:n.get_name() in ['Close','关闭'] and n.get_role_name()=='button' and client.within(n,dialog) and client.showing(n))))
        client.wait('Settings closed',lambda:not client.find(lambda n:n.get_role_name()=='dialog' and n.get_name() in ['Settings','设置'] and client.showing(n)))
    try:
        client.wait('Router page loaded',lambda:client.text('Latest requests'))
        owned.focus_only_window();client.wait('active owned window',lambda:client.find(lambda n:n.get_role_name()=='frame' and n.get_state_set().contains(client.api.StateType.ACTIVE)))
        composer=client.wait('composer',lambda:client.find(lambda n:n.get_role_name()=='text' and n.get_name().startswith('Write a message.')));client.edit(composer,draft)
        client.wait('visible latest response read',lambda:unread(root,ids['25'])==0)
        assert unread(root,ids['6'])==1
        menu();six=client.wait('older unread cue',lambda:cue(6));seven=client.wait('second unread cue',lambda:cue(7))
        geometry=bounds(six),bounds(seven)
        focused=client.find(lambda n:n.get_state_set().contains(client.api.StateType.FOCUSED))
        capture_menu('unread-before')
        # Simulate another client updating the same shared unread state. This
        # checks live metadata presentation, not the native visibility receipt.
        with connection(root) as db:db.execute("UPDATE native_router_jobs SET unread=0,json=json_set(json,'$.unread',json('false')) WHERE id=?",(ids['6'],))
        current=client.wait('live cue cleared',lambda:cue(6,False));assert current==six,'Unread update replaced a bound prompt button'
        assert cue(7)==seven,'Unrelated prompt button was replaced'
        assert (bounds(six),bounds(seven))==geometry,'Acknowledgement changed prompt geometry'
        assert client.find(lambda n:n.get_state_set().contains(client.api.StateType.FOCUSED))==focused,'Acknowledgement moved focus'
        capture_menu('unread-after');close_menu()
        language(True);menu(True);client.wait('translated unread cue with literal question',lambda:cue(7,True,True));capture_menu('unread-zh');close_menu(True)
        language(False)
        menu();client.activate(client.wait('jump to unread question',lambda:cue(7)))
        client.wait('question popup closed',lambda:not client.text('Turns in this page'))
        client.wait('actual transcript question overlap',lambda:question_visible(7))
        # Give the existing 500 ms dwell guard time to observe the question.
        # Its response end is still below the viewport and must remain unread.
        deadline=time.monotonic()+.8
        while time.monotonic()<deadline:
            assert unread(root,ids['7'])==1,'Question-only navigation acknowledged an unseen response'
            time.sleep(.05)
        client.click('Search Router requests and replies')
        entry=client.wait('persisted Router search',lambda:client.find(lambda n:n.get_role_name() in ['entry','text'] and ('Search saved requests and replies' in n.get_name() or n.get_name().startswith('Literal text in saved requests')) and client.showing(n)))
        client.edit(entry,'END CUE READ 7')
        client.wait('actual response end acknowledged',lambda:unread(root,ids['7'])==0)
        client.edit(entry,'');client.wait('unfiltered Router page',lambda:client.text('Latest requests'))
        client.click('Search Router requests and replies')
        menu();client.wait('native receipt clears cue',lambda:cue(7,False));close_menu()
        client.click('Older Router requests');client.wait('older Router page',lambda:client.text('Earlier requests'))
        menu();client.wait('older page cue',lambda:cue(1));assert not cue(7);close_menu()
        choose_source(1);client.wait('read-only browser page',lambda:client.text('Read-only browser history'))
        client.wait('browser source question',lambda:client.text('Original controller request'))
        menu();assert not client.find(lambda n:n.get_role_name()=='button' and client.showing(n) and n.get_name().endswith(' · Unread response'));close_menu()
        assert records(root)==original,'Reading browser history changed its saved records'
        choose_source(0);client.wait('current Router view restored',lambda:client.text('Latest requests'))
        sidebar=select_session('Native target')
        client.wait('normal source loaded',lambda:client.text('Later unrelated source task'))
        menu();assert not client.find(lambda n:n.get_role_name()=='button' and client.showing(n) and n.get_name().endswith(' · Unread response'));close_menu()
        select_session('Router',sidebar)
        client.wait('Router source restored',lambda:client.text('Latest requests'))
        menu();client.wait('owned unread cue restored',lambda:cue(6,False) and cue(7,False) and cue(8));close_menu()
        assert client.contents(composer)==draft
        assert unread(root,ids['foreign'])==1
        assert not runtime.starts(root),'Unread navigation sent inference'
        keys([0xffe9,0xffc1]);child.wait(timeout=25);assert child.returncode==0
        for name in ['backend.pid','opencode.pid']:
            p=root/name
            if p.exists():runtime.until(lambda:not Path('/proc',p.read_text().strip()).exists(),seconds=10)
        assert not any(t in (root/'app.log').read_text() for t in ['CRITICAL','Gtk-WARNING','panicked'])
        result=dict(ok=True,noApplicationHooks=not capture,passiveCapture=capture,installedPair=os.environ.get('STUDIO_UNREAD_INSTALLED')=='1',gatewayOverride='CODEX_THREAD_STUDIO_GATEWAY_BIN' in os.environ,
            unreadCue=True,stableMetadataUpdate=True,stableGeometryAndFocus=True,liveLanguageLiteral=True,questionDoesNotRead=True,visibleEndReceipt=True,paging=True,sourceSwitch=True,readOnlyBrowserHistory=True,foreignControllerUntouched=True,draftPreserved=True,noInference=True,artifacts=str(root))
        (root/'unread-result.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
    except BaseException:
        (root/'failed-tree.json').write_text(json.dumps(client.describe(),ensure_ascii=False,indent=2));raise
    finally:
        if child.poll() is None:child.terminate();child.wait(timeout=15)
        owned.close();log.close()

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('native');p.add_argument('gateway');p.add_argument('--private-root',type=Path);p.add_argument('--installed',action='store_true');p.add_argument('--capture',action='store_true');a=p.parse_args();resource.setrlimit(resource.RLIMIT_CORE,(0,0))
    if a.installed and a.capture:p.error('Use separate debug capture and installed acceptance')
    native,gateway=str(Path(a.native).resolve()),str(Path(a.gateway).resolve())
    if a.private_root:return run(native,a.private_root)
    directory=Path(os.environ.get('STUDIO_NATIVE_QA_DIR') or Path(__file__).resolve().parents[1]/'target/gnome-native-smoke');directory.mkdir(parents=True,exist_ok=True);root=Path(tempfile.mkdtemp(prefix='router-unread-',dir=directory));archive.create(root)
    if a.installed:
        bundle=Path(native).parent;prefix=root/'installed native 中文 % unread'
        subprocess.run(['bash',str(bundle/'install.sh'),str(bundle),str(prefix)],check=True,stdout=subprocess.DEVNULL)
        native,gateway=str(prefix/'bin/codex-thread-studio-gnome'),str(prefix/'bin/codex-thread-studio-gateway')
    seed(gateway,root)
    for name,value in [('fake-codex.py',codex()),('fake-opencode.py',archive.opencode())]:path=root/name;path.write_text(value);path.chmod(0o700)
    settings=root/'config/codex-thread-studio/settings.json';raw=json.loads(settings.read_text());raw['language']='en-US';settings.write_text(json.dumps(raw))
    (settings.parent/'gnome.json').write_text(json.dumps(dict(selectedBackend='codex',selectedSession='router',windowWidth=1280,windowHeight=900)))
    env=dict(os.environ,CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway,CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),CODEX_THREAD_STUDIO_BACKENDS_CONFIG=str(root/'config/codex-thread-studio/backends.json'),CODEX_HOME=str(root/'codex'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),READER_CACHE_DIR=str(root/'reader-cache'),GDK_BACKEND='wayland',GIO_USE_VFS='local',GIO_USE_VOLUME_MONITOR='unix',GTK_A11Y='atspi')
    for key in list(env):
        if key.startswith('STUDIO_GNOME_SMOKE') or key in ['STUDIO_UNREAD_CAPTURE','STUDIO_UNREAD_INSTALLED']:env.pop(key)
    if a.installed:env.pop('CODEX_THREAD_STUDIO_GATEWAY_BIN',None);env['STUDIO_UNREAD_INSTALLED']='1'
    if a.capture:env.update(STUDIO_UNREAD_CAPTURE='1',STUDIO_GNOME_SMOKE_DIR=str(root),STUDIO_GNOME_SMOKE_CAPTURE_ONLY='1')
    with isolated_environment(env,headless=True) as private:subprocess.run(['dbus-run-session','--',shutil.which('mutter') or 'mutter','--headless','--wayland','--no-x11','--wayland-display','studio-test','--virtual-monitor','1600x1000','--',sys.executable,str(Path(__file__).resolve()),native,gateway,'--private-root',str(root)],cwd=root,env=private,check=True,timeout=240)
if __name__=='__main__':main()
