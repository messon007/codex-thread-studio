#!/usr/bin/env python3
"""Real Wayland Router conversation controls against private deterministic backends."""
import base64
import importlib.util
import json
import os
import shutil
from pathlib import Path
import sqlite3
import subprocess
import sys
import tempfile
import time
import uuid

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location('router_runtime', Path(__file__).with_name('smoke-gnome-router-runtime.py'))
runtime = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runtime)
sys.path.insert(0,str(Path(__file__).with_name('test-support')))
from private_wayland import isolated_environment
from memory_sampler import ProcessMemory


def main():
    native, gateway = map(lambda p: str(Path(p).resolve()), sys.argv[1:3])
    if '--headless' in sys.argv and os.environ.get('STUDIO_ROUTER_OWNED_DESKTOP')!='1':
        with tempfile.TemporaryDirectory(prefix='studio-router-desktop-',dir='/tmp') as directory:
            private_root=Path(directory)
            environment=dict(os.environ,STUDIO_ROUTER_OWNED_DESKTOP='1',GDK_BACKEND='wayland',XDG_CONFIG_HOME=str(private_root/'config'),XDG_DATA_HOME=str(private_root/'data'),XDG_CACHE_HOME=str(private_root/'cache'))
            command=[shutil.which('mutter') or 'mutter','--headless','--wayland','--no-x11','--wayland-display','studio-test','--virtual-monitor','1600x1000','--']
            with isolated_environment(environment,headless=True) as private:
                subprocess.run(['dbus-run-session','--',*command,sys.executable,str(Path(__file__).resolve()),*sys.argv[1:]],env=private,check=True,timeout=240)
        return
    queue_refs_mode = "--queue-refs" in sys.argv[3:]
    queue_mode = "--queue" in sys.argv[3:] or queue_refs_mode
    image_mode = "--images" in sys.argv[3:]
    refs_mode = "--refs" in sys.argv[3:]
    artifacts = Path(__file__).resolve().parents[1] / 'target/gnome-native-smoke'
    artifacts.mkdir(parents=True,exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix='router-ui-', dir=artifacts))
    runtime.create_profile(root)
    settings = root / 'config/codex-thread-studio/settings.json'
    preferences = json.loads(settings.read_text())
    (settings.parent / 'gnome.json').write_text(json.dumps({'selectedBackend':'codex','selectedSession':'router'}))
    settings.write_text(json.dumps(preferences))
    backend = runtime.CODEX.replace("assert params['includeTurns'] is False", "pass")
    backend = backend.replace("answer(identity,{'thread':value})", "value['turns']=turns.get(session,[]) if params.get('includeTurns') else [];answer(identity,{'thread':value})")
    backend = backend.replace("assert params['excludeTurns'] is True", "pass")
    backend = backend.replace("answer(identity,{'thread':summary(session)})", "value=summary(session);value['turns']=[] if params.get('excludeTurns') or (session=='target' and (root/'trim-target-live-tail').exists()) else turns.get(session,[]);answer(identity,{'thread':value})")
    backend = backend.replace("'items':[{'id':turn_id+'-response','type':'agentMessage','text':text}]", "'items':[{'id':turn_id+'-user','type':'userMessage','content':params['input']},{'id':turn_id+'-response','type':'agentMessage','text':text}]")
    backend = backend.replace("    elif method=='turn/start':", "    elif method=='model/list':answer(identity,{'data':[{'id':'controller-model','model':'controller-model','isDefault':True,'supportedReasoningEfforts':[]}]})\n    elif method=='turn/start':")
    backend = backend.replace("    elif method=='thread/turns/list':", """    elif method=='thread/items/list':
        assert params.get('limit')==120 and params.get('sortDirection')=='desc',params
        items=[] if session=='target' and (root/'trim-target-live-tail').exists() else [
            {'turnId':turn['id'],'item':item} for turn in reversed(turns.get(session,[])) for item in reversed(turn.get('items',[]))]
        answer(identity,{'data':items[:120],'nextCursor':None})
    elif method=='thread/turns/list':""")
    backend = backend.replace("        value=summary(session)", "        if (root/'delay-target-summary').exists() and session=='target' and params.get('includeTurns') is False:\n            def delayed(identity=identity,session=session):time.sleep(.6);answer(identity,{'thread':summary(session)})\n            threading.Thread(target=delayed,daemon=True).start();continue\n        value=summary(session)")
    backend = backend.replace("summary('target'),", "*([summary('target')] if not (root/'omit-target-catalog').exists() else []),")
    backend = backend.replace("summary('router'),", "summary('router'),*([{**summary('page-'+str(index)),'name':'Paged document '+str(index)} for index in range(60)] if (root/'picker-pages').exists() else []),")
    opencode = runtime.OPENCODE.replace("        identity=path.split('/')[2]", "        if path=='/config/providers':return self.response({'providers':[{'id':'provider','models':{'oc-target-model':{},'controller-model':{}}}]})\n        if path=='/global/event':\n            self.send_response(200);self.send_header('Content-Type','text/event-stream');self.end_headers()\n            try:\n                while True:self.wfile.write(b': heartbeat\\n\\n');self.wfile.flush();time.sleep(.2)\n            except (BrokenPipeError,ConnectionResetError):pass\n            return\n        identity=path.split('/')[2]")
    opencode = opencode.replace("[session('oc-router'),session('oc-target')]", "([session('oc-router')]+([] if (root/'omit-target-catalog').exists() else [session('oc-target')]))")
    for name, source in [('fake-codex.py', backend), ('fake-opencode.py', opencode)]:
        path = root / name
        path.write_text(source)
        path.chmod(0o700)
    environment = dict(os.environ, CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway,
                       CODEX_THREAD_STUDIO_CODEX_BIN=str(root / 'fake-codex.py'),
                       CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root / 'fake-opencode.py'),
                       CODEX_HOME=str(root / 'codex'), XDG_CONFIG_HOME=str(root / 'config'),
                       XDG_DATA_HOME=str(root / 'data'), XDG_CACHE_HOME=str(root / 'cache'),
                       READER_CACHE_DIR=str(root / 'reader-cache'), GDK_BACKEND='wayland',
                       STUDIO_GNOME_SMOKE_DIR=str(root), STUDIO_GNOME_SMOKE_ROUTER='1')
    environment.pop('DISPLAY', None)
    environment.pop('XAUTHORITY', None)
    if refs_mode:
        environment["STUDIO_GNOME_SMOKE_ROUTER_REFS"]="1"
        with (root/'project/参考 large.md').open('wb') as document:
            document.write(b'PRIVATE REFERENCE CONTENT NEVER IN ROUTER TRANSPORT')
            document.truncate(30*1024*1024)
        (root/'project/newer ref.md').write_text('# Newer reference draft\n')
        (root/'project/ref-note.md').write_text('# Target project reference\n')
    if image_mode:
        environment["STUDIO_GNOME_SMOKE_ROUTER_IMAGES"]="1"
        png=base64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/l9sAAAAASUVORK5CYII=')
        (root/'router-image.png').write_bytes(png+b'\0'*(2*1024*1024))
        (root/'router-image-new.png').write_bytes(png)
    if queue_mode: environment["STUDIO_GNOME_SMOKE_ROUTER_QUEUE"]="1"
    if queue_refs_mode:
        environment['STUDIO_GNOME_SMOKE_ROUTER_QUEUE_REFS']='1'
        with (root/'project/queued 参考.md').open('wb') as document:
            document.write(b'PRIVATE QUEUED FILE CONTENT')
            document.truncate(30*1024*1024)
        (root/'project/newer queue ref.md').write_text('# Newer reference\n')
    for phase in (("initial",) if image_mode or refs_mode else ("initial","restore") if queue_mode else ("initial","restore","history")):
        if phase == 'restore':
            before = len(runtime.starts(root))
            environment['STUDIO_GNOME_SMOKE_ROUTER_RESTORE'] = '1'
        if phase == 'history':
            environment.pop('STUDIO_GNOME_SMOKE_ROUTER_RESTORE', None)
            environment['STUDIO_GNOME_SMOKE_ROUTER_HISTORY'] = '1'
            identities = []
            with sqlite3.connect(settings.parent / 'studio.sqlite3') as db:
                for index in range(50):
                    identity = str(uuid.uuid4()); identities.append(identity)
                    unread = index in (0, 25)
                    job = {'id':identity,'fingerprint':'history-fixture','controller':'codex:router','createdAt':1,'prompt':f'Historical request {index}','status':'clarify','controllerTurnId':'','target':None,'targetTurnId':'','decision':None,'response':f'Historical reply {index}','responseItems':[],'error':'','unread':unread}
                    if index==0:job['prompt']+=' ÉTÉ 中文😀 []%_.* '
                    db.execute('INSERT INTO native_router_jobs(id,controller,created,owner,phase,json,receipt,unread) VALUES(?,?,?,?,?,?,?,?)',(identity,'codex:router',1,'private-history-fixture','clarify',json.dumps(job),uuid.uuid4().hex,unread))
            (root / 'history-seed.json').write_text(json.dumps(identities))
        result_path = root / 'router-ui.json'
        result_path.unlink(missing_ok=True)
        with (root / f'{phase}.log').open('w') as log:
            child = subprocess.Popen([native], env=environment, cwd=root, stdout=log, stderr=log)
            memory = ProcessMemory(child.pid, gateway)
            try:
                child.wait(timeout=65)
            except subprocess.TimeoutExpired:
                child.terminate()
                child.wait(timeout=10)
                raise AssertionError(f'Native Router {phase} timed out: {root}')
            finally:
                measured_memory = memory.close()
        assert child.returncode == 0, (phase, root, child.returncode)
        assert result_path.exists(), (phase, root, 'Native exited without a report', (root / f'{phase}.log').read_text()[-1800:])
        result = json.loads(result_path.read_text())
        result['observedMemory'] = measured_memory
        assert result['ok'], (root, result)
        if phase=='initial' and queue_refs_mode:
            assert result['lostAckSnapshot'], result
        if phase == 'initial' and not queue_mode and not image_mode and not refs_mode:
            assert result['routerTargetLanguage'] and result['composerInstructionLanguage'] and result['localePreservesTargetAndDraft'], result
            assert result['rejectedDraftRetention'] and result['restoredControllerMetadata'] and result['targetNavigation'] and result['navigationDrafts'] and result['lateNavigationGuard'], result
        (root / f'{phase}.json').write_text(json.dumps(result, indent=2))
        if phase in ('restore', 'history') and not queue_mode:
            assert len(runtime.starts(root)) == before, 'Reopening the native Router resent a prompt'
        if phase == 'history':
            assert result['search']['controllerSwitchIsolation'] and result['search']['staleQueryGuard'], result
    if refs_mode:
        assert result['crossBackendCompletion'] and result['staleTargetCompletion'] and result['openCodeCommand'] and result['pendingCommandStop'], result
        starts=runtime.starts(root)
        controllers=[r for r in starts if r['params']['threadId']=='router']
        targets=[r for r in starts if r['params']['threadId']=='target']
        assert len(controllers)==6 and len(targets)==2, (len(controllers),len(targets))
        assert all(all(part['type']=='text' for part in r['params']['input']) for r in controllers), 'Controller activated typed target references'
        first=targets[0]['params']['input'];assert [part['type'] for part in first]==['text','skill','mention'], first
        assert str(root/'project/参考 large.md') in first[0]['text']
        assert all('PRIVATE REFERENCE CONTENT' not in json.dumps(r) for r in runtime.requests(root))
        assert len(json.dumps(runtime.requests(root)))<200000, 'File contents unexpectedly entered routing transport'
        with sqlite3.connect(settings.parent/'studio.sqlite3') as db:
            jobs=[json.loads(r[0]) for r in db.execute('SELECT json FROM native_router_jobs')]
            assert len(jobs)==7 and sorted(j['status'] for j in jobs)==['clarify','completed','completed','completed','completed','failed','failed'], jobs
        commands=[json.loads(line) for line in (root/'opencode-requests.jsonl').read_text().splitlines() if json.loads(line)['path'].endswith('/command') and json.loads(line)['method']=='POST']
        assert len(commands)==1 and commands[0]['body']['command']=='oc-build' and commands[0]['body']['arguments']=='Run native OpenCode skill 中文😀 '
        assert commands[0]['body']['parts'][0]['url']==(root/'project-oc/oc-reference.md').as_uri()
        assert not any(json.loads(line)['path'].endswith('/abort') for line in (root/'opencode-requests.jsonl').read_text().splitlines())
        for name in ('backend.pid','opencode.pid'):
            pid=int((root/name).read_text())
            for _ in range(100):
                if not Path(f'/proc/{pid}').exists():break
                time.sleep(.05)
            assert not Path(f'/proc/{pid}').exists(), f'Owned backend {pid} survived reference fixture'
        print(json.dumps({'ok':True,'artifacts':str(root),'routerReferences':True,'metadataOnlyLargeFile':True,'newerDraftRetention':True,'lateScopeGuard':True,'codexFileCompletion':True,'targetInventoryGuard':True,'crossBackendFile':True,'openCodeSkillRetention':True,'crossBackendCompletion':True,'staleTargetCompletion':True,'openCodeCommand':True,'pendingCommandStop':True}))
        return
    if image_mode:
        starts=runtime.starts(root)
        assert len([r for r in starts if r['params']['threadId']=='router'])==3, starts
        targets=[r for r in starts if r['params']['threadId']=='target']
        assert len(targets)==1 and len(targets[0]['params']['input'])==2 and len(targets[0]['params']['input'][1]['url'])>2*1024*1024
        with sqlite3.connect(settings.parent/'studio.sqlite3') as db:
            raw=[r[0] for r in db.execute('SELECT json FROM native_router_jobs')]
            jobs=list(map(json.loads,raw))
            assert len(jobs)==3 and sorted(j['status'] for j in jobs)==['clarify','completed','completed'] and all(j['imageCount']==1 for j in jobs), jobs
            assert all('data:image' not in text and 'base64' not in text for text in raw), 'Image payload persisted in job history'
        for name in ('backend.pid','opencode.pid'):
            pid=int((root/name).read_text())
            for _ in range(100):
                if not Path(f'/proc/{pid}').exists(): break
                time.sleep(.05)
            assert not Path(f'/proc/{pid}').exists(), f'Owned backend {pid} survived image fixture'
        print(json.dumps({'ok':True,'artifacts':str(root),'routerImages':True,'imageOnly':True,'largeBody':True,'crossBackend':True,'rejectedDraftRetention':True,'clarificationRetention':True,'newerDraftAndImage':True,'noImagePayloadInHistory':True}))
        return
    if queue_mode:
        starts=runtime.starts(root)
        assert len([r for r in starts if r['params']['threadId']=='router'])==4, starts
        assert len([r for r in starts if r['params']['threadId']=='target'])==3, starts
        if queue_refs_mode:
            assert result['queuedReferences'] and result['missingFileRetained'], result
            target=[r for r in starts if r['params']['threadId']=='target'][-1]['params']['input']
            assert [part['type'] for part in target]==['text','skill','mention'], target
            assert str(root/'project/queued 参考.md') in target[0]['text']
            oc=[json.loads(line) for line in (root/'opencode-requests.jsonl').read_text().splitlines() if json.loads(line)['path'].endswith('/prompt_async')]
            assert len(oc)==1 and oc[0]['body']['parts'][1]['type']=='file'
            assert oc[0]['body']['parts'][1]['url']==(root/'project/queued 参考.md').as_uri()
            assert all('PRIVATE QUEUED FILE CONTENT' not in json.dumps(request) for request in runtime.requests(root))
        with sqlite3.connect(settings.parent / 'studio.sqlite3') as db:
            jobs=[json.loads(row[0]) for row in db.execute('SELECT json FROM native_router_jobs')]
            assert len(jobs)==4 and sorted(row['status'] for row in jobs)==['completed','completed','completed','stopped'], jobs
            if queue_refs_mode:
                assert sorted(len(row.get('referenceLabels',[])) for row in jobs)==[0,0,1,3], 'Queued reference labels were lost from delivery history'
            assert db.execute('SELECT COUNT(*) FROM session_message_queues WHERE session_key=?',('codex:router',)).fetchone()[0]==0
            assert db.execute('SELECT COUNT(*) FROM native_router_claims').fetchone()[0]==0
        for name in ('backend.pid','opencode.pid'):
            pid=int((root/name).read_text())
            for _ in range(100):
                if not Path(f'/proc/{pid}').exists(): break
                time.sleep(.05)
            assert not Path(f'/proc/{pid}').exists(), f'Owned backend {pid} survived queue fixture'
        print(json.dumps({'ok':True,'artifacts':str(root),'routerQueue':True,'fullDeliveryGate':True,'crossBackend':True,'restoredPaused':True,'explicitResume':True,'ownedBackendsReaped':True,'queuedReferences':queue_refs_mode}))
        return
    starts = runtime.starts(root)
    controllers = [request for request in starts if request['params']['threadId'] == 'router']
    targets = [request for request in starts if request['params']['threadId'] == 'target']
    assert len(controllers) == 2 and len(targets) == 2, starts
    assert targets[-1]['params']['input'][0]['text']=='Native Router UI stop request', 'Explicit selection rewrote the request or called the controller'
    assert all('outputSchema' in request['params'] for request in controllers), 'Controller used the ordinary model-send path'
    assert all(request['params'].get('sandbox') == 'read-only' for request in runtime.requests(root) if request.get('method') == 'thread/resume' and request['params']['threadId'] == 'router'), 'Ordinary history resume mutated the Router controller'
    assert any(request.get('method') == 'thread/read' and request['params']['threadId'] == 'router' and request['params']['includeTurns'] is False for request in runtime.requests(root)), 'Controller metadata was not loaded'
    interrupts = [request for request in runtime.requests(root) if request.get('method') == 'turn/interrupt']
    assert len(interrupts) == 1 and interrupts[0]['params']['threadId'] == 'target', interrupts
    with sqlite3.connect(settings.parent / 'studio.sqlite3') as db:
        rows = db.execute('SELECT json FROM native_router_jobs').fetchall()
        assert len(rows)==53 and sorted(json.loads(row[0])['status'] for row in rows).count('clarify')==50
        assert db.execute('SELECT COUNT(*) FROM native_router_jobs WHERE unread!=0').fetchone()[0]==0
        favorites = db.execute('SELECT backend,thread_id,project_path,turn_id,item_id,question FROM favorites').fetchall()
        assert len(favorites) == 2, favorites
        for backend, thread, project, turn, item, question in favorites:
            assert (backend, thread) in [('codex', 'target'), ('opencode', 'oc-target')], favorites
            assert project == str(root / ('project' if backend == 'codex' else 'project-oc')), favorites
            assert turn.startswith('target-turn-' if backend == 'codex' else 'oc-target-user-'), favorites
            assert item == turn + ('-response' if backend == 'codex' else '-answer-part'), favorites
            assert question.startswith('Forwarded 中文: '), favorites
    for name in ('router-codex-reply.png', 'router-opencode-reply.png', 'router-stopped.png', 'router-restored.png', 'router-history-middle.png', 'router-history-latest.png', 'router-live-codex.png', 'router-live-opencode.png', 'router-target-historical.png'):
        assert (root / name).stat().st_size > 1000
    for name in ('backend.pid', 'opencode.pid'):
        pid = int((root / name).read_text())
        for _ in range(100):
            if not Path(f'/proc/{pid}').exists():
                break
            time.sleep(.05)
        assert not Path(f'/proc/{pid}').exists(), f'Owned backend {pid} survived native app exit'
    print(json.dumps({'ok': True, 'artifacts': str(root), 'wayland': True,
                      'routerTargetLanguage': True, 'composerInstructionLanguage': True, 'localePreservesTargetAndDraft': True,
                      'nativeSendStop': True, 'crossBackendSources': True,
                      'favorites': True, 'restartNoReplay': True, 'sharedUnread': True, 'historyPaging': True, 'targetNavigation': True}))


if __name__ == '__main__':
    main()
