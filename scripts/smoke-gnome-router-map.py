#!/usr/bin/env python3
"""Gateway-owned routed Maps: real API, fake providers, and optional native Wayland UI."""
import argparse,ast,importlib.util,json,os,resource,shutil,subprocess,sys,tempfile,time
from pathlib import Path
sys.dont_write_bytecode=True
sys.path.insert(0,str(Path(__file__).with_name('test-support')))
from private_wayland import isolated_environment
from memory_sampler import ProcessMemory
spec=importlib.util.spec_from_file_location('routed_map_runtime',Path(__file__).with_name('smoke-gnome-router-runtime.py'))
runtime=importlib.util.module_from_spec(spec);spec.loader.exec_module(runtime)
HELPERS=r'''
contexts={};pending_tools={}
def emit(value):
    with lock:print(json.dumps(value),flush=True)
def complete_map(session,turn,prompt,mode,revision):
    if mode=='MAP_TOOL' and (root/'hold-after-tool').exists():
        (root/'tool-applied-held').write_text(turn)
        while not (root/'release-after-tool').exists():time.sleep(.01)
    if mode=='MAP_FALLBACK' and (root/'gui-hold-fallback').exists():
        (root/'gui-fallback-held').write_text(turn)
        while not (root/'gui-release-fallback').exists():time.sleep(.01)
    if mode in ['MAP_HUMAN','MAP_REPLACE']:
        (root/(mode+'-held')).write_text(turn)
        while not (root/(mode+'-release')).exists():time.sleep(.01)
    operations=[] if mode=='MAP_EMPTY' else [{'op':'setCurrent','itemId':'child'}]
    private='invalid JSON' if mode=='MAP_MALFORMED' else json.dumps({'baseRevision':revision,'operations':operations})
    text='Routed visible '+mode+' 中文😀'
    if mode!='MAP_NONE':text+='\n\n<codex-thread-studio-session-map-update-v1>\n'+private+'\n</codex-thread-studio-session-map-update-v1>'
    with lock:
        saved=next(t for t in turns[session] if t['id']==turn);saved['status']='completed';saved['items'].append({'id':turn+'-answer','type':'agentMessage','text':text});save()
    emit({'method':'item/completed','params':{'threadId':session,'turnId':turn,'item':saved['items'][-1]}})
    emit({'method':'turn/completed','params':{'threadId':session,'turn':{'id':turn,'status':'completed','items':saved['items']}}})
    (root/(mode+'-completed')).write_text(turn)
def send_map_tools(session,turn,mode):
    if (root/'gui-hold-tool').exists():
        (root/'gui-tool-held').write_text(turn)
        while not (root/'gui-release-tool').exists():time.sleep(.01)
    operations=[{'op':'setState','itemId':'root','state':'done'}] if mode=='MAP_UNSAFE' else [{'op':'setCurrent','itemId':'root'}]
    for suffix in (['one','duplicate'] if mode=='MAP_TOOL' else ['one']):
        emit({'id':'router-map-'+suffix+'-'+turn,'method':'item/tool/call','params':{'threadId':session,'turnId':turn,'callId':turn+'-map-call','tool':'update_session_map','arguments':{'operations':operations}}})
'''
RESUME=r'''
    elif method=='thread/resume':
        assert params['excludeTurns'] is True
        if session=='router':
            assert params['approvalPolicy']=='never' and params['sandbox']=='read-only'
            assert params['developerInstructions'] is None and not params.get('dynamicTools')
        if 'developerInstructions' in params:
            if params.get('dynamicTools') and (root/'no-map-tools').exists():answer(identity,error='dynamicTools unsupported');continue
            if params['developerInstructions'] is None:contexts.pop(session,None)
            else:
                assert session=='target'
                contexts[session]=json.loads(params['developerInstructions'].split('Current Session Map JSON: ')[1].split('\n\nUse update_session_map')[0])
                with (root/'map-contexts.jsonl').open('a') as output:output.write(json.dumps(contexts[session])+'\n')
        answer(identity,{'thread':summary(session)})
'''
TARGET=r'''
    elif method=='turn/start' and session=='target':
        prompt=params['input'][0]['text'];mode=next(name for name in ['MAP_TOOL','MAP_EARLY','MAP_UNSAFE','MAP_FALLBACK','MAP_HUMAN','MAP_MALFORMED','MAP_EMPTY','MAP_REPLACE','MAP_NONE'] if name in prompt)
        assert (session in contexts)==(mode!='MAP_NONE'),(mode,contexts)
        revision=contexts.get(session,{}).get('revision',0)
        with lock:
            history=turns.setdefault(session,[]);turn='target-turn-'+str(len(history)+1)
            history.append({'id':turn,'status':'inProgress','items':[{'id':turn+'-user','type':'userMessage','content':params['input']}]});save()
        emit({'method':'turn/started','params':{'threadId':session,'turn':{'id':turn,'status':'inProgress'}}})
        if mode in ['MAP_TOOL','MAP_UNSAFE','MAP_EARLY']:
            pending_tools[turn]=dict(session=session,turn=turn,prompt=prompt,mode=mode,revision=revision,replies=0,ack=identity)
            if mode!='MAP_EARLY':answer(identity,{'turn':{'id':turn,'status':'inProgress'}})
            threading.Thread(target=send_map_tools,args=(session,turn,mode),daemon=True).start()
        else:
            answer(identity,{'turn':{'id':turn,'status':'inProgress'}})
            threading.Thread(target=complete_map,args=(session,turn,prompt,mode,revision),daemon=True).start()
'''
RESPONSES=r'''
    if 'method' not in request:
        assert str(identity).startswith('router-map-'),request
        turn='target-turn-'+identity.split('-target-turn-')[1];record=pending_tools[turn]
        expected=record['mode']=='MAP_TOOL'
        assert request.get('result',{}).get('success') is expected,(record,request)
        with (root/'map-replies.jsonl').open('a') as output:output.write(json.dumps(request)+'\n')
        record['replies']+=1
        if record['replies']==(2 if expected else 1):
            if record['mode']=='MAP_EARLY':answer(record['ack'],{'turn':{'id':turn,'status':'inProgress'}})
            threading.Thread(target=complete_map,args=(record['session'],turn,record['prompt'],record['mode'],record['revision']),daemon=True).start()
        continue
'''
RECOVERY=r'''
    elif method=='thread/turns/list' and session=='target' and (root/'recover-finish.json').exists():
        description=json.loads((root/'recover-finish.json').read_text())
        with lock:
            for turn in turns.get(session,[]):
                if turn['id']==description['turn']:
                    turn['status']='completed'
                    if not any(i['id']==turn['id']+'-recovered' for i in turn['items']):
                        text='Recovered routed visible 中文😀\n\n<codex-thread-studio-session-map-update-v1>\n'+json.dumps({'baseRevision':description['revision'],'operations':[{'op':'setCurrent','itemId':'child'}]})+'\n</codex-thread-studio-session-map-update-v1>'
                        turn['items'].append({'id':turn['id']+'-recovered','type':'agentMessage','text':text})
            save();values=list(reversed(turns.get(session,[])))[:4]
        answer(identity,{'data':values,'nextCursor':None})
'''
def codex():
    text=runtime.CODEX.replace('for raw in sys.stdin:',HELPERS+'\nfor raw in sys.stdin:')
    text=text.replace("    if method=='initialize':",RESPONSES+"\n    if method=='initialize':")
    a=text.index("    elif method=='thread/resume':");b=text.index("    elif method=='fuzzyFileSearch':",a);text=text[:a]+RESUME+text[b:]
    text=text.replace("    elif method=='turn/start':",TARGET+"\n    elif method=='turn/start':")
    text=text.replace("    elif method=='thread/turns/list':",RECOVERY+"\n    elif method=='thread/turns/list':")
    text=text.replace("                context=params['additionalContext']['codex-thread-studio/thread-router']['value']","                context=params['additionalContext']['codex-thread-studio/thread-router']['value']\n                assert 'baseRevision' not in context and '<codex-thread-studio-session-map-update-v1>' not in context")
    text=text.replace("    elif method=='thread/read':","    elif method=='model/list':answer(identity,{'data':[{'id':'controller-model','model':'controller-model','isDefault':True,'supportedReasoningEfforts':[]}]})\n    elif method=='thread/items/list':answer(identity,{'data':[{'turnId':t['id'],'item':i} for t in reversed(turns.get(session,[])) for i in reversed(t['items'])],'nextCursor':None})\n    elif method=='thread/read':")
    ast.parse(text);return text
