#!/usr/bin/env python3
"""Read large real document/chat/tool snapshots through GTK and private providers.

No model turns or production credentials. All exports stay in the owned fixture.
"""
import argparse
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
sys.dont_write_bytecode=True
spec=importlib.util.spec_from_file_location('output_history',Path(__file__).with_name('smoke-gnome-conversation-pages.py'))
history=importlib.util.module_from_spec(spec);spec.loader.exec_module(history);runtime=history.runtime

HELPERS=r'''
SOURCES=json.loads((root/'raw-items.json').read_text())
def output_turn(identity):return {'id':'output-turn','status':'completed','items':SOURCES if identity=='target' else []}
'''
READS=r'''    elif method in ('thread/read','thread/resume'):
        value={**summary(session),'turns':[],'historyMode':'paginated'}
        if params.get('includeTurns') is True:value['turns']=[output_turn(session)]
        answer(identity,{'thread':value})
'''
PAGES=r'''    elif method=='thread/items/list':
        assert params['limit']==120 and params['sortDirection']=='desc'
        result={'data':[{'turnId':'output-turn','item':item} for item in reversed(SOURCES if session=='target' else [])],'nextCursor':None}
        with (root/'source-payload-bytes.jsonl').open('a') as f:f.write(str(len(json.dumps(result,ensure_ascii=False,separators=(',',':')).encode()))+'\n')
        answer(identity,result)
    elif method=='thread/turns/list':
        assert params['limit']==4 and params['sortDirection']=='desc' and params['itemsView']=='summary'
        answer(identity,{'data':[{'id':'output-turn','status':'completed','items':[]}],'nextCursor':None})
'''
OC_PAGES=r'''        if path.endswith('/message'):
            assert query.get('directory')==[str(root/'project-oc')],query
            body=(root/'expected-assistant.txt').read_bytes().decode()
            user=(root/'expected-user.txt').read_bytes().decode()
            return self.response([{'info':{'id':'oc-user','role':'user','sessionID':identity},'parts':[{'id':'oc-user-part','type':'text','text':user}]},{'info':{'id':'oc-large-message','role':'assistant','parentID':'oc-user','sessionID':identity,'time':{'completed':1}},'parts':[{'id':'oc-large','type':'text','text':body}]}])
'''
OC_NAVIGATION_EVENTS=r'''        if path=='/global/event':
            self.send_response(200);self.send_header('Content-Type','text/event-stream');self.end_headers()
            try:
                while True:
                    path=root/'output-navigation-events.json'
                    if path.exists():
                        events=json.loads(path.read_text());path.unlink()
                        for event in events:
                            self.wfile.write(('data: '+json.dumps({'directory':str(root/'project-oc'),'payload':event})+'\n\n').encode())
                    self.wfile.write(b'data: {"type":"server.heartbeat"}\n\n');self.wfile.flush();time.sleep(.1)
            except (BrokenPipeError,ConnectionResetError):return
        if path=='/config/providers':return self.response({'providers':[],'default':{}})
'''
def providers():
    codex=runtime.CODEX.replace('for raw in sys.stdin:',HELPERS+'\nfor raw in sys.stdin:',1)
    codex=history.replace_region(codex,"    elif method=='thread/read':","    elif method=='fuzzyFileSearch':",READS)
    codex=history.replace_region(codex,"    elif method=='thread/turns/list':","    elif method=='turn/interrupt':",PAGES)
    codex=codex.replace("    elif method=='thread/list':","    elif method=='model/list':answer(identity,{'data':[]})\n    elif method=='thread/list':",1)
    codex=codex.replace("if 'id' not in request:continue","if 'id' not in request or 'method' not in request:continue",1)
    opencode=history.replace_region(history.providers()[1],"        if path.endswith('/message'):","        if path.endswith('/prompt_async'):",OC_PAGES)
    assert opencode.count(history.OPENCODE_EVENTS)==1
    opencode=opencode.replace(history.OPENCODE_EVENTS,OC_NAVIGATION_EVENTS,1)
    return codex,opencode

