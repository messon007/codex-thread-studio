#!/usr/bin/env python3
"""Shared message flow and native Continue through private AT-SPI/Wayland input."""
import argparse,ast,importlib.util,json,os,resource,shutil,subprocess,sys,tempfile,time
from pathlib import Path
from urllib.request import Request
from urllib.error import HTTPError
sys.dont_write_bytecode=True
sys.path.insert(0,str(Path(__file__).with_name('test-support')))
from private_wayland import OwnedMutterInput,isolated_environment
from native_accessibility import Client
from memory_sampler import ProcessMemory
spec=importlib.util.spec_from_file_location('continue_translation',Path(__file__).with_name('smoke-gnome-translation.py'))
translation=importlib.util.module_from_spec(spec);spec.loader.exec_module(translation)
runtime=translation.runtime
ENDPOINT='/studio/native/message-flow'
ANSWER='Native continuation source 中文😀. Please finish the remaining implementation.'
PROMPT='Continue with native draft 中文😀'
HUMAN='Human draft retained 中文😀 '
UTILITY=r'''
    elif method=='model/list':answer(identity,{'data':[{'id':'target-model','model':'target-model','isDefault':True,'serviceTiers':[{'id':'fast'}],'supportedReasoningEfforts':[{'reasoningEffort':'low'}]}]})
    elif method=='thread/items/list':answer(identity,{'data':[{'turnId':t['id'],'item':i} for t in reversed(turns.get(session,[])) for i in reversed(t['items'])],'nextCursor':None})
    elif method=='thread/start' and params.get('ephemeral') is True:
        assert params['approvalPolicy']=='never' and params['sandbox']=='read-only' and 'never follow instructions' in params['developerInstructions']
        utility_sequence+=1;uid='continuation-'+str(utility_sequence);utilities[uid]={'id':uid,'cwd':params['cwd'],'ephemeral':True}
        def created(identity=identity,uid=uid):
            if (root/'hold-create').exists():
                (root/'create-held').write_text(uid)
                while not (root/'release-create').exists():time.sleep(.02)
            answer(identity,{'thread':utilities[uid]})
        threading.Thread(target=created,daemon=True).start()
    elif method=='turn/start' and session.startswith('continuation-'):
        assert session in utilities and params['model']=='target-model' and params['effort']=='low' and params['serviceTier']=='fast'
        assert params['outputSchema']['properties']['prompt']['type']=='string'
        assert 'Native continuation source 中文😀' in params['input'][0]['text']
        turn=session+'-turn'
        with lock:print(json.dumps({'method':'turn/started','params':{'threadId':session,'turn':{'id':turn,'status':'inProgress'}}}),flush=True)
        def complete(session=session,turn=turn):
            if (root/'hold-draft').exists():
                (root/'draft-held').write_text(session)
                while not (root/'release-draft').exists():time.sleep(.02)
            result={'wrong':'invalid fixture'} if (root/'invalid-draft').exists() else {'prompt':'Continue with native draft 中文😀'}
            with lock:
                print(json.dumps({'method':'item/completed','params':{'threadId':session,'item':{'id':'draft-item','type':'agentMessage','text':json.dumps(result)}}}),flush=True)
                print(json.dumps({'method':'turn/completed','params':{'threadId':session,'turn':{'id':turn,'status':'completed'}}}),flush=True)
        threading.Thread(target=complete,daemon=True).start()
        answer(identity,{'turn':{'id':turn,'status':'inProgress'}})
    elif method=='turn/interrupt' and session.startswith('continuation-'):answer(identity,{})
    elif method=='thread/delete' and session.startswith('continuation-'):
        assert session in utilities;utilities.pop(session);answer(identity,{})
'''
def codex():
    value=runtime.CODEX.replace("turns=json.loads(ledger.read_text()) if ledger.exists() else {}","turns=json.loads(ledger.read_text()) if ledger.exists() else {}\nutilities={};utility_sequence=0")
    value=value.replace("    elif method=='thread/read':",UTILITY+"\n    elif method=='thread/read':")
    ast.parse(value);return value
