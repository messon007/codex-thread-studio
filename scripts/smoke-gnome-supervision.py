#!/usr/bin/env python3
"""Real native supervision/utility/queue/Router workflows, private providers only."""
import argparse
import ast
import importlib.util
import json
import os
from pathlib import Path
import resource
import sqlite3
import shutil
import subprocess
import sys
import tempfile

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).with_name("test-support")))
from private_wayland import isolated_environment
from memory_sampler import ProcessMemory

spec = importlib.util.spec_from_file_location("supervision_runtime", Path(__file__).with_name("smoke-gnome-router-runtime.py"))
runtime = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runtime)

HELPERS = r'''
utilities={};utility_sequence=0
def emit(value):
    with lock:print(json.dumps(value),flush=True)
def restored_completion():
    if not (root/'restore-mode').exists() or not (root/'release-source-1').exists():return
    with lock:
        saved=next((t for t in turns.get('target',[]) if t['id']=='target-turn-1'),None)
        if not saved or saved['status']!='inProgress':return
        saved['status']='completed';item={'id':saved['id']+'-answer','type':'agentMessage','text':'A done; B remains'};saved['items'].append(item);save()
    emit({'method':'item/completed','params':{'threadId':'target','turnId':saved['id'],'item':item}})
    emit({'method':'turn/completed','params':{'threadId':'target','turn':saved}})
def complete_source(session,turn,prompt,count):
    if not prompt.startswith('Queued '):
        (root/('source-held-'+str(count))).write_text(turn)
        while not (root/('release-source-'+str(count))).exists():time.sleep(.01)
    text='A done; B remains' if count in [1,5,8] else 'All work complete 中文😀'
    item={'id':turn+'-answer','type':'agentMessage','text':text}
    with lock:
        saved=next(t for t in turns[session] if t['id']==turn)
        if saved['status']!='inProgress':return
        saved['status']='completed';saved['items'].append(item);save()
    emit({'method':'item/completed','params':{'threadId':session,'turnId':turn,'item':item}})
    emit({'method':'turn/completed','params':{'threadId':session,'turn':saved}})
def complete_evaluation(session,count,decision):
    (root/('evaluation-held-'+str(count))).touch()
    if count in [1,3,5]:
        while not (root/('release-evaluation-'+str(count))).exists():
            with lock:
                if session not in utilities:return
            time.sleep(.01)
    item={'id':session+'-answer','type':'agentMessage','text':json.dumps(decision,ensure_ascii=False)}
    with lock:
        if session not in utilities:return
        turn={'id':session+'-turn','status':'completed','items':[item]};utilities[session]['turns']=[turn]
    # Omit event output deliberately; supervision must read authoritative output.
    emit({'method':'turn/completed','params':{'threadId':session,'turn':{'id':turn['id'],'status':'completed'}}})
'''

