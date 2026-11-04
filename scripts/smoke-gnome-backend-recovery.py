#!/usr/bin/env python3
"""Drop owned backend transports; never contact real providers or user sessions."""
import argparse,importlib.util,json,os,resource,shutil,sqlite3,subprocess,sys,tempfile,time
from pathlib import Path
sys.dont_write_bytecode=True
sys.path.insert(0,str(Path(__file__).with_name('test-support')))
from private_wayland import isolated_environment
from private_opencode_events import SOURCE as EVENTS
from private_codex_exit import instrument as instrument_exit
from private_codex_lag import instrument as instrument_lag
from memory_sampler import ProcessMemory
spec=importlib.util.spec_from_file_location('recovery_runtime',Path(__file__).with_name('smoke-gnome-router-runtime.py'))
runtime=importlib.util.module_from_spec(spec);spec.loader.exec_module(runtime)

def child(native,gateway,root):
    with (root/'app.log').open('w') as log:
        app=subprocess.Popen([native],cwd=root,env=os.environ,stdout=log,stderr=log);memory=ProcessMemory(app.pid,gateway)
        try:code=app.wait(timeout=100)
        except BaseException:app.terminate();app.wait(timeout=10);raise
        finally:measured=memory.close()
    result=json.loads((root/'backend-recovery.json').read_text());assert code==0 and result['ok'],(root,result)
    assert not any(s in (root/'app.log').read_text() for s in ['CRITICAL','panicked','Gtk-WARNING']),root
    starts=runtime.starts(root);assert len(starts)==1 and starts[0]['params']['input'][0]['text']=='Uncertain fake prompt 中文😀',starts
    requests=runtime.requests(root)
    assert not any(r.get('method') in ['thread/shellCommand','review/start','turn/interrupt'] for r in requests)
    traces=[json.loads(s) for s in (root/'transport-events.jsonl').read_text().splitlines()]
    process=os.environ.get('STUDIO_GNOME_SMOKE_PROCESS_RECOVERY')=='1'
    gap=os.environ.get('STUDIO_GNOME_SMOKE_EVENT_GAP')=='1'
    assert sum(e['kind']=='dropped' and e['transport']=='codex' for e in traces)>=(1 if process or gap else 3),traces
    assert sum(e['kind']=='dropped' and e['transport']=='opencode' for e in traces)>=3,traces
    if os.environ.get('STUDIO_GNOME_SMOKE_STARTUP_RECOVERY')=='1':
        assert result['initialCodex'] and result['initialOpenCode'] and result['startupDraftRetained'],result
        for transport in ['codex','opencode']:
            rejected=[e for e in traces if e['kind']=='rejected' and e['transport']==transport]
            opened=[e for e in traces if e['kind']=='opened' and e['transport']==transport]
            assert len(rejected)>=2 and opened and max(e['at'] for e in rejected)<min(e['at'] for e in opened),traces
    if process:
        assert result['selectedProcessRecovery'] and result['backgroundProcessRecovery'],result
        exits=[json.loads(line) for line in (root/'codex-process-exits.jsonl').read_text().splitlines()]
        pids=[json.loads(line)['pid'] for line in (root/'codex-process-starts.jsonl').read_text().splitlines()]
        assert len(exits)==2 and len(set(pids))>=3,(exits,pids)
        opened=[e['at'] for e in traces if e['kind']=='opened' and e['transport']=='codex']
        for event in exits:
            assert event['pid'] in pids and event['code']==23 and any(at>event['at'] for at in opened),(exits,traces)
        assert not any(Path('/proc',str(pid)).exists() for pid in pids),('Owned Codex process survived',pids)
    if gap:
        assert result['selectedGapRecovery'] and result['backgroundGapRecovery'],result
        assert sum(e['kind']=='lagged' for e in traces)>=2,traces
        assert sum(e['kind']=='held' for e in traces)==2 and sum(e['kind']=='resumed' for e in traces)==2,traces
        assert sum(r.get('method')=='initialize' for r in requests)==1,'Event-gap recovery restarted the provider'
        assert json.loads((root/'gap-published.json').read_text())['sequence']==2
    closing=(root/'closing-recovery-fixture').stat().st_mtime_ns
    assert not any(e['kind']=='opened' and e['at']>closing for e in traces),'Retry opened a transport after accepted close'
    for p in root.glob('*.pid'):
        for _ in range(100):
            if not Path('/proc',p.read_text().strip()).exists():break
            time.sleep(.05)
        assert not Path('/proc',p.read_text().strip()).exists(),('Owned process survived',p)
    oc=[json.loads(s) for s in (root/'opencode-requests.jsonl').read_text().splitlines()]
    assert not any(e['path'].endswith(('/prompt_async','/command','/shell','/abort')) for e in oc),oc
    with sqlite3.connect(root/'config/codex-thread-studio/studio.sqlite3') as db:
        queue=json.loads(db.execute("SELECT queue_json FROM session_message_queues WHERE session_key='codex:target'").fetchone()[0])
    assert len(queue)==1 and queue[0]['text']=='Do not automatically send this queued message',queue
    timing=[json.loads(p.read_text()) for p in sorted(root.glob('gap-responsiveness-*.json'))]
    if gap:assert len(timing)==2 and all(t['mainLoopTicks']>0 for t in timing),timing
    print(json.dumps(dict(ok=True,checks=result,oneExplicitFakeSend=True,neverReplayed=True,transports=traces,eventGapMeasurements=timing,memory=measured,ownedCleanup=True,artifacts=str(root))))

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('native');p.add_argument('gateway');p.add_argument('--private-root');mode=p.add_mutually_exclusive_group();mode.add_argument('--startup-failures',action='store_true');mode.add_argument('--process-exit',action='store_true');mode.add_argument('--event-gap',action='store_true');args=p.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE,(0,0));native,gateway=str(Path(args.native).resolve()),str(Path(args.gateway).resolve())
    if args.private_root:return child(native,gateway,Path(args.private_root))
    base=Path(__file__).resolve().parents[1]/'target/gnome-native-smoke';base.mkdir(exist_ok=True,parents=True)
    root=Path(tempfile.mkdtemp(prefix='backend-recovery-',dir=base));runtime.create_profile(root)
    settings=root/'config/codex-thread-studio/settings.json';raw=json.loads(settings.read_text());raw['language']='en-US';settings.write_text(json.dumps(raw))
    (settings.parent/'gnome.json').write_text(json.dumps(dict(selectedBackend='codex',selectedSession='target')))
    with sqlite3.connect(settings.parent/'studio.sqlite3') as db:
        db.execute('CREATE TABLE session_message_queues(session_key TEXT PRIMARY KEY,queue_json TEXT NOT NULL)')
        db.execute('INSERT INTO session_message_queues VALUES(?,?)',('codex:target',json.dumps([dict(id='recovery-queue',text='Do not automatically send this queued message',input=[],createdAt=123)])))
    code=runtime.CODEX.replace("    elif method=='thread/read':", "    elif method=='model/list':answer(identity,{'data':[]})\n    elif method=='thread/items/list':answer(identity,error='Method not found')\n    elif method=='thread/read':")
    catalog=next(line for line in code.splitlines() if "elif method=='thread/list':answer" in line)
    code=code.replace(catalog,"    elif method=='thread/list':\n        while (root/'hold-reconnect-catalog').exists():time.sleep(.01)\n        answer"+catalog.split(':answer',1)[1])
    if args.process_exit:code=instrument_exit(code)
    if args.event_gap:code=instrument_lag(code)
    oc=runtime.OPENCODE.replace("messages=json.loads(ledger.read_text()) if ledger.exists() else {}","messages=json.loads(ledger.read_text()) if ledger.exists() else {}\n"+EVENTS)
    oc=oc.replace("        if path=='/global/health':", "        if path=='/global/event':return stream_sdk(self)\n        if path=='/config/providers':return self.response({'providers':[{'id':'provider','models':{'oc-target-model':{}}}]})\n        if path=='/global/health':")
    for name,body in [('fake-codex.py',code),('fake-opencode.py',oc)]:path=root/name;path.write_text(body);path.chmod(0o700)
    wrapper=root/'gateway-proxy.py';wrapper.write_text('#!/usr/bin/python3\nimport sys\nsys.dont_write_bytecode=True\nsys.path.insert(0,'+repr(str(Path(__file__).with_name('test-support').resolve()))+')\nfrom private_ollama_proxy import main\nraise SystemExit(main('+repr(gateway)+',sys.argv[1:],faults=True))\n');wrapper.chmod(0o700)
    env=dict(os.environ,CODEX_THREAD_STUDIO_GATEWAY_BIN=str(wrapper),CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),CODEX_HOME=str(root/'codex'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),READER_CACHE_DIR=str(root/'reader-cache'),GDK_BACKEND='wayland',G_DEBUG='fatal-criticals',STUDIO_GNOME_SMOKE_DIR=str(root),STUDIO_GNOME_SMOKE_BACKEND_RECOVERY='1')
    if args.startup_failures:
        env['STUDIO_GNOME_SMOKE_STARTUP_RECOVERY']='1'
        (root/'reject-codex').write_text('1')
    if args.process_exit:
        env['STUDIO_GNOME_SMOKE_PROCESS_RECOVERY']='1'
    if args.event_gap:
        env['STUDIO_GNOME_SMOKE_EVENT_GAP']='1'
    with isolated_environment(env,headless=True) as private:
        subprocess.run(['dbus-run-session','--',shutil.which('mutter') or 'mutter','--headless','--wayland','--no-x11','--wayland-display','studio-test','--virtual-monitor','1600x1000','--',sys.executable,str(Path(__file__).resolve()),native,gateway,'--private-root',str(root)],env=private,cwd=root,check=True,timeout=120)
if __name__=='__main__':main()
