#!/usr/bin/env python3
"""Same-turn native Map tools/fallback, private streaming and offscreen queued turns."""
import argparse,ast,importlib.util,json,os,resource,shutil,subprocess,sys,tempfile
from pathlib import Path
sys.dont_write_bytecode=True
sys.path.insert(0,str(Path(__file__).with_name('test-support')))
from private_wayland import isolated_environment
from memory_sampler import ProcessMemory
spec=importlib.util.spec_from_file_location('sync_runtime',Path(__file__).with_name('smoke-gnome-router-runtime.py'))
runtime=importlib.util.module_from_spec(spec);spec.loader.exec_module(runtime)
HELPERS=r'''
contexts={};tool_pending={}
def emit(value):
    with lock:print(json.dumps(value),flush=True)
def complete(session,turn,prompt,revision):
    if 'offscreen' in prompt or 'stale' in prompt:
        name='offscreen' if 'offscreen' in prompt else 'stale'
        (root/(name+'-held')).write_text(turn)
        while not (root/('release-'+name)).exists():time.sleep(.01)
    operations=[] if 'empty' in prompt else [{'op':'setCurrent','itemId':'root' if 'offscreen' in prompt else 'child'}]
    private='invalid JSON' if 'malformed' in prompt else json.dumps({'baseRevision':revision,'operations':operations})
    marker='<codex-thread-studio-session-map-update-v1>'
    text='Visible '+prompt+' 中文😀\n\n'+marker+'\n'+private+'\n</codex-thread-studio-session-map-update-v1>'
    for chunk in ['Visible '+prompt+' 中文😀\n\n',marker[:11],marker[11:30],marker[30:]+'\n'+private+'\n</codex-thread-studio-session-map-update-v1>']:
        emit({'method':'item/agentMessage/delta','params':{'threadId':session,'turnId':turn,'itemId':turn+'-answer','delta':chunk}});time.sleep(.02)
    with lock:
        saved=next(t for t in turns[session] if t['id']==turn);saved['status']='completed';saved['items'].append({'id':turn+'-answer','type':'agentMessage','text':text});save()
    emit({'method':'item/completed','params':{'threadId':session,'turnId':turn,'item':{'id':turn+'-answer','type':'agentMessage','text':text}}})
    emit({'method':'turn/completed','params':{'threadId':session,'turn':{'id':turn,'status':'completed','items':saved['items']}}})
    if 'queued' in prompt:(root/'queued-completed').write_text(turn)
    if 'tool turn' in prompt:(root/'tool-completed').write_text(turn)
'''
RESUME=r'''
    elif method=='thread/resume':
        assert params['excludeTurns'] is True
        if 'developerInstructions' in params:
            if params.get('dynamicTools') and (root/'no-map-tools').exists():answer(identity,error='dynamicTools not supported');continue
            rules=params['developerInstructions']
            if rules is None:contexts.pop(session,None)
            else:
                context=json.loads(rules.split('Current Session Map JSON: ')[1].split('\n\nUse update_session_map')[0]);contexts[session]=context
                with (root/'map-contexts.jsonl').open('a') as output:output.write(json.dumps(context)+'\n')
        answer(identity,{'thread':summary(session)})
'''
START=r'''
    elif method=='turn/start':
        assert session=='target' and session in contexts,'Source turn was not given Map context'
        prompt=params['input'][0]['text'];context=contexts[session]
        expected={'Native tool turn':1,'Native fallback turn':2,'Native offscreen turn':3,'Native queued turn':4,'Native stale turn':5,'Native empty turn':6,'Native malformed turn':6}[prompt]
        assert context['revision']==expected,(prompt,context['revision'],expected)
        with lock:
            history=turns.setdefault(session,[]);turn=session+'-turn-'+str(len(history)+1)
            history.append({'id':turn,'status':'inProgress','items':[{'id':turn+'-user','type':'userMessage','content':params['input']}]});save()
        answer(identity,{'turn':{'id':turn,'status':'inProgress'}})
        emit({'method':'turn/started','params':{'threadId':session,'turn':{'id':turn,'status':'inProgress'}}})
        if 'tool turn' in prompt:
            tool_pending[turn]={'session':session,'turn':turn,'prompt':prompt,'revision':context['revision'],'replies':0}
            def tools(session=session,turn=turn):
                (root/'tool-held').write_text(turn)
                while not (root/'release-tool').exists():time.sleep(.01)
                for suffix in ['one','duplicate']:
                    emit({'id':'map-tool-'+suffix+'-'+turn,'method':'item/tool/call','params':{'threadId':session,'turnId':turn,'callId':turn+'-map-call','tool':'update_session_map','arguments':{'operations':[{'op':'setCurrent','itemId':'root'}]}}})
            threading.Thread(target=tools,daemon=True).start()
        else:threading.Thread(target=complete,args=(session,turn,prompt,context['revision']),daemon=True).start()
'''
RESPONSES=r'''
    if 'method' not in request:
        assert str(identity).startswith('map-tool-'),request
        assert request.get('result',{}).get('success') is True,request
        turn=identity.split('-target-turn-')[1];record=tool_pending['target-turn-'+turn]
        with (root/'map-tool-replies.jsonl').open('a') as output:output.write(json.dumps(request)+'\n')
        record['replies']+=1
        if record['replies']==2:threading.Thread(target=complete,args=(record['session'],record['turn'],record['prompt'],record['revision']),daemon=True).start()
        continue
'''
def source():
    text=runtime.CODEX.replace('for raw in sys.stdin:',HELPERS+'\nfor raw in sys.stdin:')
    text=text.replace("    if method=='initialize':",RESPONSES+"\n    if method=='initialize':")
    a=text.index("    elif method=='thread/resume':");b=text.index("    elif method=='fuzzyFileSearch':",a);text=text[:a]+RESUME+text[b:]
    a=text.index("    elif method=='turn/start':");b=text.index("    elif method=='thread/turns/list':",a);text=text[:a]+START+text[b:]
    text=text.replace("    elif method=='thread/read':","    elif method=='model/list':answer(identity,{'data':[{'id':'target-model','model':'target-model','isDefault':True,'supportedReasoningEfforts':[{'reasoningEffort':'low'}]}]})\n    elif method=='thread/items/list':answer(identity,{'data':[{'turnId':t['id'],'item':i} for t in reversed(turns.get(session,[])) for i in reversed(t['items'])],'nextCursor':None})\n    elif method=='thread/read':")
    ast.parse(text);return text