def create_map(api,thread='target',goal='Routed goal 中文😀'):
    status,value=api.api('/studio/session-map',dict(backend='codex',threadId=thread,goal=goal,definitionOfDone='Human confirms',structure='hierarchy',items=[dict(id='root',title='Root topic'),dict(id='child',parentId='root',title='Child topic')]))
    assert status==201,(status,value);return value
def read_map(api):
    status,value=api.api('/studio/session-map/codex/target');assert status==200,(status,value);return value
def map_change(api,raw,operations):
    status,value=api.api('/studio/session-map/codex/target/operations',dict(baseRevision=raw['revision'],mapId=raw['id'],actor='user',operations=operations));assert status==200,(status,value);return value
def api_checks(gateway,root):
    runtime.create_profile(root);runtime.fixture.BACKEND=codex();runtime.fixture.OPENCODE_BACKEND=runtime.OPENCODE
    oc=root/'fake-opencode.py';oc.write_text(runtime.OPENCODE);oc.chmod(0o700)
    api=runtime.fixture.Gateway(Path(gateway),root,'codex',oc)
    try:
        api.setup();create_map(api,'router','Controller Map must not contaminate structured decisions');raw=create_map(api);original=raw['id'];reports=[]
        for mode in ['MAP_TOOL','MAP_FALLBACK','MAP_UNSAFE','MAP_EARLY','MAP_HUMAN','MAP_MALFORMED','MAP_EMPTY','MAP_REPLACE','MAP_NONE']:
            if mode=='MAP_FALLBACK':(root/'no-map-tools').touch()
            else:(root/'no-map-tools').unlink(missing_ok=True)
            if mode=='MAP_NONE':
                raw=read_map(api);assert api.api('/studio/session-map/codex/target?revision='+str(raw['revision'])+'&mapId='+raw['id'],method='DELETE')[0]==200
            before=read_map(api) if mode!='MAP_NONE' else None
            identity=runtime.submit(api,runtime.body(api,mode))
            if mode in ['MAP_HUMAN','MAP_REPLACE']:
                runtime.until(lambda:(root/(mode+'-held')).exists());raw=read_map(api)
                if mode=='MAP_HUMAN':map_change(api,raw,[dict(op='setState',itemId='root',state='done')])
                else:
                    assert api.api('/studio/session-map/codex/target?revision='+str(raw['revision'])+'&mapId='+raw['id'],method='DELETE')[0]==200
                    create_map(api,goal='Replacement goal')
                (root/(mode+'-release')).touch()
            job=runtime.finished(api,identity);assert job['response']=='Routed visible '+mode+' 中文😀',job
            assert all('baseRevision' not in item['text'] for item in job['responseItems']);assert 'baseRevision' not in job['contextExcerpt']
            if mode=='MAP_NONE':assert job['mapScope'] is None and not job.get('mapNotice');continue
            raw=read_map(api)
            if mode in ['MAP_TOOL','MAP_FALLBACK','MAP_UNSAFE','MAP_EARLY']:
                assert raw['revision']==before['revision']+1,(mode,raw,job)
                assert raw['currentItemId']==('root' if mode=='MAP_TOOL' else 'child');assert not any(i['state']=='done' for i in raw['items'])
            elif mode=='MAP_HUMAN':assert raw['revision']==before['revision']+1 and any(i['state']=='done' for i in raw['items']) and 'conflict' in job['mapNotice']
            elif mode=='MAP_REPLACE':assert raw['id']!=original and raw['revision']==1 and 'replaced' in job['mapNotice']
            else:assert raw['revision']==before['revision']
            if mode=='MAP_MALFORMED':assert 'not valid JSON' in job['mapNotice']
            if mode=='MAP_EMPTY':assert 'did not require' in job['mapNotice']
            assert job['mapScope']['id']==before['id'] and job['mapScope']['revision']==before['revision']
            reports.append(dict(mode=mode,revision=raw['revision'],notice=job['mapNotice']))
        assert len(runtime.starts(root,'router'))==9 and len(runtime.starts(root,'target'))==9
        assert len((root/'map-replies.jsonl').read_text().splitlines())==4
        assert api.api('/studio/session-map/codex/router')[1]['revision']==1
        api.close();recovery=recovery_checks(gateway,root)
        return dict(ok=True,cases=reports,sourceTurns=9,controllerTurns=9,extraInference=0,controllerMapManualOnly=True,privateProjection=True,recovery=recovery)
    finally:api.close()