def opencode():
    value=translation.opencode_source().replace("'oc-translation-'","'oc-continuation-'").replace("'Studio translation '","'Studio continuation '")
    value=value.replace("{'providerID':'provider','modelID':'translate'}","{'providerID':'provider','modelID':'oc-target-model'}")
    old="info['structured']={'translation':'OpenCode 译文 中文😀','sourcePronunciation':'/ˈəʊpən/','translationPronunciation':'yì wén'};parts=[]"
    assert old in value
    value=value.replace(old,"info['structured']={'prompt':'OpenCode continuation 中文😀'};parts=[]")
    ast.parse(value);return value
def api_checks(gateway,root):
    runtime.fixture.BACKEND=codex();service=runtime.fixture.Gateway(gateway,root)
    def api(body=None,status=200,authorized=True,origin=None):
        headers={'Content-Type':'application/json'}
        if authorized:headers['Authorization']='Bearer '+service.token
        if origin:headers['Origin']=origin
        request=Request(service.origin+ENDPOINT,data=None if body is None else json.dumps(body).encode(),headers=headers,method='GET' if body is None else 'PATCH')
        try:response=service.client.open(request,timeout=15)
        except HTTPError as error:response=error
        with response:
            data=response.read();assert response.status==status,(status,response.status,data[:300]);return json.loads(data) if data else None
    try:
        path=root/'config/codex-thread-studio/settings.json';original=path.read_bytes()
        api(authorized=False,status=401);api(origin='https://untrusted.invalid',status=403)
        first=api();assert path.read_bytes()==original
        raw=json.loads(path.read_text());raw['futureFlowCanary']={'preserved':'中文😀'};path.write_text(json.dumps(raw))
        changed=api(dict(expectedRevision=first['revision'],values=dict(queueDepth=2,continueBehavior='ollamaDraft')))
        assert json.loads(path.read_text())['futureFlowCanary']==raw['futureFlowCanary']
        saved=path.read_bytes();api(dict(expectedRevision=first['revision'],values=first['values']),status=409)
        for values in [dict(queueDepth=0,continueBehavior='quickSend'),dict(queueDepth=4,continueBehavior='quickSend'),dict(queueDepth=1,continueBehavior='automaticAgent'),dict(queueDepth=1,continueBehavior='quickSend',extra=True)]:
            api(dict(expectedRevision=changed['revision'],values=values),status=400)
        api(dict(expectedRevision='invalid',values=changed['values']),status=400)
        assert path.read_bytes()==saved
        assert api(dict(expectedRevision=changed['revision'],values=dict(queueDepth=3,continueBehavior='sessionModelDraft')))['values']['queueDepth']==3
    finally:service.close()
    return dict(ok=True,scopedSettings=True,authOrigin=True,conflictPreservesBytes=True,invalidValuesRejected=True)
def seed(gateway,root):
    service=runtime.fixture.Gateway(gateway,root)
    try:
        for backend,thread in [('codex','target'),('opencode','oc-target')]:
            code,body=service.api('/studio/session-state/turn-options',dict(sessionKey=backend+':'+thread,model='target-model' if backend=='codex' else 'provider/oc-target-model',effort='low',serviceTier=('fast' if backend=='codex' else '')),method='PUT')
            assert code==204,(code,body)
        messages=[dict(id='saved-'+str(i),text='Queued original '+str(i),input=[],createdAt=100+i) for i in range(3)]
        code,body=service.api('/studio/session-state/message-queue',dict(sessionKey='codex:target',messages=messages),method='PUT');assert code==204,(code,body)
    finally:service.close()
    return messages