def gui(native,gateway,root):
    with (root/'native.log').open('w') as log:
        app=subprocess.Popen([native],cwd=root,env=os.environ,stdout=log,stderr=log);memory=ProcessMemory(app.pid,gateway)
        try:app.wait(timeout=180)
        except BaseException:app.terminate();app.wait(timeout=10);raise
        finally:measured=memory.close()
    path=root/'map-sync-result.json';result=json.loads(path.read_text()) if path.exists() else {}
    assert app.returncode==0 and result.get('ok'),(str(root),result)
    assert not any(word in (root/'native.log').read_text() for word in ['CRITICAL','panicked','Gtk-WARNING'])
    calls=runtime.requests(root);starts=[v for v in calls if v.get('method')=='turn/start'];assert len(starts)==7,(len(starts),root)
    assert all(v['params']['threadId']=='target' for v in starts);assert not any(v.get('method')=='thread/start' for v in calls),'Unexpected extra AI task'
    assert len((root/'map-tool-replies.jsonl').read_text().splitlines())==2
    contexts=[json.loads(line) for line in (root/'map-contexts.jsonl').read_text().splitlines()];assert [v['revision'] for v in contexts]==[1,2,3,4,5,6,6]
    ledger=json.loads((root/'codex-turns.json').read_text());assert len(ledger['target'])==7
    assert all('<codex-thread-studio-session-map-update-v1>' in turn['items'][-1]['text'] for turn in ledger['target'])
    for name in ['backend.pid','opencode.pid']:
        if (root/name).exists():assert not Path('/proc',(root/name).read_text().strip()).exists(),'Owned provider survived'
    print(json.dumps(dict(ok=True,checks=result,sourceTurns=7,extraInference=0,privateProviderHistoryRetained=True,contextRevisions=[v['revision'] for v in contexts],memory=measured,artifacts=str(root))))
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('native');parser.add_argument('gateway');parser.add_argument('--private-root');args=parser.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE,(0,0));native,gateway=str(Path(args.native).resolve()),str(Path(args.gateway).resolve())
    if args.private_root:return gui(native,gateway,Path(args.private_root))
    base=Path(__file__).resolve().parents[1]/'target/gnome-native-smoke';base.mkdir(parents=True,exist_ok=True);root=Path(tempfile.mkdtemp(prefix='map-sync-',dir=base));runtime.create_profile(root)
    runtime.fixture.BACKEND=source();service=runtime.fixture.Gateway(Path(gateway),root,'codex')
    try:
        status,value=service.api('/studio/session-map',dict(backend='codex',threadId='target',goal='Map sync goal 中文😀',definitionOfDone='Human confirms',structure='hierarchy',items=[dict(id='root',title='Root topic'),dict(id='child',parentId='root',title='Child topic')]))
        assert status==201 and value['revision']==1,(status,value)
    finally:service.close()
    (root/'config/codex-thread-studio/gnome.json').write_text(json.dumps(dict(selectedBackend='codex',selectedSession='target')))
    for name,body in [('fake-codex.py',source()),('fake-opencode.py',runtime.OPENCODE)]:path=root/name;path.write_text(body);path.chmod(0o700)
    env=dict(os.environ,CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway,CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),CODEX_HOME=str(root/'codex'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),READER_CACHE_DIR=str(root/'reader-cache'),GDK_BACKEND='wayland',GIO_USE_VFS='local',GIO_USE_VOLUME_MONITOR='unix',STUDIO_GNOME_SMOKE_DIR=str(root),STUDIO_GNOME_SMOKE_MAP_SYNC='1')
    with isolated_environment(env,headless=True) as private:
        subprocess.run(['dbus-run-session','--',shutil.which('mutter') or 'mutter','--headless','--wayland','--no-x11','--wayland-display','studio-test','--virtual-monitor','1600x1000','--',sys.executable,str(Path(__file__).resolve()),native,gateway,'--private-root',str(root)],cwd=root,env=private,check=True,timeout=220)
if __name__=='__main__':main()
