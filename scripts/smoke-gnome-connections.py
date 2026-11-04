#!/usr/bin/env python3
"""Native connection diagnostics, partial health failure, stale reads and gateway recovery on owned Wayland."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import resource
import shutil
import subprocess
import sys
import tempfile
import time

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).with_name('test-support')))
from memory_sampler import ProcessMemory
from private_wayland import isolated_environment, OwnedMutterInput
from private_opencode_events import SOURCE as OPENCODE_EVENTS


def module(name, filename):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(filename))
    loaded = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(loaded)
    return loaded


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('native')
    parser.add_argument('gateway')
    parser.add_argument('--private-root')
    parser.add_argument('--backend',choices=['codex','opencode'],default='codex')
    args = parser.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    native, gateway = (str(Path(value).resolve()) for value in [args.native, args.gateway])
    runtime = module('navigation_runtime', 'smoke-gnome-router-runtime.py')
    if args.private_root:
        root = Path(args.private_root)
        originals = {str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest()
                     for name in ['project','project-oc','shared'] for p in (root/name).rglob('*') if p.is_file()}
        owned=OwnedMutterInput()
        owned.enable_clipboard_reader()
        with (root/'native.log').open('w') as log:
            app = subprocess.Popen([native], cwd=root, stdout=log, stderr=log)
            memory = ProcessMemory(app.pid, gateway, include_web_content=True)
            try:
                deadline=time.monotonic()+150
                while app.poll() is None:
                    owned.respond(root)
                    if time.monotonic()>deadline:raise TimeoutError(('Navigation timed out',root))
                    time.sleep(.01)
            finally:
                if app.poll() is None:
                    app.terminate()
                    app.wait(timeout=10)
                measured = memory.close()
                owned.close()
        result = json.loads((root/'connections-result.json').read_text())
        assert app.returncode == 0 and result['ok'], (root, result)
        assert not any(word in (root/'native.log').read_text() for word in ['CRITICAL', 'panicked', 'Gtk-WARNING'])
        requests=[json.loads(line) for line in (root/'requests.jsonl').read_text().splitlines()]
        assert not any(r.get('method') in ['turn/start','turn/interrupt','thread/start','review/start','thread/shellCommand'] for r in requests), 'Diagnostics sent a provider mutation'
        requests=[json.loads(line) for line in (root/'opencode-requests.jsonl').read_text().splitlines()]
        assert not any(r['method']!='GET' for r in requests), 'Diagnostics wrote to OpenCode'
        assert not (root/'must-not-run').exists()
        launches=[json.loads(line) for line in (root/'diagnostics-launches.jsonl').read_text().splitlines()]
        assert len(launches)==2, 'Gateway restart count changed'
        pids=[int(line) for line in (root/'diagnostics-provider-pids.jsonl').read_text().splitlines()]
        pids.extend(value[key] for value in launches for key in ['wrapper','gateway'])
        assert pids and all(not Path('/proc',str(pid)).exists() for pid in pids), ('Owned process survived',pids)
        assert originals == {str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest()
                             for name in ['project','project-oc','shared'] for p in (root/name).rglob('*') if p.is_file()}, 'Viewer tests changed source files'
        for name in ['backend.pid', 'opencode.pid']:
            if (root/name).exists():
                assert not Path('/proc', (root/name).read_text().strip()).exists(), 'Owned provider survived'
        deadline = time.monotonic()+10
        while memory.web_content_alive() and time.monotonic() < deadline:
            time.sleep(.1)
        assert not memory.web_content_alive(), 'Owned EPUB process survived'
        print(json.dumps(dict(ok=True, checks=result, memory=measured, sourceFilesUnchanged=True, ownedCleanup=True, artifacts=str(root))))
        return
    base = Path(__file__).resolve().parents[1]/'target/gnome-native-smoke'
    base.mkdir(parents=True, exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix='connections-', dir=base))
    runtime.create_profile(root)
    project = root/'project'
    turns=[];oc=[]
    for i in range(150):
        question='Original opening question 中文😀' if i==0 else f'Later question {i}'
        turns.append(dict(id=f'initial-turn-{i}',status='completed',items=[dict(id=f'initial-user-{i}',type='userMessage',content=[dict(type='text',text=question)]),dict(id=f'initial-answer-{i}',type='agentMessage',text=f'Reply {i}')]))
        user=f'oc-initial-user-{i}'
        oc.extend([dict(info=dict(id=user,role='user',sessionID='oc-target'),parts=[dict(id=user+'-part',type='text',text=question)]),dict(info=dict(id=user+'-assistant',role='assistant',parentID=user,sessionID='oc-target',time=dict(completed=123)),parts=[dict(id=user+'-answer-part',type='text',text=f'Reply {i}')])])
    (root/'codex-turns.json').write_text(json.dumps({'target':turns}))
    (root/'oc-turns.json').write_text(json.dumps({'oc-target':oc}))
    code=runtime.CODEX.replace("    elif method=='thread/read':", "    elif method=='model/list':answer(identity,{'data':[{'id':'target-model','model':'target-model','displayName':'Native target model','supportedReasoningEfforts':[{'reasoningEffort':'low','description':'Low'}],'defaultReasoningEffort':'low','serviceTiers':[{'id':'fast'}]}]})\n    elif method=='thread/items/list':answer(identity,{'data':list(reversed([{'turnId':t['id'],'item':i} for t in turns.get(session,[]) for i in t['items']]))[:120],'nextCursor':None})\n    elif method=='thread/read':")
    code=code.replace("summary('target'),summary('hidden')","summary('target'),summary('other'),summary('hidden')")
    code=code.replace("value=summary(session)","value={**summary(session),'cliVersion':'native-fixture-cli','source':'<b>literal source</b>','sessionId':'session-tree','forkedFromId':'fork-origin'}")
    begin=code.index("    elif method=='thread/turns/list':")
    end=code.index("    elif method=='turn/interrupt':",begin)
    code=code[:begin]+"""    elif method=='thread/turns/list':
        history=list(turns.get(session,[]))
        direction=params.get('sortDirection','desc')
        if direction=='desc':history.reverse()
        cursor=int(params.get('cursor','0'))
        page=history[cursor:cursor+4]
        answer(identity,{'data':page,'nextCursor':str(cursor+4) if cursor+4<len(history) else None})