METHODS = r'''
    elif method=='thread/start' and params.get('ephemeral') is True:
        assert params['approvalPolicy']=='never' and params['sandbox']=='read-only'
        assert params['developerInstructions'].startswith('Judge whether the original user task is complete')
        assert params['cwd']==str(root/'project') and params['model']=='target-model',params
        utility_sequence+=1;utility_id='native-supervisor-'+str(utility_sequence)
        utilities[utility_id]={'id':utility_id,'cwd':params['cwd'],'ephemeral':True,'turns':[]}
        answer(identity,{'thread':utilities[utility_id]})
    elif method=='turn/start' and session.startswith('native-supervisor-'):
        assert session in utilities and params['model']=='target-model' and params['effort']=='low'
        assert params['outputSchema']['properties']['decision']['enum']==['complete','continue','needs_user','error']
        evidence=json.loads(params['input'][0]['text']);assert evidence['originalUserMessage']
        unfinished='B remains' in evidence['latestAssistantMessage']
        prompt='Finish B' if utility_sequence==1 else 'Finish routed B'
        evaluation_decision={'decision':'continue' if unfinished else 'complete','reason':'B remains' if unfinished else 'All requested work is done','prompt':prompt if unfinished else ''}
        turn={'id':session+'-turn','status':'inProgress','items':[]};utilities[session]['turns']=[turn]
        answer(identity,{'turn':{'id':turn['id'],'status':'inProgress'}})
        threading.Thread(target=complete_evaluation,args=(session,utility_sequence,evaluation_decision),daemon=True).start()
    elif method=='thread/read' and session.startswith('native-supervisor-'):
        assert params['includeTurns'] is True and session in utilities
        answer(identity,{'thread':utilities[session]})
    elif method=='thread/delete' and session.startswith('native-supervisor-'):
        assert session in utilities;utilities.pop(session);(root/('utility-cleaned-'+session.split('-')[-1])).touch();answer(identity,{})
    elif method=='turn/interrupt' and session.startswith('native-supervisor-'):
        assert session in utilities;utilities[session]['turns'][-1]['status']='interrupted';answer(identity,{})
    elif method=='model/list':answer(identity,{'data':[{'id':'target-model','model':'target-model','isDefault':True,'serviceTiers':[{'id':'fast'}],'supportedReasoningEfforts':[{'reasoningEffort':'low'}]}]})
    elif method=='thread/turns/list' and (root/'restore-mode').exists():
        restored_completion()
        with lock:rows=list(reversed(turns.get(session,[])))[:4]
        answer(identity,{'data':rows,'nextCursor':None})
    elif method=='thread/items/list':
        answer(identity,{'data':[{'turnId':t['id'],'item':i} for t in reversed(turns.get(session,[])) for i in reversed(t['items'])],'nextCursor':None})
    elif method=='turn/start' and session=='target':
        prompt=params['input'][0]['text']
        assert params.get('model')=='target-model' and params.get('effort')=='low',params
        with lock:
            history=turns.setdefault(session,[]);count=len(history)+1;turn='target-turn-'+str(count)
            user={'id':turn+'-user','type':'userMessage','content':params['input']}
            history.append({'id':turn,'status':'inProgress','items':[user]});save()
        emit({'method':'turn/started','params':{'threadId':session,'turn':{'id':turn,'status':'inProgress'}}})
        emit({'method':'item/completed','params':{'threadId':session,'turnId':turn,'item':user}})
        answer(identity,{'turn':{'id':turn,'status':'inProgress'}})
        threading.Thread(target=complete_source,args=(session,turn,prompt,count),daemon=True).start()
'''


def source():
    value = runtime.CODEX.replace("for raw in sys.stdin:", HELPERS + "\nfor raw in sys.stdin:")
    value = value.replace("    elif method=='thread/read':", METHODS + "\n    elif method=='thread/read':")
    ast.parse(value)
    return value