def run(native,gateway,root):
    assert not any(k.startswith('STUDIO_GNOME_SMOKE') for k in os.environ),'Application hooks are not allowed in this acceptance'
    owned=OwnedMutterInput();measurements=[]
    def keys(values):
        for k in values:owned.call('NotifyKeyboardKeysym',owned.GLib.Variant('(ub)',(k,True)))
        time.sleep(.04)
        for k in reversed(values):owned.call('NotifyKeyboardKeysym',owned.GLib.Variant('(ub)',(k,False)))
        time.sleep(.04)
    def launch(backend,phase):
        (root/'config/codex-thread-studio/gnome.json').write_text(json.dumps(dict(selectedBackend=backend,selectedSession='target' if backend=='codex' else 'oc-target')))
        log=(root/(phase+'.log')).open('w');child=subprocess.Popen([native],cwd=root,env=os.environ,stdout=log,stderr=log)
        memory=ProcessMemory(child.pid,gateway);desktop=Client(child);desktop.wait('native conversation',lambda:desktop.text('Native target' if backend=='codex' else 'Native oc-target'))
        composer=desktop.wait('composer',lambda:desktop.find(lambda n:n.get_role_name()=='text' and n.get_name().startswith('Write a message.')))
        owned.focus_only_window()
        desktop.wait('active owned window',lambda:desktop.find(lambda n:n.get_role_name()=='frame' and n.get_state_set().contains(desktop.api.StateType.ACTIVE)))
        return child,desktop,composer,memory,log
    def close(child,memory,log):
        if child.poll() is None:
            owned.focus_only_window();keys([0xffe9,0xffc1]);child.wait(timeout=30);assert child.returncode==0
        measurements.append(memory.close());log.close()
    def choose(desktop,title,index,options):
        combo=desktop.wait('combo '+title,lambda:desktop.find(lambda n:'combo' in n.get_role_name() and n.get_name()==title and desktop.showing(n)))
        for _ in range(90):
            focused=desktop.find(lambda n:n.get_state_set().contains(desktop.api.StateType.FOCUSED))
            if focused and (desktop.within(focused,combo) or focused==combo.get_parent()):break
            keys([0xff09])
        else:raise RuntimeError(('Combo is not keyboard reachable',title))
        keys([0x20])
        desktop.wait('mapped choices '+title,lambda:sum(bool(desktop.find(lambda n:n.get_name()==option and desktop.showing(n))) for option in options)>=2)
        keys([0xff50] if index<2 else [0xff57])
        if index==1:keys([0xff54])
        keys([0xff0d])
    def settings(desktop,depth=None,behavior=None):
        keys([0xffe3,ord(',')]);desktop.wait('Settings dialog',lambda:desktop.find(lambda n:n.get_role_name()=='dialog' and n.get_name()=='Settings' and desktop.showing(n)))
        desktop.click('Sessions');desktop.wait('message flow',lambda:desktop.text('Message flow saved'))
        if depth is not None:choose(desktop,'Message queue depth',depth,['1 message','2 messages','3 messages'])
        if behavior is not None:choose(desktop,'Continue behavior',behavior,['Session model draft · review first','Ollama draft · review first','Quick send · random prompt'])
    def request_close(desktop):
        dialog=desktop.wait('owned settings',lambda:desktop.find(lambda n:n.get_role_name()=='dialog' and n.get_name()=='Settings' and desktop.showing(n)))
        close=desktop.wait('settings close control',lambda:desktop.find(lambda n:n.get_role_name()=='button' and n.get_name()=='Close' and desktop.within(n,dialog) and desktop.showing(n)))
        desktop.activate(close)
    def apply(desktop):
        desktop.click('Apply message flow');desktop.wait('flow saved',lambda:desktop.text('Message flow saved'))
        request_close(desktop)
        desktop.wait('settings closed',lambda:not desktop.find(lambda n:n.get_role_name()=='dialog' and n.get_name()=='Settings' and desktop.showing(n)))
    def start(desktop):desktop.click('Continue')
    def focus(desktop,target):
        for _ in range(140):
            current=desktop.find(lambda n:n.get_state_set().contains(desktop.api.StateType.FOCUSED))
            if current and desktop.within(current,target):return
            keys([0xff09])
        raise RuntimeError(('Control is not keyboard reachable',target.get_name()))
    child=desktop=memory=log=None
    try:
        child,desktop,composer,memory,log=launch('codex','settings')
        settings(desktop,0,2);request_close(desktop)
        desktop.wait('dirty message-flow close guard',lambda:desktop.text('Discard settings changes?'));desktop.click('Keep editing')
        desktop.wait('discard confirmation dismissed',lambda:not desktop.text('Discard settings changes?'))
        path=root/'config/codex-thread-studio/settings.json';external=json.loads(path.read_text());external.update(queueDepth=2,continueBehavior='ollamaDraft');path.write_text(json.dumps(external));expected=path.read_bytes()
        desktop.click('Apply message flow');desktop.wait('visible settings conflict',lambda:desktop.text('changed in another client'))
        assert path.read_bytes()==expected,'Conflicting settings save replaced another writer'
        desktop.click('Reload message flow');desktop.wait('reloaded flow',lambda:desktop.text('Message flow saved'))
        choose(desktop,'Message queue depth',0,['1 message','2 messages','3 messages'])
        choose(desktop,'Continue behavior',2,['Session model draft · review first','Ollama draft · review first','Quick send · random prompt'])
        apply(desktop)
        assert json.loads((root/'config/codex-thread-studio/settings.json').read_text())['queueDepth']==1
        close(child,memory,log);child=None
        # The queue remains durable when its admission limit is reduced.
        service=runtime.fixture.Gateway(gateway,root)
        try:
            code,messages=service.api('/studio/session-state/message-queue?sessionKey=codex%3Atarget');assert code==200 and len(messages)==3
            code,_=service.api('/studio/session-state/message-queue',dict(sessionKey='codex:target',messages=[]),method='PUT');assert code==204
        finally:service.close()
        child,desktop,composer,memory,log=launch('codex','continue')
        settings(desktop,2,0);apply(desktop)
        path=root/'config/codex-thread-studio/settings.json';raw=json.loads(path.read_text());raw['continueBehavior']='quickSend';path.write_text(json.dumps(raw))
        start(desktop);desktop.wait('external mode change rejected',lambda:desktop.text('Continue behavior changed.'))
        assert not runtime.starts(root),'An external mode change triggered inference or a source send'
        raw['continueBehavior']='sessionModelDraft';path.write_text(json.dumps(raw))
        start(desktop);desktop.wait('session-model draft',lambda:desktop.contents(composer)==PROMPT)
        assert not [r for r in runtime.starts(root) if r['params']['threadId']=='target']
        desktop.edit(composer,'')
        (root/'hold-draft').touch();start(desktop);desktop.wait('held draft',lambda:(root/'draft-held').exists())
        desktop.edit(composer,HUMAN);(root/'release-draft').touch()
        desktop.wait('stale draft cleanup',lambda:desktop.find(lambda n:n.get_role_name()=='button' and n.get_name()=='Continue' and desktop.showing(n)))
        assert desktop.contents(composer)==HUMAN
        for name in ['hold-draft','release-draft','draft-held']:(root/name).unlink(missing_ok=True)
        desktop.edit(composer,'');(root/'hold-create').touch();start(desktop)
        desktop.wait('held utility creation',lambda:(root/'create-held').exists());desktop.click('Cancel draft')
        desktop.wait('creation cancellation admitted',lambda:desktop.text('Stopping draft…'));(root/'release-create').touch()
        desktop.wait('creation cancellation cleanup',lambda:desktop.find(lambda n:n.get_role_name()=='button' and n.get_name()=='Continue' and desktop.showing(n) and desktop.sensitive(n)))
        assert desktop.contents(composer)==''
        for name in ['hold-create','release-create','create-held']:(root/name).unlink(missing_ok=True)
        settings(desktop,behavior=1);apply(desktop)
        (root/'hold-local-draft').touch();start(desktop);desktop.wait('held local HTTP result',lambda:(root/'local-draft-held').exists())
        desktop.edit(composer,HUMAN)
        desktop.wait('local cancellation before upstream reply',lambda:desktop.find(lambda n:n.get_role_name()=='button' and n.get_name()=='Continue' and desktop.showing(n)))
        assert desktop.contents(composer)==HUMAN
        (root/'release-local-draft').touch()
        desktop.wait('late local reply completed',lambda:(root/'local-draft-released').exists());assert desktop.contents(composer)==HUMAN
        for name in ['hold-local-draft','release-local-draft','local-draft-held','local-draft-released']:(root/name).unlink(missing_ok=True)
        desktop.edit(composer,'');start(desktop)
        desktop.wait('local Ollama draft',lambda:desktop.contents(composer)=='Local continuation 中文😀');desktop.edit(composer,'')
        settings(desktop,behavior=2);apply(desktop)
        menu=desktop.wait('main menu control',lambda:desktop.find(lambda n:n.get_role_name()=='button' and n.get_name()=='Main menu' and desktop.showing(n)))
        focus(desktop,menu);before=len(runtime.starts(root));keys([0xffe3,0xffe1,0xff0d]);time.sleep(.2)
        assert len(runtime.starts(root))==before,'Continue shortcut escaped its composer scope'
        focus(desktop,composer)
        keys([0xffe3,0xffe1,0xff0d])
        desktop.wait('one explicit quick send',lambda:len([r for r in runtime.starts(root) if r['params']['threadId']=='target'])==1)
        desktop.wait('quick send retained empty composer',lambda:desktop.contents(composer)=='')
        close(child,memory,log);child=None
        raw=json.loads((root/'config/codex-thread-studio/settings.json').read_text());raw['continueBehavior']='sessionModelDraft';(root/'config/codex-thread-studio/settings.json').write_text(json.dumps(raw))
        child,desktop,composer,memory,log=launch('opencode','opencode')
        start(desktop);desktop.wait('OpenCode structured continuation',lambda:desktop.contents(composer)=='OpenCode continuation 中文😀')
        close(child,memory,log);child=None
    except BaseException:
        if desktop:(root/'accessibility.json').write_text(json.dumps(desktop.describe(),indent=2,ensure_ascii=False))
        raise
    finally:
        if child and child.poll() is None:child.terminate();child.wait(timeout=10);memory.close();log.close()
        owned.close()
    calls=runtime.requests(root);created=[r for r in calls if r.get('method')=='thread/start'];deleted=[r for r in calls if r.get('method')=='thread/delete'];turns=runtime.starts(root)
    assert len(created)==len(deleted)==3 and len([r for r in turns if r['params']['threadId'].startswith('continuation-')])==2
    normal=[r for r in turns if r['params']['threadId']=='target'];assert len(normal)==1 and normal[0]['params']['input'][0]['text'] in ['Continue.','Go on.','Keep going.','Please continue.','Continue with the task.']
    original=json.loads((root/'original-sources.json').read_text())
    assert json.loads((root/'codex-turns.json').read_text())['target'][0]==original['codex'],'A draft task changed the original Codex source'
    assert json.loads((root/'oc-turns.json').read_text())['oc-target']==original['opencode'],'An OpenCode utility changed its source conversation'
    oc=[json.loads(s) for s in (root/'opencode-requests.jsonl').read_text().splitlines()]
    assert len([r for r in oc if r['path'].endswith('/prompt_async')])==1 and len([r for r in oc if r['method']=='DELETE' and r['path'].startswith('/session/oc-continuation-')])==1
    for name in ['settings.log','continue.log','opencode.log']:assert not any(v in (root/name).read_text() for v in ['CRITICAL','Gtk-WARNING','panicked'])
    for name in ['backend.pid','opencode.pid','translation-gateway.pid','translation-proxy.pid']:
        if (root/name).exists():runtime.until(lambda:not Path('/proc',(root/name).read_text().strip()).exists(),seconds=10)
    result=dict(ok=True,noApplicationHooks=True,privateOllamaProxy=True,installedPair=os.environ.get('STUDIO_CONTINUATION_INSTALLED')=='1',gatewayOverride=True,nativeSettings=True,nativeSettingsConflict=True,dirtyCloseGuard=True,lowerDepthPreservesQueuedMessages=True,sessionModelDraft=True,localDraft=True,localCancellationBeforeReply=True,externalBehaviorCannotAutoSend=True,explicitQuickSend=True,shortcut=True,shortcutScopedToComposer=True,staleTypingPreserved=True,cancelDuringCreation=True,opencodeDraft=True,ownedUtilityCleanup=True,modelEffortFastPreserved=True,memory=measurements,artifacts=str(root))
    (root/'continuation-result.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('native');p.add_argument('gateway');p.add_argument('--api-only',action='store_true');p.add_argument('--private-root');p.add_argument('--installed',action='store_true');a=p.parse_args();resource.setrlimit(resource.RLIMIT_CORE,(0,0))
    native,gateway=str(Path(a.native).resolve()),str(Path(a.gateway).resolve())
    if a.private_root:return run(native,gateway,Path(a.private_root))
    base=Path(os.environ.get('STUDIO_NATIVE_QA_DIR') or Path(__file__).resolve().parents[1]/'target/gnome-native-smoke');base.mkdir(exist_ok=True,parents=True);root=Path(tempfile.mkdtemp(prefix='continuation-',dir=base));runtime.create_profile(root)
    if a.installed:
        bundle=Path(native).parent;prefix=root/'installed native 中文 % continue'
        subprocess.run(['bash',str(bundle/'install.sh'),str(bundle),str(prefix)],check=True,stdout=subprocess.DEVNULL)
        native,gateway=str(prefix/'bin/codex-thread-studio-gnome'),str(prefix/'bin/codex-thread-studio-gateway')
    result=api_checks(gateway,root);print(json.dumps(dict(result,artifacts=str(root))),flush=True)
    if a.api_only:return
    seed(gateway,root)
    (root/'codex-turns.json').write_text(json.dumps({'target':[dict(id='source-turn',status='completed',items=[dict(id='source-user',type='userMessage',content=[dict(type='text',text='Native request')]),dict(id='source-answer',type='agentMessage',text=ANSWER)])]}))
    (root/'oc-turns.json').write_text(json.dumps({'oc-target':[
        dict(info=dict(id='oc-source-user',role='user',sessionID='oc-target'),parts=[dict(id='oc-source-input',messageID='oc-source-user',sessionID='oc-target',type='text',text='Native request')]),
        dict(info=dict(id='oc-source-answer',role='assistant',sessionID='oc-target',parentID='oc-source-user',time=dict(completed=123)),parts=[dict(id='oc-source-part',messageID='oc-source-answer',sessionID='oc-target',type='text',text=ANSWER)]),
    ]}))
    (root/'original-sources.json').write_text(json.dumps(dict(codex=json.loads((root/'codex-turns.json').read_text())['target'][0],opencode=json.loads((root/'oc-turns.json').read_text())['oc-target'])))
    for name,value in [('fake-codex.py',codex()),('fake-opencode.py',opencode())]:path=root/name;path.write_text(value);path.chmod(0o700)
    raw=json.loads((root/'config/codex-thread-studio/settings.json').read_text());raw['translation']=dict(engine='backend',ollamaModel='fixture-local',models={},efforts={});raw['language']='en-US';(root/'config/codex-thread-studio/settings.json').write_text(json.dumps(raw))
    wrapper=root/'gateway-proxy.py';wrapper.write_text('#!/usr/bin/python3\nimport sys\nsys.dont_write_bytecode=True\nsys.path.insert(0,'+repr(str(Path(__file__).with_name('test-support').resolve()))+')\nfrom private_ollama_proxy import main\nraise SystemExit(main('+repr(gateway)+',sys.argv[1:]))\n');wrapper.chmod(0o700)
    env=dict(os.environ,CODEX_THREAD_STUDIO_GATEWAY_BIN=str(wrapper),CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),CODEX_THREAD_STUDIO_BACKENDS_CONFIG=str(root/'config/codex-thread-studio/backends.json'),CODEX_HOME=str(root/'codex'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),READER_CACHE_DIR=str(root/'reader-cache'),STUDIO_GNOME_PRIVATE_PROXY_DIR=str(root),GDK_BACKEND='wayland',GIO_USE_VFS='local',GIO_USE_VOLUME_MONITOR='unix',GTK_A11Y='atspi')
    for key in list(env):
        if key.startswith('STUDIO_GNOME_SMOKE'):env.pop(key)
    if a.installed:env['STUDIO_CONTINUATION_INSTALLED']='1'
    with isolated_environment(env,headless=True) as private:
        subprocess.run(['dbus-run-session','--',shutil.which('mutter') or 'mutter','--headless','--wayland','--no-x11','--wayland-display','studio-test','--virtual-monitor','1600x1000','--',sys.executable,str(Path(__file__).resolve()),native,gateway,'--private-root',str(root)],cwd=root,env=private,check=True,timeout=300)
if __name__=='__main__':main()