def recovery_checks(gateway,root):
    results=[]
    api=runtime.fixture.Gateway(Path(gateway),root,'codex',root/'fake-opencode.py')
    try:
        create_map(api)
        for applied in [False,True]:
            mode='MAP_TOOL' if applied else 'MAP_HUMAN'
            before=read_map(api)
            if applied:(root/'hold-after-tool').touch()
            else:(root/'MAP_HUMAN-release').unlink(missing_ok=True)
            identity=runtime.submit(api,runtime.body(api,mode))
            runtime.until(lambda:runtime.get(api,identity)['status']=='running')
            held=root/('tool-applied-held' if applied else 'MAP_HUMAN-held');runtime.until(held.exists)
            pending=runtime.get(api,identity);turn=pending['targetTurnId'];assert turn and pending['mapScope']['revision']==before['revision']
            if applied:runtime.until(lambda:read_map(api)['revision']==before['revision']+1)
            starts=len(runtime.starts(root));api.close()
            (root/'recover-finish.json').write_text(json.dumps(dict(turn=turn,revision=before['revision'])))
            api=runtime.fixture.Gateway(Path(gateway),root,'codex',root/'fake-opencode.py')
            job=runtime.finished(api,identity);after=read_map(api)
            assert after['revision']==before['revision']+1 and len(runtime.starts(root))==starts,(applied,job,after)
            assert after['lastSyncedTurnId']==turn
            assert ('already updated' in job['mapNotice']) if applied else ('revision '+str(after['revision']) in job['mapNotice'])
            assert job['response']=='Recovered routed visible 中文😀' and 'baseRevision' not in job['contextExcerpt']
            results.append(dict(alreadyApplied=applied,revision=after['revision'],monitorOnly=True,notice=job['mapNotice']))
        return results
    finally:api.close()
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('native');parser.add_argument('gateway');parser.add_argument('--api-only',action='store_true');parser.add_argument('--private-root');args=parser.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE,(0,0));gateway=str(Path(args.gateway).resolve());native=str(Path(args.native).resolve())
    if args.private_root:return gui(native,gateway,Path(args.private_root))
    base=Path(__file__).resolve().parents[1]/'target/gnome-native-smoke';base.mkdir(parents=True,exist_ok=True);root=Path(tempfile.mkdtemp(prefix='router-map-',dir=base));report=api_checks(gateway,root)
    if args.api_only:print(json.dumps(dict(ok=True,api=report,artifacts=str(root))));return
    ui_root=Path(tempfile.mkdtemp(prefix='router-map-ui-',dir=base));runtime.create_profile(ui_root);runtime.fixture.BACKEND=codex();oc=ui_root/'fake-opencode.py';oc.write_text(runtime.OPENCODE);oc.chmod(0o700)
    api=runtime.fixture.Gateway(Path(gateway),ui_root,'codex',oc)
    try:api.setup();create_map(api,'router','Controller Map stays manual');create_map(api)
    finally:api.close()
    (ui_root/'config/codex-thread-studio/gnome.json').write_text(json.dumps(dict(selectedBackend='codex',selectedSession='router')))
    code=ui_root/'fake-codex.py';code.write_text(codex());code.chmod(0o700);(ui_root/'gui-hold-tool').touch()
    env=dict(os.environ,CODEX_THREAD_STUDIO_BACKENDS_CONFIG=str(ui_root/'config/codex-thread-studio/backends.json'),CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway,CODEX_THREAD_STUDIO_CODEX_BIN=str(code),CODEX_THREAD_STUDIO_OPENCODE_BIN=str(oc),CODEX_HOME=str(ui_root/'codex'),XDG_CONFIG_HOME=str(ui_root/'config'),XDG_DATA_HOME=str(ui_root/'data'),XDG_CACHE_HOME=str(ui_root/'cache'),READER_CACHE_DIR=str(ui_root/'reader-cache'),GDK_BACKEND='wayland',GIO_USE_VFS='local',GIO_USE_VOLUME_MONITOR='unix',STUDIO_GNOME_SMOKE_DIR=str(ui_root),STUDIO_GNOME_SMOKE_ROUTER_MAP='1')
    with isolated_environment(env,headless=True) as private:
        subprocess.run(['dbus-run-session','--',shutil.which('mutter') or 'mutter','--headless','--wayland','--no-x11','--wayland-display','studio-test','--virtual-monitor','1600x1000','--',sys.executable,str(Path(__file__).resolve()),native,gateway,'--private-root',str(ui_root)],cwd=ui_root,env=private,check=True,timeout=180)
    print(json.dumps(dict(ok=True,api=report,apiArtifacts=str(root),uiArtifacts=str(ui_root))))
def gui(native,gateway,root):
    with (root/'native.log').open('w') as log:
        app=subprocess.Popen([native],cwd=root,env=os.environ,stdout=log,stderr=log);memory=ProcessMemory(app.pid,gateway)
        try:app.wait(timeout=140)
        except BaseException:app.terminate();app.wait(timeout=10);raise
        finally:measured=memory.close()
    path=root/'routed-map-result.json';result=json.loads(path.read_text()) if path.exists() else {};assert app.returncode==0 and result.get('ok'),(str(root),result)
    assert not any(word in (root/'native.log').read_text() for word in ['CRITICAL','panicked','Gtk-WARNING'])
    assert len(runtime.starts(root,'router'))==2 and len(runtime.starts(root,'target'))==2
    assert len((root/'map-replies.jsonl').read_text().splitlines())==2,'A viewer also answered a server-owned Map call'
    for name in ['backend.pid','opencode.pid']:
        if (root/name).exists():assert not Path('/proc',(root/name).read_text().strip()).exists(),'Owned provider survived'
    print(json.dumps(dict(ok=True,ui=result,memory=measured,sourceTurns=2,controllerTurns=2,extraInference=0,artifacts=str(root))))
if __name__=='__main__':main()