def gui(native, gateway, root):
    with (root / "native.log").open("w") as log:
        app = subprocess.Popen([native], cwd=root, env=os.environ, stdout=log, stderr=log)
        memory = ProcessMemory(app.pid, gateway)
        try:
            app.wait(timeout=160)
        except BaseException:
            app.terminate()
            app.wait(timeout=10)
            raise
        finally:
            measured = memory.close()
    result_path = root / "supervision-result.json"
    result = json.loads(result_path.read_text()) if result_path.exists() else {}
    assert app.returncode == 0 and result.get("ok"), (str(root), result)
    assert not any(word in (root / "native.log").read_text() for word in ["CRITICAL", "panicked", "Gtk-WARNING"])
    requests = runtime.requests(root)
    sources = [r for r in requests if r.get("method") == "turn/start" and r["params"]["threadId"] == "target"]
    utilities = [r for r in requests if r.get("method") == "thread/start"]
    deletions = [r for r in requests if r.get("method") == "thread/delete"]
    restoring=(root/'restore-mode').exists()
    assert len(sources) == (3 if restoring else 8) and len(utilities) == (2 if restoring else 5) and len(deletions) == len(utilities), (len(sources), len(utilities), len(deletions))
    assert [r["params"]["input"][0]["text"] for r in sources if r["params"]["input"][0]["text"].startswith("[Studio turn supervisor]")] == (["[Studio turn supervisor]\nFinish B"] if restoring else ["[Studio turn supervisor]\nFinish B", "[Studio turn supervisor]\nFinish routed B"])
    assert all(r["params"]["serviceTier"] == "fast" for r in sources[1:])
    assert len([r for r in requests if r.get("method") == "turn/start" and r["params"]["threadId"] == "router"]) == 2
    created = {"native-supervisor-" + str(i) for i in range(1, 3 if restoring else 6)}
    assert {r["params"]["threadId"] for r in deletions} == created
    assert all(any(r.get("method") == "thread/read" and r["params"].get("threadId") == identity and r["params"].get("includeTurns") is True for r in requests) for identity in created if identity != "native-supervisor-5")
    if not restoring:assert any(r.get("method") == "turn/interrupt" and r["params"].get("threadId") == "native-supervisor-5" for r in requests)
    for name in ["backend.pid", "opencode.pid"]:
        path = root / name
        if path.exists():
            assert not Path("/proc", path.read_text().strip()).exists(), "Owned provider survived"
    print(json.dumps({"ok": True, "checks": result, "restored":restoring,"sourceTurns": len(sources), "utilityEvaluations": len(utilities), "utilityCleanup": len(deletions), "routerDecisions": 2, "privateProvidersOnly": True, "memory": measured, "artifacts": str(root)}))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("native")
    parser.add_argument("gateway")
    parser.add_argument("--private-root")
    parser.add_argument("--restore",action="store_true")
    args = parser.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    native, gateway = str(Path(args.native).resolve()), str(Path(args.gateway).resolve())
    if args.private_root:
        return gui(native, gateway, Path(args.private_root))
    base = Path(__file__).resolve().parents[1] / "target/gnome-native-smoke"
    base.mkdir(parents=True, exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix="supervision-ui-", dir=base))
    runtime.create_profile(root)
    with sqlite3.connect(root / "config/codex-thread-studio/studio.sqlite3") as database:
        database.execute("INSERT INTO session_turn_options VALUES('codex:router','controller-model','high','default')")
        if args.restore:database.execute("UPDATE session_turn_options SET service_tier='fast' WHERE session_key='codex:target'")
    if args.restore:
        runtime.fixture.BACKEND=source();runtime.fixture.OPENCODE_BACKEND=runtime.OPENCODE
        seed=runtime.fixture.Gateway(Path(gateway),root,'opencode')
        try:
            identity=runtime.submit(seed,runtime.body(seed,'Restored routed supervision A and B'))
            runtime.until(lambda:runtime.get(seed,identity)['status']=='running')
        finally:seed.close()
        (root/'restore-mode').touch()
    (root / "config/codex-thread-studio/gnome.json").write_text(json.dumps({"selectedBackend": "codex", "selectedSession": "target"}))
    for name, code in [("fake-codex.py", source()), ("fake-opencode.py", runtime.OPENCODE)]:
        path = root / name
        path.write_text(code)
        path.chmod(0o700)
    env = dict(os.environ, CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway, CODEX_THREAD_STUDIO_CODEX_BIN=str(root / "fake-codex.py"), CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root / "fake-opencode.py"), CODEX_HOME=str(root / "codex"), XDG_CONFIG_HOME=str(root / "config"), XDG_DATA_HOME=str(root / "data"), XDG_CACHE_HOME=str(root / "cache"), READER_CACHE_DIR=str(root / "reader-cache"), GDK_BACKEND="wayland", GIO_USE_VFS="local", GIO_USE_VOLUME_MONITOR="unix", STUDIO_GNOME_SMOKE_DIR=str(root), STUDIO_GNOME_SMOKE_SUPERVISION="restore" if args.restore else "1")
    with isolated_environment(env, headless=True) as private:
        subprocess.run(["dbus-run-session", "--", shutil.which("mutter") or "mutter", "--headless", "--wayland", "--no-x11", "--wayland-display", "studio-test", "--virtual-monitor", "1600x1000", "--", sys.executable, str(Path(__file__).resolve()), native, gateway, "--private-root", str(root)], cwd=root, env=private, check=True, timeout=200)


if __name__ == "__main__":
    main()
