#!/usr/bin/env python3
"""Private two-provider history/context acceptance for native turn navigation."""
import argparse, ast, importlib.util, json, os, resource, shutil, subprocess, sys, tempfile, time
from pathlib import Path
sys.dont_write_bytecode=True
sys.path.insert(0,str(Path(__file__).with_name('test-support')))
from private_wayland import OwnedMutterInput, isolated_environment
from native_accessibility import Client
spec=importlib.util.spec_from_file_location('navigation_base',Path(__file__).with_name('smoke-gnome-turn-navigation.py'))
base=importlib.util.module_from_spec(spec);spec.loader.exec_module(base)
runtime=base.runtime
PROMPT=lambda i:'Prompt '+str(i).zfill(2)+' 中文😀 literal {index}'

CODEX_PAGE=r'''    elif method=='thread/items/list':
        entries=[{'turnId':t['id'],'item':i} for t in turns.get(session,[]) for i in t['items']]
        cursor=params.get('cursor');end=len(entries) if not cursor else int(cursor.split('=',1)[1])
        if cursor:assert cursor.startswith('native-nav:中文/+?&=')
        start=max(0,end-12)
        result={'data':list(reversed(entries[start:end])),'nextCursor':('native-nav:中文/+?&='+str(start)) if start else None}
        delay=bool(cursor) and (root/'hold-navigation-page').exists()
        def deliver(identity=identity,result=result,delay=delay):
            if delay:
                (root/'navigation-page-held').touch()
                while not (root/'release-navigation-page').exists():time.sleep(.01)
            answer(identity,result)
            if delay:(root/'navigation-page-released').touch()
        threading.Thread(target=deliver,daemon=True).start()
'''
CODEX_EVENTS=r'''
def navigation_events():
    try:
        while True:
            path=root/'navigation-events.json'
            if path.exists():
                events=json.loads(path.read_text());path.unlink()
                for event in events:
                    with lock:print(json.dumps(event),flush=True)
                (root/'navigation-events-emitted').touch()
            time.sleep(.01)
    except BaseException:
        import traceback
        (root/'navigation-events-error').write_text(traceback.format_exc())
threading.Thread(target=navigation_events,daemon=True).start()
'''
OC_PAGE=r'''        if path.endswith('/message'):
            assert query.get('directory')==[str(root/'project-oc')],query
            rows=messages.get(identity,[]);before=query.get('before',[None])[0]
            end=len(rows) if not before else int(before.split('=',1)[1])
            if before:assert before.startswith('native-nav-oc:%E4%B8%AD/+?&=')
            start=max(0,end-12);value=rows[start:end]
            body=json.dumps(value).encode();self.send_response(200);self.send_header('Content-Type','application/json');self.send_header('Content-Length',str(len(body)))
            if start:self.send_header('x-next-cursor','native-nav-oc:%E4%B8%AD/+?&='+str(start))
            self.end_headers();self.wfile.write(body);return
'''

def providers():
    code=base.fixture.codex()
    begin=code.index("    elif method=='thread/items/list':");end=code.index("    elif method=='thread/start'",begin)
    code=code[:begin]+CODEX_PAGE+code[end:]
    code=code.replace('for raw in sys.stdin:',CODEX_EVENTS+'\nfor raw in sys.stdin:',1)
    code=code.replace("summary('target'),summary('hidden')","summary('target'),summary('other'),summary('hidden')",1)
    oc=base.fixture.opencode()
    begin=oc.index("        if path.endswith('/message'):");end=oc.index("        if path.endswith('/prompt_async'):",begin)
    oc=oc[:begin]+OC_PAGE+oc[end:]
    oc=oc.replace("session('oc-target'),{**session('oc-similar')","session('oc-target'),session('oc-other'),{**session('oc-similar')",1)
    for value in [code,oc]:ast.parse(value)
    return code,oc