"""+code[end:]
    code=code.replace('Native routed target response 中文😀','Own native session metadata and routing summaries 中文😀')
    opencode=runtime.OPENCODE.replace("'permission':[{'permission':'*','pattern':'*','action':'deny'}]", "'version':'native-fixture-cli','permission':[{'permission':'*','pattern':'*','action':'deny'}]")
    opencode=opencode.replace("session('oc-router'),session('oc-target')","session('oc-router'),session('oc-target'),session('oc-other')")
    opencode=opencode.replace('class Handler(BaseHTTPRequestHandler):', OPENCODE_EVENTS+'\nclass Handler(BaseHTTPRequestHandler):')
    opencode=opencode.replace("        identity=path.split('/')[2]", """        if path=='/config/providers':return self.response({'providers':[{'id':'provider','models':{'oc-target-model':{}}}]})
        if path=='/global/event':return stream_sdk(self)
        identity=path.split('/')[2]""")
    opencode=opencode.replace('Native routed OpenCode target response 中文😀','Own native session metadata and routing summaries 中文😀')
    opencode=opencode.replace("                history.append({'info':info,'parts':parts});ledger.write_text(json.dumps(messages))", "                if (root/'oc-active').exists():info['time']={}\n                history.append({'info':info,'parts':parts});ledger.write_text(json.dumps(messages))")
    opencode=opencode.replace("            return self.response(status=204)", """            for message in history[-2:]:emit_sdk({'directory':session(identity)['directory'],'payload':{'type':'message.updated','properties':{'info':message['info']}}})
            emit_sdk({'directory':session(identity)['directory'],'payload':{'type':'session.status','properties':{'sessionID':identity,'status':{'type':'busy' if (root/'oc-active').exists() else 'idle'}}}})
            return self.response(status=204)""")
    opencode=opencode.replace("            (root/'release-oc-target').touch();return self.response(True)", """            (root/'release-oc-target').touch()
            with lock:
                for message in messages.get(identity,[]):
                    if message['info']['role']=='assistant' and not message['info'].get('time',{}).get('completed'):
                        message['info']['time']={'completed':123};message['info']['error']={'name':'MessageAbortedError'}
                ledger.write_text(json.dumps(messages))
            emit_sdk({'directory':session(identity)['directory'],'payload':{'type':'session.status','properties':{'sessionID':identity,'status':{'type':'idle'}}}})
            return self.response(True)""")
    opencode=opencode.replace('def response(self,value=None,status=200):','def response(self,value=None,status=200,headers=None):')
    opencode=opencode.replace("body=json.dumps(value).encode() if status!=204 else b'';self.send_response(status);self.send_header('Content-Type','application/json');self.send_header('Content-Length',str(len(body)));self.end_headers();self.wfile.write(body)","body=json.dumps(value).encode() if status!=204 else b''\n        self.send_response(status);self.send_header('Content-Type','application/json');self.send_header('Content-Length',str(len(body)))\n        for name,value in (headers or {}).items():self.send_header(name,value)\n        self.end_headers();self.wfile.write(body)")
    opencode=opencode.replace("return self.response(messages.get(identity,[])[-limit:])", "history=messages.get(identity,[])\n            before=query.get('before',[None])[0]\n            end=next((i for i,m in enumerate(history) if m['info']['id']==before),len(history))\n            start=max(0,end-limit)\n            return self.response(history[start:end],headers={'X-Next-Cursor':history[start]['info']['id']} if start else {})")
    opencode=opencode.replace("        if path=='/global/health':return self.response({'healthy':True,'version':'native-runtime'})", """        if path=='/global/health':
            if (root/'diagnostics-unavailable').exists():return self.response({'error':'Private unavailable health'},503)
            if (root/'diagnostics-hold').exists():
                (root/'diagnostics-health-waiting').touch()
                end=time.monotonic()+10
                while (root/'diagnostics-hold').exists() and time.monotonic()<end:time.sleep(.02)
            return self.response({'healthy':True,'version':'native-runtime'})""")
    for name,body in [('fake-codex.py',code),('fake-opencode.py',opencode)]:
        body=body.replace('root=Path(__file__).parent', "root=Path(__file__).parent\nwith (root/'diagnostics-provider-pids.jsonl').open('a') as log:log.write(str(os.getpid())+'\\n')")
        import ast
        ast.parse(body,filename=name)
        path=root/name;path.write_text(body);path.chmod(0o700)
    config=root/'config/codex-thread-studio/backends.json'
    canary=root/'must-not-run-provider.py'
    canary.write_text("#!/usr/bin/env python3\nfrom pathlib import Path\nPath(__file__).with_name('must-not-run').touch()\nraise SystemExit(23)\n")
    canary.chmod(0o700)
    config.write_text(json.dumps({'version':1,'backends':[{'id':'extra','name':'<Native extra> 中文','tag':'NX','adapter':'codex-app-server','command':str(canary),'args':['--literal','<metadata>']}]}))
    actual=gateway
    wrapper=root/'owned-gateway.py'
    wrapper.write_text("#!/usr/bin/env python3\nimport sys\nfrom pathlib import Path\nsys.path.insert(0,"+repr(str(Path(__file__).resolve().parent/'test-support'))+")\nfrom private_gateway_control import main\nraise SystemExit(main("+repr(actual)+",sys.argv[1:]))\n")
    wrapper.chmod(0o700)
    gateway_launcher=str(wrapper)
    (root/'config/codex-thread-studio/gnome.json').write_text(json.dumps(dict(selectedBackend=args.backend, selectedSession='oc-target' if args.backend=='opencode' else 'target')))
    preferences = root/'config/codex-thread-studio/settings.json'
    raw = json.loads(preferences.read_text())
    raw['language'] = 'en'
    preferences.write_text(json.dumps(raw))
    env = dict(os.environ, CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway_launcher,
               CODEX_THREAD_STUDIO_BACKENDS_CONFIG=str(root/'config/codex-thread-studio/backends.json'),
               CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),
               CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'), CODEX_HOME=str(root/'codex'),
               XDG_CONFIG_HOME=str(root/'config'), XDG_DATA_HOME=str(root/'data'), XDG_CACHE_HOME=str(root/'cache'),
               READER_CACHE_DIR=str(root/'reader-cache'), GDK_BACKEND='wayland', GIO_USE_VFS='local',
               GIO_USE_VOLUME_MONITOR='unix', STUDIO_GNOME_SMOKE_DIR=str(root),
               STUDIO_GNOME_SMOKE_CONNECTIONS='1')
    env['STUDIO_OWNED_MUTTER_INPUT']='1'
    with isolated_environment(env, headless=True) as private:
        subprocess.run(['dbus-run-session', '--', shutil.which('mutter') or 'mutter', '--headless', '--wayland',
                        '--no-x11', '--wayland-display', 'studio-test', '--virtual-monitor', '1600x1000', '--',
                        sys.executable, str(Path(__file__).resolve()), native, gateway, '--private-root', str(root), '--backend', args.backend],
                       cwd=root, env=private, check=True, timeout=170)


if __name__ == '__main__':
    main()