def sources(root,document):
    report=Path(document).read_bytes()
    assistant=report.decode()+ '\n\noutput-tail-marker 中文😀 \nsecond output-tail-marker 中文😀 \n'
    command='command-output-字😀'*40_000+'\r\ncommand end\rprogress\u2028line\u2029end\0\rlast\n'
    reasoning='Reasoning 中文😀\r\n'*20_000
    tool='tool output 中文😀\n'*18_000
    files='- old 中文\n+ new 😀\n'*15_000
    user='User control \0\n'+'Long original question 中文😀\n'*15_000
    items=[{'id':'large-assistant','type':'agentMessage','text':assistant},
        {'id':'large-command','type':'commandExecution','command':'printf private-fixture','aggregatedOutput':command,'status':'completed','source':'agent'},
        {'id':'large-reasoning','type':'reasoning','summary':[{'text':reasoning}]},
        {'id':'large-tool','type':'mcpToolCall','server':'fixture-server','tool':'fixture-tool','result':tool},
        {'id':'large-files','type':'fileChange','changes':[{'path':'src/example.rs','diff':files}]},
        {'id':'large-user','type':'userMessage','content':[{'type':'text','text':user}]}]
    expected={'assistant':assistant,'command':'$ printf private-fixture\n'+command,'reasoning':reasoning,'tool':'fixture-server / fixture-tool\n'+json.dumps(tool,ensure_ascii=False),'files':'src/example.rs\n'+files,'user':user}
    for case,text in expected.items():(root/f'expected-{case}.txt').write_bytes(text.encode())
    (root/'raw-items.json').write_text(json.dumps(items,ensure_ascii=False))
    return {'realDocumentBytes':len(report),'snapshotBytes':{k:len(v.encode()) for k,v in expected.items()}}

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('native');parser.add_argument('gateway');parser.add_argument('document');parser.add_argument('--private-root');parser.add_argument('--headless',action='store_true');args=parser.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE,(0,0))
    native,gateway=map(lambda p:str(Path(p).resolve()),[args.native,args.gateway])
    if args.private_root:return run(native,gateway,Path(args.private_root),dict(os.environ))
    base=Path(__file__).resolve().parents[1]/'target/gnome-native-smoke';base.mkdir(parents=True,exist_ok=True)
    root=Path(tempfile.mkdtemp(prefix='outputs-ui-',dir=base));runtime.create_profile(root)
    manifest=sources(root,args.document);(root/'manifest.json').write_text(json.dumps(manifest))
    (root/'config/codex-thread-studio/gnome.json').write_text(json.dumps({'selectedBackend':'codex','selectedSession':'target'}))
    for name,source in zip(['fake-codex.py','fake-opencode.py'],providers()):
        compile(source,name,'exec');path=root/name;path.write_text(source);path.chmod(0o700)
    env=dict(os.environ,CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway,CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),CODEX_HOME=str(root/'codex'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),READER_CACHE_DIR=str(root/'reader-cache'),GDK_BACKEND='wayland',STUDIO_GNOME_SMOKE_DIR=str(root),STUDIO_GNOME_SMOKE_OUTPUT='1')
    compositor=[]
    if args.headless:
        mutter=shutil.which('mutter')
        if not mutter:raise RuntimeError('Mutter is needed for --headless testing')
        compositor=[mutter,'--headless','--wayland','--no-x11','--wayland-display','studio-test','--virtual-monitor','1600x1000','--']
        env['STUDIO_OWNED_MUTTER_INPUT']='1'
    with history.isolated_environment(env,headless=args.headless) as private:
        subprocess.run(['dbus-run-session','--',*compositor,sys.executable,str(Path(__file__).resolve()),native,gateway,str(Path(args.document).resolve()),'--private-root',str(root)],env=private,cwd=root,check=True,timeout=180)

def run(native,gateway,root,env):
    with (root/'app.log').open('w') as log:
        child=subprocess.Popen([native],cwd=root,env=env,stdout=log,stderr=log);memory=history.ProcessMemory(child.pid,gateway)
        try:
            if env.get('STUDIO_OWNED_MUTTER_INPUT')=='1':
                from private_wayland import OwnedMutterInput
                controlled=OwnedMutterInput()
                try:
                    time.sleep(1)
                    controlled.focus_only_window()
                    deadline=time.monotonic()+160
                    while child.poll() is None:
                        if time.monotonic()>deadline:raise TimeoutError('Native output fixture exceeded its deadline')
                        activate=root/'output-key-activate'
                        if activate.exists():
                            activate.unlink();controlled.activate_focused_control()
                        time.sleep(.01)
                    code=child.returncode
                finally:controlled.close()
            else:code=child.wait(timeout=160)
        except BaseException:child.terminate();child.wait(timeout=10);raise
        finally:measure=memory.close()
    result=json.loads((root/'outputs.json').read_text()) if (root/'outputs.json').exists() else {'ok':False,'error':'No native output acceptance result'}
    assert code==0 and result['ok'],(root,code,result,(root/'app.log').read_text()[-2500:])
    assert result['searchNavigationPriority'] and result['liveUpdatesRetainSearch'] and result['followTailResumes'],result
    manifest=json.loads((root/'manifest.json').read_text())
    for case in ['assistant','command','reasoning','tool','files','user','historical','opencode','opencode-user']:
        expected='assistant' if case in ['historical','opencode'] else 'user' if case=='opencode-user' else case
        assert (root/f'snapshot-{case}.txt').read_bytes()==(root/f'expected-{expected}.txt').read_bytes(),f'Incomplete or transformed {case} export'
    requests=runtime.requests(root);assert not runtime.starts(root),'Output reading started a model turn'
    opencode_requests=[json.loads(line) for line in (root/'opencode-requests.jsonl').read_text().splitlines()]
    assert all(request['method']=='GET' for request in opencode_requests),'Output navigation issued a provider write'
    pages=[v for v in requests if v.get('method')=='thread/items/list']
    assert len(pages)==2,'Opening cached full outputs made extra provider reads'
    for name in ['backend.pid','opencode.pid']:
        if not (root/name).exists():continue
        pid=(root/name).read_text().strip();assert not Path('/proc',pid).exists(),f'Owned provider survived: {pid}'
    text=(root/'app.log').read_text();assert not any(s in text for s in ['CRITICAL','panicked','Theme parser error']),(root,text[-2500:])
    sizes=[int(s) for s in (root/'source-payload-bytes.jsonl').read_text().splitlines()]
    print(json.dumps(dict(result,**manifest,memory=measure,providerPageBytes={'largest':max(sizes),'total':sum(sizes),'responses':len(sizes)},noExtraProviderReads=True,exactExports=True,backendCleanup=True,artifacts=str(root))))

if __name__=='__main__':main()
