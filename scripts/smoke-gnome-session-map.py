#!/usr/bin/env python3
"""Native Map controls, isolated AI tasks and persisted-store restart on owned Wayland."""
import argparse,ast,importlib.util,json,os,resource,shutil,subprocess,sys,tempfile
from pathlib import Path
sys.dont_write_bytecode=True
sys.path.insert(0,str(Path(__file__).with_name('test-support')))
from private_wayland import isolated_environment
from memory_sampler import ProcessMemory
spec=importlib.util.spec_from_file_location('map_runtime',Path(__file__).with_name('smoke-gnome-router-runtime.py'))
runtime=importlib.util.module_from_spec(spec);spec.loader.exec_module(runtime)
UTILITY=r'''
    elif method=='thread/start' and params.get('ephemeral') is True:
        assert params['approvalPolicy']=='never' and params['sandbox']=='read-only'
        task='map-task-'+str(len(owned)+1);owned.append(task);utilities.add(task)
        answer(identity,{'thread':{'id':task}})
    elif method=='turn/start' and session.startswith('map-task-'):
        assert session in utilities and session!='target' and params.get('outputSchema')
        schema=params['outputSchema'];text=params['input'][0]['text'];turn=session+'-turn'
        if 'goal' in schema['properties']:
            assert 'Native source question' in text and 'Native source answer' in text
            content={'goal':'Inferred goal 中文😀','definitionOfDone':'Human acceptance'}
        else:
            context=json.loads(text.split('Authoritative map:\n')[1].split('\nRecent conversation:')[0])
            assert context['goal']=='Inferred goal 中文😀'
            content={'operations':[{'op':'setCurrent','itemId':'child'}]} if context['items'] else {'operations':[
                {'op':'addItem','itemId':'root','parentId':None,'afterItemId':None,'title':'Root topic 中文😀','kind':'topic','summary':'<literal> source','state':'notStarted'},
                {'op':'addItem','itemId':'child','parentId':'root','afterItemId':None,'title':'Child topic','kind':'topic','summary':'Context','state':'notStarted'}]}
        def complete(session=session,turn=turn,content=content):
            if (root/'hold-map').exists():
                (root/'map-held').write_text(session)
                while not (root/'release-map').exists():time.sleep(.01)
            with lock:
                print(json.dumps({'method':'item/completed','params':{'threadId':session,'item':{'id':turn+'-answer','type':'agentMessage','text':json.dumps(content)}}}),flush=True)
                print(json.dumps({'method':'turn/completed','params':{'threadId':session,'turn':{'id':turn,'status':'completed'}}}),flush=True)
        threading.Thread(target=complete,daemon=True).start()
        answer(identity,{'turn':{'id':turn,'status':'inProgress'}})
    elif method=='turn/interrupt' and session.startswith('map-task-'):
        assert session in utilities;answer(identity,{})
    elif method=='thread/delete' and session.startswith('map-task-'):
        assert session in utilities;utilities.remove(session);answer(identity,{})
'''
def source():
    value=runtime.CODEX.replace("turns=json.loads(ledger.read_text()) if ledger.exists() else {}","turns=json.loads(ledger.read_text()) if ledger.exists() else {}\nutilities=set();owned=[]")
    value=value.replace("    elif method=='thread/read':",UTILITY+"\n    elif method=='model/list':answer(identity,{'data':[{'id':'target-model','model':'target-model','isDefault':True,'supportedReasoningEfforts':[{'reasoningEffort':'low'}]}]})\n    elif method=='thread/items/list':answer(identity,{'data':[{'turnId':t['id'],'item':i} for t in reversed(turns.get(session,[])) for i in reversed(t['items'])],'nextCursor':None})\n    elif method=='thread/read':")
    ast.parse(value);return value
def gui(native,gateway,root):
    reports=[]
    for phase in ['edit','restart']:
        env=dict(os.environ,STUDIO_GNOME_SMOKE_MAP=phase)
        with (root/f'{phase}.log').open('w') as log:
            app=subprocess.Popen([native],cwd=root,env=env,stdout=log,stderr=log);memory=ProcessMemory(app.pid,gateway)
            try:app.wait(timeout=120)
            except BaseException:app.terminate();app.wait(timeout=10);raise
            finally:measured=memory.close()
        path=root/'session-map-result.json';result=json.loads(path.read_text()) if path.exists() else {}
        assert app.returncode==0 and result.get('ok'),(phase,str(root),result)
        assert not any(word in (root/f'{phase}.log').read_text() for word in ['CRITICAL','panicked','Gtk-WARNING'])
        for name in ['backend.pid','opencode.pid']:
            if (root/name).exists():
                assert not Path('/proc',(root/name).read_text().strip()).exists(),'Owned provider survived application shutdown'
        path.rename(root/f'{phase}.json');reports.append(dict(phase=phase,checks=result,memory=measured))
    calls=runtime.requests(root);starts=[v for v in calls if v.get('method')=='turn/start'];created=[v for v in calls if v.get('method')=='thread/start'];deleted=[v for v in calls if v.get('method')=='thread/delete']
    assert len(starts)==5 and len(created)==5 and len(deleted)==5,(len(starts),len(created),len(deleted))
    assert all(v['params']['threadId'].startswith('map-task-') for v in starts+deleted)
    assert all(v['params']['ephemeral'] is True for v in created)
    assert json.loads((root/'codex-turns.json').read_text())==json.loads((root/'original-turns.json').read_text()),'Source history changed'
    print(json.dumps(dict(ok=True,phases=reports,isolatedModelTasks=5,sourceTurnsUnchanged=True,ownedCleanup=True,artifacts=str(root))))
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('native');parser.add_argument('gateway');parser.add_argument('--private-root');args=parser.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE,(0,0));native,gateway=str(Path(args.native).resolve()),str(Path(args.gateway).resolve())
    if args.private_root:return gui(native,gateway,Path(args.private_root))
    base=Path(__file__).resolve().parents[1]/'target/gnome-native-smoke';base.mkdir(parents=True,exist_ok=True);root=Path(tempfile.mkdtemp(prefix='session-map-',dir=base));runtime.create_profile(root)
    (root/'config/codex-thread-studio/gnome.json').write_text(json.dumps(dict(selectedBackend='codex',selectedSession='target')))
    turns={'target':[{'id':'source-turn','status':'completed','items':[{'id':'source-user','type':'userMessage','content':[{'type':'text','text':'Native source question 中文😀'}]},{'id':'source-answer','type':'agentMessage','text':'Native source answer 中文😀'}]}]}
    for name in ['codex-turns.json','original-turns.json']:(root/name).write_text(json.dumps(turns))
    for name,body in [('fake-codex.py',source()),('fake-opencode.py',runtime.OPENCODE)]:path=root/name;path.write_text(body);path.chmod(0o700)
    env=dict(os.environ,CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway,CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),CODEX_HOME=str(root/'codex'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),READER_CACHE_DIR=str(root/'reader-cache'),GDK_BACKEND='wayland',GIO_USE_VFS='local',GIO_USE_VOLUME_MONITOR='unix',STUDIO_GNOME_SMOKE_DIR=str(root))
    with isolated_environment(env,headless=True) as private:
        subprocess.run(['dbus-run-session','--',shutil.which('mutter') or 'mutter','--headless','--wayland','--no-x11','--wayland-display','studio-test','--virtual-monitor','1600x1000','--',sys.executable,str(Path(__file__).resolve()),native,gateway,'--private-root',str(root)],cwd=root,env=private,check=True,timeout=260)
if __name__=='__main__':main()