def run(native,root,backend):
    assert not any(k.startswith('STUDIO_GNOME_SMOKE') for k in os.environ)
    owned=OwnedMutterInput()
    def keys(values):
        for k in values:owned.call('NotifyKeyboardKeysym',owned.GLib.Variant('(ub)',(k,True)))
        time.sleep(.04)
        for k in reversed(values):owned.call('NotifyKeyboardKeysym',owned.GLib.Variant('(ub)',(k,False)))
        time.sleep(.04)
    log=(root/'app.log').open('w');child=subprocess.Popen([native],cwd=root,env=os.environ,stdout=log,stderr=log);client=Client(child)
    original={p.name:p.read_bytes() for p in [root/'codex-turns.json',root/'oc-turns.json']}
    def publish(events):
        staged=root/'navigation-events.pending';staged.write_text(json.dumps(events));staged.replace(root/'navigation-events.json')
    def emitted():
        error=root/'navigation-events-error'
        assert not error.exists(),error.read_text() if error.exists() else ''
        return (root/'navigation-events-emitted').exists()
    draft='Scoped navigator draft '+backend+' 中文😀 '
    try:
        client.wait('latest source',lambda:client.text('Answer 15'))
        owned.focus_only_window();client.wait('active owned frame',lambda:client.find(lambda n:n.get_role_name()=='frame' and n.get_state_set().contains(client.api.StateType.ACTIVE)))
        composer=client.wait('composer',lambda:client.find(lambda n:n.get_role_name()=='text' and n.get_name().startswith('Write a message.')));client.edit(composer,draft)
        def overlap(index):
            for node in client.tree():
                if not client.showing(node) or node.get_name()!=PROMPT(index):continue
                parent=node.get_parent()
                while parent and parent.get_role_name()!='scroll pane':parent=parent.get_parent()
                if parent is None:continue
                box=client.api.Component.get_extents(node.get_component_iface(),client.api.CoordType.WINDOW);viewport=client.api.Component.get_extents(parent.get_component_iface(),client.api.CoordType.WINDOW)
                if box.width>0 and box.height>0 and box.y+box.height>viewport.y and box.y<viewport.y+viewport.height:return node
        def choose(ordinal,index):
            client.click('Turn navigation');client.wait('popup',lambda:client.text('Turns in this page'))
            client.click('User input '+str(ordinal)+': '+PROMPT(index))
            client.wait('popup closed',lambda:not client.text('Turns in this page'))
            client.wait('prompt overlap '+str(index),lambda:overlap(index))
            assert client.contents(composer)==draft
        keys([0xffe3,0xffe1,ord('h')]);client.wait('history page one',lambda:client.text('History · page 1'))
        choose(1,10)
        client.click('Older');client.wait('history page two',lambda:client.text('History · page 2'))
        choose(1,4)
        client.click('Older');client.wait('history page three',lambda:client.text('History · page 3'))
        choose(1,1)
        if backend=='codex':
            events=[dict(method='turn/started',params=dict(threadId='target',turn=dict(id='turn-live'))),dict(method='item/completed',params=dict(threadId='target',turnId='turn-live',item=dict(id='user-live',type='userMessage',content=[dict(type='text',text='Live prompt')]))),dict(method='item/completed',params=dict(threadId='target',turnId='turn-live',item=dict(id='answer-live',type='agentMessage',text='Live streaming answer')))]
            publish(events);client.wait('upstream events emitted',emitted)
            client.click('Turn navigation');client.wait('historical marker retained',lambda:client.text('User input 1: '+PROMPT(1)))
            assert not client.text('User input 1: Live prompt');keys([0xff1b])
            client.wait('history position survives live input',lambda:overlap(1))
        client.click('Newer');client.wait('history newer',lambda:client.text('History · page 2'));choose(6,9)
        client.click('Live');client.wait('history closed',lambda:not client.text('History · page'))
        choose(1,10)
        if backend=='codex':
            (root/'navigation-events-emitted').unlink()
            publish([dict(method='item/agentMessage/delta',params=dict(threadId='target',turnId='turn-live',itemId='answer-live',delta=' — newer live output 中文😀'))])
            client.wait('live delta emitted',emitted)
            client.click('Turn navigation');client.wait('stream keeps reading marker',lambda:client.text('User input 1: '+PROMPT(10)))
            keys([0xff1b]);client.wait('reading position survives live delta',lambda:overlap(10))
            assert client.contents(composer)==draft
        # Select another source using the standard accessible list selection.
        def select(title):
            label=client.wait('session '+title,lambda:client.find(lambda n:client.showing(n) and n.get_role_name()=='label' and n.get_name().endswith(title)))
            row=label
            while row.get_role_name()!='list item':row=row.get_parent()
            parent=row.get_parent();selection=parent.get_selection_iface()
            assert selection and client.api.Selection.select_child(selection,row.get_index_in_parent()),'Native session selection unavailable'
        client.click('Turn navigation');old=client.wait('old prompt action',lambda:client.find(lambda n:n.get_role_name()=='button' and n.get_name()=='User input 1: '+PROMPT(10)))
        keys([0xff1b]);select('Native other' if backend=='codex' else 'Native oc-other')
        client.wait('new source answer',lambda:client.text('Other answer'))
        client.click('Turn navigation');client.wait('new source preview',lambda:client.text('Other prompt'))
        assert not client.text('User input 1: '+PROMPT(10))
        keys([0xff1b]);select('Native target' if backend=='codex' else 'Native oc-target')
        client.wait('draft restored',lambda:client.contents(composer)==draft)
        if backend=='codex':
            # Returning restores the existing reading position, so the latest
            # answer need not be visible. The enabled native action is the
            # authoritative history-readiness barrier.
            client.click('Main menu');client.click('Browse conversation history…')
            client.wait('fresh history page',lambda:client.text('History · page 1'))
            (root/'hold-navigation-page').touch();client.click('Older')
            client.wait('held history request',lambda:(root/'navigation-page-held').exists())
            select('Native other');client.wait('other source during held read',lambda:client.text('Other answer'))
            (root/'release-navigation-page').touch();client.wait('late old-source reply delivered',lambda:(root/'navigation-page-released').exists())
            client.click('Turn navigation');client.wait('late reply retains new scope',lambda:client.text('Other prompt'))
            assert not client.text('User input 1: '+PROMPT(4));keys([0xff1b])
            select('Native target');client.wait('draft after late reply',lambda:client.contents(composer)==draft)
        assert not runtime.starts(root)
        assert all((root/name).read_bytes()==value for name,value in original.items())
        keys([0xffe9,0xffc1]);child.wait(timeout=25);assert child.returncode==0
        for name in ['backend.pid','opencode.pid']:
            path=root/name
            if path.exists():runtime.until(lambda:not Path('/proc',path.read_text().strip()).exists(),seconds=10)
        assert not any(t in (root/'app.log').read_text() for t in ['CRITICAL','Gtk-WARNING','panicked'])
        result=dict(ok=True,backend=backend,noApplicationHooks=True,installedPair=os.environ.get('STUDIO_NAVIGATION_INSTALLED')=='1',gatewayOverride='CODEX_THREAD_STUDIO_GATEWAY_BIN' in os.environ,historyPages=3,newerAndLive=True,contextSwitch=True,sourceAndDraftPreserved=True,liveWhileHistorical=backend=='codex',navigationDuringStream=backend=='codex',lateHistoryRejected=backend=='codex',noInference=True,artifacts=str(root))
        (root/'scope-result.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
    except BaseException:
        (root/'failed-tree.json').write_text(json.dumps(client.describe(),ensure_ascii=False,indent=2));raise
    finally:
        if child.poll() is None:child.terminate();child.wait(timeout=15)
        owned.close();log.close()

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('native');p.add_argument('gateway');p.add_argument('--backend',choices=['codex','opencode'],default='codex');p.add_argument('--private-root',type=Path);p.add_argument('--installed',action='store_true');a=p.parse_args();resource.setrlimit(resource.RLIMIT_CORE,(0,0))
    native,gateway=str(Path(a.native).resolve()),str(Path(a.gateway).resolve())
    if a.private_root:return run(native,a.private_root,a.backend)
    artifacts=Path(os.environ.get('STUDIO_NATIVE_QA_DIR') or Path(__file__).resolve().parents[1]/'target/gnome-native-smoke');artifacts.mkdir(parents=True,exist_ok=True);root=Path(tempfile.mkdtemp(prefix='navigation-scope-'+a.backend+'-',dir=artifacts));runtime.create_profile(root)
    if a.installed:
        bundle=Path(native).parent;prefix=root/'installed native 中文 % scope'
        subprocess.run(['bash',str(bundle/'install.sh'),str(bundle),str(prefix)],check=True,stdout=subprocess.DEVNULL)
        native,gateway=str(prefix/'bin/codex-thread-studio-gnome'),str(prefix/'bin/codex-thread-studio-gateway')
    raw=json.loads((root/'config/codex-thread-studio/settings.json').read_text());raw['language']='en-US';(root/'config/codex-thread-studio/settings.json').write_text(json.dumps(raw))
    (root/'config/codex-thread-studio/gnome.json').write_text(json.dumps(dict(selectedBackend=a.backend,selectedSession='target' if a.backend=='codex' else 'oc-target',windowWidth=1280,windowHeight=900)))
    turns=[dict(id='turn-'+str(i),status='completed',items=[dict(id='user-'+str(i),type='userMessage',content=[dict(type='text',text=PROMPT(i))]),dict(id='answer-'+str(i),type='agentMessage',text='\n\n'.join('Answer '+str(i)+' paragraph '+str(j)+' '+'Readable context. '*12 for j in range(8)))]) for i in range(1,16)]
    other=[dict(id='other-'+str(i),status='completed',items=[dict(id='other-user-'+str(i),type='userMessage',content=[dict(type='text',text='Other prompt '+str(i))]),dict(id='other-answer-'+str(i),type='agentMessage',text='Other answer '+str(i))]) for i in range(2)]
    (root/'codex-turns.json').write_text(json.dumps(dict(target=turns,other=other)))
    messages={}
    for target,source in [('oc-target',turns),('oc-other',other)]:
        rows=[]
        for turn in source:
            user,answer=turn['items'];uid=user['id'];aid=answer['id']
            rows.extend([dict(info=dict(id=uid,role='user',sessionID=target),parts=[dict(id=uid+'-part',messageID=uid,sessionID=target,type='text',text=user['content'][0]['text'])]),dict(info=dict(id=aid,role='assistant',sessionID=target,parentID=uid,time=dict(completed=123)),parts=[dict(id=aid+'-part',messageID=aid,sessionID=target,type='text',text=answer['text'])])])
        messages[target]=rows
    (root/'oc-turns.json').write_text(json.dumps(messages))
    for name,value in zip(['fake-codex.py','fake-opencode.py'],providers()):path=root/name;path.write_text(value);path.chmod(0o700)
    env=dict(os.environ,CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway,CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),CODEX_THREAD_STUDIO_BACKENDS_CONFIG=str(root/'config/codex-thread-studio/backends.json'),CODEX_HOME=str(root/'codex'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),READER_CACHE_DIR=str(root/'reader-cache'),GDK_BACKEND='wayland',GIO_USE_VFS='local',GIO_USE_VOLUME_MONITOR='unix',GTK_A11Y='atspi')
    for key in list(env):
        if key.startswith('STUDIO_GNOME_SMOKE'):env.pop(key)
    env.pop('STUDIO_NAVIGATION_INSTALLED',None)
    if a.installed:
        env.pop('CODEX_THREAD_STUDIO_GATEWAY_BIN',None);env['STUDIO_NAVIGATION_INSTALLED']='1'
    with isolated_environment(env,headless=True) as private:subprocess.run(['dbus-run-session','--',shutil.which('mutter') or 'mutter','--headless','--wayland','--no-x11','--wayland-display','studio-test','--virtual-monitor','1600x1000','--',sys.executable,str(Path(__file__).resolve()),native,gateway,'--backend',a.backend,'--private-root',str(root)],cwd=root,env=private,check=True,timeout=210)
if __name__=='__main__':main()
