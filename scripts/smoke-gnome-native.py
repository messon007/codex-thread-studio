#!/usr/bin/env python3
"""Exercise the actual GTK4 UI and shared gateway with an isolated fake Codex.

Python is test tooling only. The installed native app and gateway are Rust.
Pass the debug native executable and gateway-only executable as arguments.
Runs on an owned headless Wayland desktop; no host windows or session bus are used.
The optional report needs a multi-page first table within the diagnostic
projection budgets (64 fragments, 2 MiB source and 512 KiB projected text). Installed large-reader coverage tests larger files separately.
"""
import json
import os
import shutil
import sqlite3
from pathlib import Path
import subprocess
import sys
import tempfile
import threading
import time

FAKE_CODEX = r'''#!/usr/bin/env python3
import json,sys,os,threading,time
from pathlib import Path
Path(__file__).with_name('backend.pid').write_text(str(os.getpid()))
EXTRA=[]
LAST_INPUT=''
TIER='fast'
TITLES={'native-smoke-thread':'Native GTK4 acceptance','native-cache-thread':'Native cached sibling'}
ARCHIVED=set()
DELETED=set()
AGENTS={
 'native-agent-before':('native-smoke-thread',1,'First worker','worker'),
 'native-agent-child':('native-smoke-thread',2,'Worker 中文','worker'),
 'native-agent-grandchild':('native-agent-child',3,'Explorer 😀','explorer'),
 'native-agent-unrelated':('native-cache-thread',0,'Other family','worker'),
 'native-agent-orphan':('missing-parent',0,'Missing parent','worker'),
 'native-agent-cycle-a':('native-agent-cycle-b',0,'Cycle A','worker'),
 'native-agent-cycle-b':('native-agent-cycle-a',0,'Cycle B','worker'),
}
def emit(value):
    print(json.dumps(value),flush=True)
def event(method,params):
    if method=='item/completed': EXTRA.append(params['item'])
    emit({"method":method,"params":{"threadId":"native-smoke-thread",**params}})
def thread(identity='native-smoke-thread'):
    if identity=='native-managed-router-id':
        return {'id':identity,'name':'Thread Router','cwd':str(Path(os.environ['XDG_DATA_HOME']) / 'codex-thread-studio/router'),'turns':[]}
    if identity in AGENTS:
        parent,created,nickname,role=AGENTS[identity]
        return {'id':identity,'parentThreadId':parent,'createdAt':created,'name':None,'agentNickname':nickname,'agentRole':role,'cwd':str(Path(__file__).with_name('project')),'status':{'type':'idle'},'turns':[{'id':identity+'-history','status':'completed','items':[{'id':identity+'-reply','type':'agentMessage','text':'# Native agent session\n\n'+nickname}]}]}
    table = "\n\n| Item | Requirement | Context |\n|---|---|---|\n" + "".join(f"| item {row} | **REQ-{row:04}** | 中文😀 long context {row} {'details ' * 12} |\n" for row in range(80))
    diagrams = "\n\n```mermaid\nflowchart LR\nA[\"Native chat $$x^2$$\"] --> B[\"Reader $$y_1$$\"]\n```\n\n@startuml\nAlice -> Bob: Native chat\n@enduml\n\n```d2\nchat -> reader: Native chat\n```\n"
    diagrams += "\n\n```math\n" + r"x=\frac{-b\pm\sqrt{b^2-4ac}}{2a}" + "\n```\n"
    return {"id":identity,"name":TITLES[identity],"cwd":str(Path(__file__).with_name('project')),"turns":[{"id":"history-turn","status":"completed","items":[{"id":"history-user","type":"userMessage","content":[{"type":"text","text":"Check the native GNOME conversation view."}]},{"id":"history-message","type":"agentMessage","text":"# Native GNOME\n\nThis response uses **GTK4** and *libadwaita*.\n\n- No WebView UI\n- Shared Rust gateway\n- Unicode: 中文 · 日本語\n\n[Open the native reader](nested/note.md:1)\n\n```rust\nfn main() { println!(\"Hello, GNOME\"); }\n```"+table+diagrams}]+EXTRA}]}
def old_turn():
    return {'id':'native-older-turn','status':'completed','items':[{'id':'native-older-question','type':'userMessage','content':[{'type':'text','text':'An earlier question'}]},{'id':'native-old-response','type':'agentMessage','text':'# Earlier response\n\nnative-old-marker is available from paged history.\n\n[Open project document](nested/note.md:1)'}]}
def occurrences(identity,query):
    rows=[]
    for turn in thread(identity)['turns']+[old_turn()]:
        for item in turn['items']:
            text=item.get('text') or '\n'.join(part.get('text','') for part in item.get('content',[]))
            index=text.lower().find(query.lower())
            if index<0: continue
            start=max(0,index-82); end=min(len(text),index+len(query)+82); snippet=text[start:end]
            units=lambda value:len(value.encode('utf-16-le'))//2
            rows.append({'turnId':turn['id'],'itemId':item['id'],'type':'user' if item['type']=='userMessage' else 'assistant','snippet':snippet,'snippetMatchRange':{'start':units(text[start:index]),'end':units(text[start:index+len(query)])}})
    return {'data':rows[:100]}
for raw in sys.stdin:
    message=json.loads(raw)
    method=message.get('method','')
    with Path(__file__).with_name('requests.jsonl').open('a') as log:
        log.write(json.dumps(message)+'\n')
    if 'id' not in message: continue
    if message.get('id')==710 and 'result' in message:
        assert message['result']['decision']=='accept'
        event('turn/completed',{'turn':{'id':'native-background-turn','status':'completed'}})
        continue
    params=message.get('params',{})
    if (method=='thread/read' and params.get('includeTurns') is False) or (method=='thread/list' and params.get('sourceKinds')):
        if method=='thread/read':
            summary=thread(params['threadId']); summary['turns']=[]
            if Path(__file__).with_name('native-agent-lineage-cycle.flag').exists(): summary['parentThreadId']=summary['id']
            reply={'thread':summary}
        else:
            assert params['sourceKinds']==['subAgent','subAgentReview','subAgentCompact','subAgentThreadSpawn','subAgentOther']
            assert params['limit']==100 and params['useStateDbOnly'] is True and 'ancestorThreadId' not in params
            cursor=params.get('cursor')
            identities=['native-agent-grandchild','native-agent-unrelated','native-agent-orphan','native-agent-cycle-a','native-agent-cycle-b'] if not cursor else ['native-agent-before','native-agent-child']
            summaries=[thread(identity) for identity in identities]
            for summary in summaries: summary['turns']=[]
            cycle=Path(__file__).with_name('native-agent-cursor-cycle.flag').exists()
            reply={'data':summaries,'nextCursor':'agent-cycle' if cycle else 'agents:中文/+?' if not cursor else None}
        flag=Path(__file__).with_name('native-agent-delay.flag'); late=flag.exists()
        if late: flag.unlink()
        def agent_reply(identity=message['id'],reply=reply,late=late):
            if late: time.sleep(0.4)
            emit({'id':identity,'result':reply})
        threading.Thread(target=agent_reply,daemon=True).start()
        continue
    if method=='thread/items/list':
        # This fixture models the older turn-page protocol. Unknown methods
        # must be explicit errors, not malformed successful empty objects.
        emit({'id':message['id'],'error':{'code':-32601,'message':'legacy item endpoint'}})
        continue
    if method=='mcpServerStatus/list':
        assert params['threadId']=='native-smoke-thread' and params['limit']==100
        delay=Path(__file__).with_name('native-mcp-delay.flag')
        late=delay.exists()
        if late: delay.unlink()
        cycle=Path(__file__).with_name('native-mcp-cycle.flag').exists()
        cursor=params.get('cursor')
        data=[{'name':'Native MCP 中文' if not cursor else 'Native MCP second','authStatus':'oAuth' if not cursor else 'notLoggedIn','runtimeStatus':'connected' if not cursor else 'authenticationRequired','tools':{'test':{'name':'test','inputSchema':{'type':'object','privateFixture':'x'*100000}}},'resources':[{'name':'native','uri':'private://fixture'}],'resourceTemplates':[]}]
        result={'data':data,'nextCursor':'cycle' if cycle else 'mcp:中文/+?' if not cursor else None}
        def mcp_reply(identity=message['id'],reply=result,late=late):
            if late: time.sleep(0.4)
            emit({'id':identity,'result':reply})
        threading.Thread(target=mcp_reply,daemon=True).start()
        continue
    if method=='turn/interrupt' and LAST_INPUT=='native-smoke-stale':
        emit({'id':message['id'],'error':{'code':-32000,'message':'no active turn to interrupt'}})
        continue
    if method=='thread/turns/list' and Path(__file__).with_name('native-legacy-read.flag').exists():
        emit({'id':message['id'],'error':{'code':-32601,'message':'legacy turn endpoint'}})
        continue
    if method=='thread/searchOccurrences' and Path(__file__).with_name('native-no-search.flag').exists():
        emit({'id':message['id'],'error':{'code':-32601,'message':'legacy search endpoint'}})
        continue
    if method=='thread/shellCommand':
        if params['command']=='native-shell-fail':
            emit({'id':message['id'],'error':{'message':'Private shell fixture rejected the command'}})
            continue
        emit({'id':message['id'],'result':{}})
        def shell_events(identity=message['id'],command=params['command']):
            turn='native-shell-'+str(identity)
            event('thread/status/changed',{'status':{'type':'active'}})
            event('turn/started',{'turn':{'id':turn,'status':'inProgress'}})
            event('item/started',{'turnId':turn,'item':{'id':'shell-'+str(identity),'type':'commandExecution','source':'userShell','command':command,'status':'inProgress','aggregatedOutput':''}})
            if command=='native-shell-hold':
                flag=Path(__file__).with_name('release-native-shell.flag')
                while not flag.exists(): time.sleep(0.02)
            event('item/completed',{'turnId':turn,'item':{'id':'shell-'+str(identity),'type':'commandExecution','source':'userShell','command':command,'status':'completed','aggregatedOutput':'Native shell stdout 中文😀\n','exitCode':0}})
            event('thread/status/changed',{'status':{'type':'idle'}})
            event('turn/completed',{'turn':{'id':turn,'status':'completed'}})
        threading.Thread(target=shell_events,daemon=True).start()
        continue
    if method=='thread/compact/start':
        emit({'id':message['id'],'result':{}})
        continue
    if method=='review/start':
        turn='native-review-'+str(message['id'])
        event('turn/started',{'turn':{'id':turn,'status':'inProgress'}})
        event('turn/diff/updated',{'turnId':turn,'diff':'--- a/native.txt\n+++ b/native.txt\n'+''.join(f'+native-turn-diff-中文😀 {line}\n' for line in range(300))})
        event('thread/tokenUsage/updated',{'tokenUsage':{'total':{'totalTokens':4242},'modelContextWindow':256000}})
        event('turn/diff/updated',{'threadId':'native-cache-thread','turnId':'sibling-turn','diff':'+wrong-session'})
        event('thread/tokenUsage/updated',{'threadId':'native-cache-thread','tokenUsage':{'total':{'totalTokens':9999}}})
        event('item/completed',{'turnId':turn,'item':{'id':turn+'-reply','type':'agentMessage','text':'Native review result'}})
        event('turn/completed',{'turn':{'id':turn,'status':'completed'}})
        time.sleep(0.3)
        emit({'id':message['id'],'result':{'turn':{'id':turn,'status':'inProgress'}}})
        continue
    if method=='turn/start' and params.get('input',[{}])[0].get('text','').startswith('native-command-policy'):
        turn='native-command-policy-'+str(message['id'])
        items=[{'id':turn+'-user','type':'userMessage','content':params['input']},{'id':turn+'-reply','type':'agentMessage','text':'Native message permission test reply.'}]
        EXTRA.extend(items)
        emit({'id':message['id'],'result':{'turn':{'id':turn,'status':'completed','items':items}}})
        continue
    if method=='turn/start' and params.get('input',[{}])[0].get('text')=='native-ref-active':
        LAST_INPUT='native-ref-active'
        emit({'id':message['id'],'result':{'turn':{'id':'native-ref-active','status':'inProgress'}}})
        event('turn/started',{'turn':{'id':'native-ref-active','status':'inProgress'}})
        continue
    if method=='turn/start' and any(part.get('type') in ('skill','mention') for part in params.get('input',[])):
        text=next((part.get('text','') for part in params['input'] if part.get('type')=='text'),'')
        if 'native-ref-reject' in text:
            emit({'id':message['id'],'error':{'message':'Reference fixture rejected the request'}})
            continue
        def reference_reply(identity=message['id'],content=params['input'],late='native-ref-late' in text):
            if late: time.sleep(0.6)
            turn='native-reference-'+str(identity)
            emit({'id':identity,'result':{'turn':{'id':turn,'status':'completed','items':[{'id':'user-'+turn,'type':'userMessage','content':content},{'id':'answer-'+turn,'type':'agentMessage','text':'# Native reference completed\n\nWorker-projected command result.'}]}}})
        threading.Thread(target=reference_reply,daemon=True).start()
        continue
    if method=='turn/start' and any(part.get('type')=='image' for part in params.get('input',[])):
        text=next((part.get('text','') for part in params['input'] if part.get('type')=='text'),'')
        if text=='native-image-reject':
            emit({'id':message['id'],'error':{'message':'Image fixture rejected the request'}})
            continue
        turn='native-image-'+str(message['id'])
        if text=='native-image-late': time.sleep(0.8)
        emit({'id':message['id'],'result':{'turn':{'id':turn,'status':'inProgress'}}})
        event('turn/started',{'turn':{'id':turn,'status':'inProgress'}})
        event('item/completed',{'item':{'id':'user-'+turn,'type':'userMessage','content':params['input']}})
        event('turn/completed',{'turn':{'id':turn,'status':'completed'}})
        continue
    if method=='turn/start' and params.get('input',[{}])[0].get('text','').startswith('native-queue-'):
        value=params['input'][0]['text']; turn='turn-'+value
        LAST_INPUT=value
        if value=='native-queue-two':
            event('turn/started',{'turn':{'id':turn,'status':'inProgress'}})
            event('item/completed',{'item':{'id':'reply-'+value,'turnId':turn,'type':'agentMessage','text':'Queue second response.'}})
            event('turn/completed',{'turn':{'id':turn,'status':'completed'}})
            time.sleep(0.2)
            emit({'id':message['id'],'result':{'turn':{'id':turn,'status':'inProgress'}}})
            continue
        emit({'id':message['id'],'result':{'turn':{'id':turn,'status':'inProgress'}}})
        event('turn/started',{'turn':{'id':turn,'status':'inProgress'}})
        event('item/completed',{'item':{'id':'reply-'+value,'turnId':turn,'type':'agentMessage','text':'Queue response: '+value}})
        if value in ('native-queue-hold','native-queue-one','native-queue-final-hold'):
            if value!='native-queue-final-hold':
                def finish_queue(value=value,turn=turn):
                    flag=Path(__file__).with_name('release-'+value+'.flag')
                    while not flag.exists(): time.sleep(0.02)
                    event('turn/completed',{'turn':{'id':turn,'status':'completed'}})
                threading.Thread(target=finish_queue,daemon=True).start()
        else: event('turn/completed',{'turn':{'id':turn,'status':'completed'}})
        continue
    result={}
    if method=='initialize': result={'userAgent':'native-smoke','platformFamily':'linux'}
    elif method=='thread/list': result={'data':[thread(identity) for identity in TITLES if identity not in DELETED and (identity in ARCHIVED)==bool(params.get('archived'))],'nextCursor':None}
    elif method in ('thread/read','thread/resume','thread/start'): result={'thread':thread(params.get('threadId','native-smoke-thread')),'model':'native-smoke-model','serviceTier':TIER}
    elif method=='thread/searchOccurrences': result=occurrences(params['threadId'],params['searchTerm'])
    elif method=='thread/turns/list': result={'data':[old_turn()] if params.get('cursor')=='native-older-page' else thread(params['threadId'])['turns'],'nextCursor':None if params.get('cursor') else 'native-older-page'}
    elif method=='thread/name/set': TITLES[params['threadId']]=params['name']
    elif method=='thread/fork':
        TITLES['native-fork-thread']='Native private fork'
        result={'thread':thread('native-fork-thread')}
    elif method=='thread/archive': ARCHIVED.add(params['threadId'])
    elif method=='thread/unarchive': ARCHIVED.discard(params['threadId'])
    elif method=='thread/delete': DELETED.add(params['threadId'])
    elif method=='fuzzyFileSearch':
        result={'files':[{'path':'nested/说明 note.md'},{'file_name':'nested/note.md'}]}
        if params.get('query')=='stale':
            def late_file_reply(identity=message['id']):
                time.sleep(0.5)
                emit({'id':identity,'result':{'files':[{'path':'old-stale-result.md'}]}})
            threading.Thread(target=late_file_reply,daemon=True).start()
            continue
    elif method=='skills/list': result={'data':[{'skills':[{'name':'native-build','path':str(Path(__file__).with_name('project')/'skills'/'native-build'/'SKILL.md'),'description':'Native build skill 中文😀','enabled':True},{'name':'native-second','path':str(Path(__file__).with_name('project')/'skills'/'native-second'/'SKILL.md'),'description':'Second native skill','enabled':True},{'name':'native-disabled','path':'/not-enabled','enabled':False}]}]}
    elif method=='plugin/list': result={'marketplaces':[{'plugins':[{'id':'native-plugin@fixture','name':'native-plugin','installed':True,'enabled':True,'interface':{'displayName':'Native plugin','shortDescription':'Installed native plugin'}}]}]}
    elif method=='model/list': result={'data':[{'id':'native-smoke-model','model':'native-smoke-model','displayName':'Native smoke model','isDefault':True,'serviceTiers':[{'id':'fast'}],'supportedReasoningEfforts':[{'reasoningEffort':effort} for effort in ('low','medium','high')]}]}
    elif method=='turn/start':
        TIER=params.get('serviceTier',TIER)
        text=params.get('input',[{}])[0].get('text')
        result={'turn':{'id':'native-background-turn' if text=='native-smoke-background' else 'native-delayed-turn' if text=='native-smoke-delayed-send' else 'native-turn','status':'inProgress'}}
        if text=='native-smoke-delayed-send': time.sleep(0.4)
    elif method=='turn/steer': result={'turnId':'native-turn'}
    if method=='thread/read' and Path(__file__).with_name('native-legacy-read.flag').exists():
        result['thread']['turns'].append(old_turn())
    emit({'id':message['id'],'result':result})
    if method in ('turn/start','turn/steer'):
        text=params.get('input',[{}])[0].get('text','')
        LAST_INPUT=text
        if method=='turn/start': event('turn/started',{'turn':{'id':result['turn']['id'],'status':'inProgress'}})
        event('item/completed',{'item':{'id':'user-'+text,'type':'userMessage','content':[{'type':'text','text':text}]}})
        if text=='native-smoke-send':
            event('item/started',{'item':{'id':'native-smoke-reply','type':'agentMessage','text':''}})
            for delta in ['The **native** ','stream is responsive.','\n\n你好，GNOME。']:
                event('item/agentMessage/delta',{'itemId':'native-smoke-reply','delta':delta})
            event('item/completed',{'item':{'id':'native-smoke-reply','type':'agentMessage','text':'The **native** stream is responsive.\n\n你好，GNOME。'}})
            event('turn/completed',{'turn':{'id':'native-turn','status':'completed'}})
        elif text=='native-smoke-steer':
            event('item/completed',{'item':{'id':'native-smoke-steered','type':'agentMessage','text':'Steering accepted; ready to test Stop.'}})
        elif text=='native-smoke-background':
            def request_background():
                time.sleep(0.8)
                emit({'id':710,'method':'item/commandExecution/requestApproval','params':{'threadId':'native-smoke-thread','command':'echo native-background'}})
            threading.Thread(target=request_background,daemon=True).start()
        elif text=='native-smoke-delayed-send':
            event('turn/completed',{'turn':{'id':'native-delayed-turn','status':'completed'}})
    elif method=='turn/interrupt':
        event('turn/completed',{'turn':{'id':'turn-'+LAST_INPUT if LAST_INPUT.startswith('native-queue-') else 'native-ref-active' if LAST_INPUT=='native-ref-active' else 'native-turn','status':'interrupted'}})
'''

def main():
    if len(sys.argv) not in (3, 4):
        raise SystemExit('Usage: smoke-gnome-native.py NATIVE_DEBUG_BINARY GATEWAY_BINARY [MARKDOWN_FILE]')
    native, gateway = map(lambda p: str(Path(p).resolve()), sys.argv[1:3])
    root = Path(__file__).resolve().parent.parent
    output = root / 'target' / 'gnome-native-smoke'
    output.mkdir(parents=True, exist_ok=True)
    if os.environ.get('STUDIO_TEST_PRIVATE_NATIVE') != '1':
        import resource
        resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
        sys.path.insert(0, str(Path(__file__).with_name('test-support')))
        from private_wayland import isolated_environment
        owned = Path(tempfile.mkdtemp(prefix='native-desktop-', dir=output))
        env = dict(os.environ, STUDIO_TEST_PRIVATE_NATIVE='1', GDK_BACKEND='wayland',
                   XDG_CONFIG_HOME=str(owned/'config'), XDG_DATA_HOME=str(owned/'data'),
                   XDG_CACHE_HOME=str(owned/'cache'))
        arguments = [native, gateway, *([str(Path(sys.argv[3]).resolve())] if len(sys.argv)==4 else [])]
        with isolated_environment(env, headless=True) as private:
            subprocess.run(['dbus-run-session', '--', shutil.which('mutter') or 'mutter',
                            '--headless', '--wayland', '--no-x11', '--wayland-display', 'studio-test',
                            '--virtual-monitor', '1600x1000', '--', sys.executable,
                            str(Path(__file__).resolve()), *arguments], cwd=root, env=private,
                           check=True, timeout=380)
        return
    directory = Path(tempfile.mkdtemp(prefix='run-', dir=output))
    project = directory / 'project'
    project.mkdir()
    if len(sys.argv) == 4:
        shutil.copyfile(sys.argv[3], project / 'large.md')
    else:
        with (project / 'large.md').open('w') as stream:
            stream.write('# Native paged reader\n\n| Item | Requirement | Context |\n|---|---|---|\n')
            # This in-memory projection probe must fit 64 index fragments.
            # Large-file behavior is covered by the installed 32 MiB runner.
            for row in range(768):
                stream.write(f'| item {row} | REQ-{row:06} | 中文 context {"details " * 12} |\n')
    (project / 'edit.md').write_bytes('# Native edit\r\n\r\n中文😀 before\r\n'.encode())
    (project / 'diagrams.md').write_text('# Native diagrams\n\n```mermaid\nflowchart LR\nA[Reader 中文] --> B[Editor]\n```\n\n@startuml\nAlice -> Bob: Native 中文\n@enduml\n\n```d2\nreader -> editor: Native 中文\n```\n')
    (project/'math.md').write_text('# Native math\n\n$$\nx=\\frac{-b\\pm\\sqrt{b^2-4ac}}{2a}\n$$\n\n```math\n\\int_0^\\infty e^{-x^2}\\,dx=\\frac{\\sqrt{\\pi}}{2}\n```\n\n```latex\n\\begin{pmatrix}a&b\\\\c&d\\end{pmatrix}\n```\n',encoding='utf-8')
    (project/'math-invalid.md').write_text('# Invalid math\n\n```math\n\\unsupportedNativeMacro{x}\n```\n',encoding='utf-8')
    (project / 'code.rs').write_text(''.join(f'// line {line}: 中文😀\n' for line in range(1, 451)))
    (project / 'nested').mkdir()
    (project / 'nested' / 'note.md').write_text('# Nested document\n')
    (directory / 'outside.txt').write_text('Outside the chosen project.\n')
    (project / 'outside-link.txt').symlink_to(directory / 'outside.txt')
    repository = directory / 'git-project'
    repository.mkdir()
    git_environment = dict(os.environ, GIT_CONFIG_GLOBAL='/dev/null', GIT_CONFIG_SYSTEM='/dev/null')
    def git(*arguments):
        subprocess.run(['git', '-C', str(repository), *arguments], env=git_environment,
                       check=True, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    git('init', '--initial-branch=main', '--template=')
    git('config', 'user.name', 'Native acceptance')
    git('config', 'user.email', 'native-test@example.invalid')
    git('config', 'core.hooksPath', '/dev/null')
    git('config', 'commit.gpgsign', 'false')
    # Keep the two meaningful review commits newest, but cross a real history page.
    for checkpoint in range(101):
        git('commit', '--allow-empty', '-m', f'Older native checkpoint {checkpoint:03}')
    (repository / 'README.md').write_text('# Native Git fixture\n\nOriginal.\n')
    git('add', '--', 'README.md')
    git('commit', '-m', 'Add native fixture')
    (repository / 'code.rs').write_text('fn main() { println!("Native"); }\n')
    git('add', '--', 'code.rs')
    git('commit', '-m', 'Add code for history review')
    (repository / 'README.md').write_text('# Native Git fixture\n\nModified.\n')
    (repository / 'untracked.txt').write_text('New file.\n')
    fake = directory / 'fake-codex.py'
    fake.write_text(FAKE_CODEX)
    fake.chmod(0o700)
    fake_opencode = directory / 'fake-opencode.py'
    shutil.copyfile(root / 'scripts/test-support/gnome-opencode.py', fake_opencode)
    fake_opencode.chmod(0o700)
    profile_directory = directory / 'config/codex-thread-studio'
    profile_directory.mkdir(parents=True)
    (profile_directory / 'settings.json').write_text(json.dumps({'queueDepth': 3,'contentWidth':'wide','router':{'controllerBackend':'codex','controllers':{'codex':'native-managed-router-id','opencode':'native-managed-opencode-id'},'fallbacks':[]}}))
    with sqlite3.connect(profile_directory / 'studio.sqlite3') as database:
        database.execute('CREATE TABLE session_message_queues (session_key TEXT PRIMARY KEY, queue_json TEXT NOT NULL)')
        database.execute('INSERT INTO session_message_queues VALUES (?, ?)', ('opencode:native-oc-thread', json.dumps([{'id':'restored-native-message','text':'native-oc-restore-do-not-send','input':[],'createdAt':123}])))
        database.execute('CREATE TABLE session_opening_messages (session_key TEXT PRIMARY KEY,message_json TEXT NOT NULL)')
        database.execute('INSERT INTO session_opening_messages VALUES (?,?)',('codex:native-smoke-thread',json.dumps({'text':'Original opening for native Router','responsibility':'Original responsibility','source':'history','capturedAt':'native-capture-time','truncated':True})))
    (profile_directory / 'environments.json').write_text(json.dumps({'profiles': {str(project.resolve()): {
        'root': str(project.resolve()), 'variables': {'STUDIO_NATIVE_PROFILE_TEST': 'enabled'}, 'secrets': {'STUDIO_NATIVE_STORED_SECRET': 'native-original-secret'},
        'networkPolicy': 'restricted', 'allowedHosts': ['example.invalid'],
        'cacheVariables': {'XDG_CACHE_HOME': str(directory / 'backend-cache')},
    }}}))
    environment = dict(os.environ)
    environment.update({
        'CODEX_THREAD_STUDIO_GATEWAY_BIN': gateway,
        'CODEX_THREAD_STUDIO_CODEX_BIN': str(fake),
        'CODEX_THREAD_STUDIO_OPENCODE_BIN': str(fake_opencode),
        'CODEX_HOME': str(directory / 'codex'),
        'XDG_CONFIG_HOME': str(directory / 'config'),
        'XDG_DATA_HOME': str(directory / 'data'),
        'XDG_CACHE_HOME': str(directory / 'cache'),
        'READER_CACHE_DIR': str(directory / 'reader-cache'),
        'STUDIO_GNOME_SMOKE_DIR': str(directory),
        'GDK_BACKEND': 'wayland',
        'SHELL': '/bin/sh',
        'GIT_CONFIG_GLOBAL': '/dev/null',
        'GIT_CONFIG_SYSTEM': '/dev/null',
    })
    environment.pop('DISPLAY', None)
    environment.pop('ENV', None)
    sys.path.insert(0,str(Path(__file__).with_name('test-support')))
    from memory_sampler import ProcessMemory
    memory = {'nativeObservedPeakRssKiB': 0, 'nativeObservedPeakHwmKiB': 0,
              'gatewayObservedPeakRssKiB': 0, 'gatewayObservedPeakHwmKiB': 0, 'samples': 0}
    with (directory / 'app.log').open('w') as log:
        child = subprocess.Popen([native], env=environment, stdout=log, stderr=log)
        engine_observer = ProcessMemory(child.pid,gateway,include_web_content=True)
        finished = threading.Event()
        def observe_memory():
            gateway_pid = None
            def sample(pid, prefix):
                try:
                    rows = (Path('/proc') / str(pid) / 'status').read_text().splitlines()
                    values = {name: int(line.split()[1]) for line in rows
                              for name in ('VmRSS:', 'VmHWM:') if line.startswith(name)}
                    for field, target in [('VmRSS:', 'RssKiB'), ('VmHWM:', 'HwmKiB')]:
                        memory[prefix + 'ObservedPeak' + target] = max(memory[prefix + 'ObservedPeak' + target], values.get(field, 0))
                except (OSError, ValueError):
                    pass
            while not finished.is_set():
                sample(child.pid, 'native')
                try:
                    if gateway_pid is None:
                        # The owned gateway is spawned on a Tokio thread, so
                        # inspect children of all native tasks until located.
                        for task in (Path('/proc') / str(child.pid) / 'task').glob('*/children'):
                            for pid in task.read_text().split():
                                if os.readlink(Path('/proc') / pid / 'exe') == gateway:
                                    gateway_pid = pid
                                    break
                            if gateway_pid is not None:
                                break
                    if gateway_pid is not None:
                        sample(gateway_pid, 'gateway')
                except OSError:
                    pass
                memory['samples'] += 1
                finished.wait(0.05)
        monitor = threading.Thread(target=observe_memory, daemon=True)
        monitor.start()
        started = time.monotonic()
        try:
            code = child.wait(timeout=200)
        except subprocess.TimeoutExpired:
            child.kill()
            child.wait()
            raise RuntimeError(f'Native smoke did not finish; see {directory / "app.log"}')
        finally:
            engine_memory = engine_observer.close()
            finished.set()
            monitor.join(timeout=2)
            memory['nativeWallMs'] = round((time.monotonic() - started) * 1000)
            (directory / 'memory.json').write_text(json.dumps(memory, indent=2))
    result_path = directory / 'result.json'
    if not result_path.exists():
        raise RuntimeError(f'Native app exited {code} without a report; see {directory / "app.log"}')
    result = json.loads(result_path.read_text())
    assert memory['nativeObservedPeakRssKiB'] > 0 and memory['gatewayObservedPeakRssKiB'] > 0, memory
    result['observedMemory'] = memory
    assert engine_memory['webContentProcessesObserved'] == 0, 'EPUB engine started without opening a book'
    result['lazyEpubEngine'] = True
    result['lazyBrowserEngine'] = True
    if code != 0 or not result.get('ok') or result.get('display') != 'GdkWaylandDisplay':
        raise RuntimeError(f'Native smoke failed: {result}; see {directory / "app.log"}')
    assert 'Gtk-CRITICAL' not in (directory / 'app.log').read_text(), (
        'GTK critical during native acceptance', directory / 'app.log')
    requests = [json.loads(line) for line in (directory / 'requests.jsonl').read_text().splitlines()]
    methods = {request.get('method') for request in requests}
    assert {'thread/list', 'thread/resume', 'model/list', 'turn/start', 'turn/steer', 'turn/interrupt'} <= methods
    assert not any(request.get('params', {}).get('approvalPolicy') == 'never' for request in requests)
    starts = [request['params'] for request in requests if request.get('method') == 'turn/start']
    assert starts[0]['serviceTier'] == 'default' and starts[0]['model'] == 'native-smoke-model' and starts[0]['effort'] == 'high', starts[0]
    assert starts[1]['serviceTier'] == 'fast', starts[1]
    applied = [request for request in requests if request.get('method') == 'thread/resume'
               and request.get('params', {}).get('config', {}).get('shell_environment_policy', {}).get('set', {}).get('STUDIO_NATIVE_PROFILE_TEST') == 'enabled']
    assert applied and all(request['params'].get('excludeTurns') for request in applied), applied
    assert requests.index(applied[0]) < next(index for index, request in enumerate(requests) if request.get('method') == 'turn/start')
    result['projectEnvironment'] = True
    assert result.get('staleTurnRecovery') and result.get('collapsed')
    document_result = json.loads((directory / 'documents.json').read_text())
    assert document_result.get('ok'), document_result
    flow = document_result.get('flowComments', {})
    assert all(flow.get(key) for key in ['toolbarProjectedText', 'toolbarLatestSelection', 'realReaderContainer',
                                        'modeSwitchDuringWorkerRejected', 'conversationSwitchRejected',
                                        'oldMappedRevisionRejected', 'explicitSaveAndReopen']), flow
    flow_diagrams = document_result.get('flowDiagrams', {})
    assert all(flow_diagrams.get(key) for key in ['ok','isolatedEngineError','sourceReadable','sourceCopy','themeReplay','previewBudget','modeSwitchCleanup','fileUnchanged']), flow_diagrams
    assert flow_diagrams.get('rasterBytesBeforeMount') == 0 and flow_diagrams.get('formats') == ['mermaid','plantuml','d2','math'], flow_diagrams
    assert flow_diagrams.get('mermaidMathLabels') == [3, 7], flow_diagrams
    for name in ['flow-mermaid-math-0.png', 'flow-mermaid-math-1.png']:
        assert (directory/name).stat().st_size > 1000, name
    viewport = flow_diagrams.get('viewportAdmission', {})
    assert all(viewport.get(key) for key in ['ok','twoSlots','nestedClipping','horizontalClipping','offscreenCancellation','lateReplyRejected','pixelsReleased','geometryRetained','resetCleanup','failureSettles','nativeRetry']), viewport
    assert viewport.get('idlePolling') is False and flow_diagrams.get('sequentialRealFormats'), flow_diagrams
    flow_tables = document_result.get('flowTables', {})
    assert all(flow_tables.get(key) for key in ['ok','productionTableWidget','columnFiltering','columnWidths','paging','outerScrollStable','cellCopy','cellSourceComment','wholeTableCopy','wholeTablePrimary','commentBudgetChecked','selectionUnmapCancelled','modeEpochRejected','staleReplyRejected','rowsReleased','fileUnchanged']), flow_tables
    assert flow_tables.get('rowLimit') == 50 and 0 < flow_tables.get('initialRows', 0) <= 50, flow_tables
    assert flow_tables.get('contextFragments') == 2 and flow_tables.get('realIndexPages') and flow_tables.get('coalescedTable') and flow_tables.get('joinedSourceRange') and flow_tables.get('joinedRows', 0) >= 2, flow_tables
    result['documents'] = document_result
    terminal_result = json.loads((directory / 'terminal.json').read_text())
    assert terminal_result.get('ok'), f'{terminal_result}; see {directory}'
    result['terminal'] = terminal_result
    links_result = json.loads((directory / 'links.json').read_text())
    assert links_result.get('ok'), f'{links_result}; see {directory}'
    result['links'] = links_result
    comparison_result = json.loads((directory / 'compare.json').read_text())
    assert comparison_result.get('ok') and comparison_result['sourceNavigation'] and comparison_result['independentModes'], comparison_result
    assert comparison_result['liveLanguage'] and comparison_result['literalFileIdentity'], comparison_result
    result['comparison'] = comparison_result
    for name in ('compare-left-only', 'compare-preview', 'compare-source', 'compare-chinese-left', 'compare-chinese-preview'):
        assert (directory / (name + '.png')).stat().st_size > 10000, name
    workspace_result = json.loads((directory / 'workspace.json').read_text())
    assert all(value.get('ok') for value in workspace_result.values()), f'{workspace_result}; see {directory}'
    assert all(workspace_result['files'][key] for key in ('liveLanguage', 'literalRowBindings', 'menuTargetPreserved')), workspace_result['files']
    assert all(workspace_result['source'][key] for key in ('liveStatusLanguage', 'literalTitle', 'selectionRetained')), workspace_result['source']
    result['workspace'] = workspace_result
    session_result = json.loads((directory / 'sessions.json').read_text())
    assert session_result.get('ok') and session_result['archivedReadOnly'] and session_result['deleteConfirmation'], session_result
    result['sessionManagement'] = session_result
    cache_result = json.loads((directory / 'conversation-cache.json').read_text())
    assert cache_result.get('ok') and cache_result['lateSendReceipt'] and cache_result['sharedImmutableRows'], cache_result
    result['conversationCache'] = cache_result
    chat_result = json.loads((directory / 'chat.json').read_text())
    assert chat_result.get('ok') and chat_result['stablePaging'] and chat_result['syntaxHighlighting'], chat_result
    result['richChat'] = chat_result
    diagram_chat = json.loads((directory / 'chat-diagrams.json').read_text())
    assert diagram_chat.get('ok') and diagram_chat['formats'] == ['mermaid', 'plantuml', 'd2', 'math'] and diagram_chat['recreatedOwnerIsolation'], diagram_chat
    assert diagram_chat.get('mermaidMathLabels') == 2, diagram_chat
    math_result=json.loads((directory/'math.json').read_text())
    assert math_result['ok'] and math_result['darkTheme'] and math_result['sourceEditPreserved'],math_result
    result['math']=math_result
    result['chatDiagrams'] = diagram_chat
    conversation_search = json.loads((directory / 'conversation-search.json').read_text())
    assert conversation_search.get('ok') and conversation_search['staleResultIsolation'], conversation_search
    result['conversationSearch'] = conversation_search
    historical_search = json.loads((directory / 'historical-search.json').read_text())
    assert historical_search.get('ok') and historical_search['liveConversationUnchanged'], historical_search
    result['historicalSearch'] = historical_search
    favorites_result = json.loads((directory / 'favorites.json').read_text())
    assert favorites_result.get('ok') and favorites_result['sourceContext'] and favorites_result['liveConversationUnchanged'], favorites_result
    result['favorites'] = favorites_result
    opencode_source = json.loads((directory / 'opencode-favorite-source.json').read_text())
    assert opencode_source.get('ok') and opencode_source['otherBackend'] and opencode_source['liveConversationUnchanged'], opencode_source
    result['opencodeFavoriteSource'] = opencode_source
    assert (directory / 'opencode-favorite-context.png').stat().st_size > 10000
    assert all((directory / name).stat().st_size > 10000 for name in ('favorites-preview.png', 'favorites-source.png', 'favorite-context.png'))
    exported = (directory / 'favorites-export.md').read_text()
    assert 'Native favorite 中文😀' in exported and 'Shared native note 中文😀' in exported and 'Exported favorites: 1' in exported
    assert (directory / 'historical-context.png').stat().st_size > 10000
    assert any(request.get('method') == 'thread/turns/list' and request.get('params', {}).get('cursor') == 'native-older-page' for request in requests)
    assert (directory / 'conversation-search.png').stat().st_size > 10000
    assert all((directory / name).stat().st_size > 10000 for name in ('chat-table.png', 'chat-code.png', 'chat-top.png'))
    settings_result = json.loads((directory / 'settings.json').read_text())
    assert settings_result.get('ok') and settings_result['secretRedaction'] and settings_result['discardCancellation'], settings_result
    result['settings'] = settings_result
    queue_result=json.loads((directory / 'queue.json').read_text())
    assert queue_result.get('ok') and queue_result['backgroundDispatch'] and queue_result['newDraftRetained'], queue_result
    result['messageQueue']=queue_result
    commands=json.loads((directory/'commands.json').read_text())
    assert commands.get('ok') and commands['permissionConfirmation'] and commands['stalePermissionsIsolation'] and commands['shellLifecycle'] and commands['futureQueuePolicy'] and commands['queuedAfterEarlyCompletion'] and commands['scopedTurnDiff'] and commands['tokenUsage'] and commands['mcpInventory'],commands
    result['commands']=commands
    agents=json.loads((directory/'agents.json').read_text())
    assert agents.get('ok') and agents['rootMembership'] and agents['rootAndChildDrafts'] and agents['selectedAgentSurvivesRefresh'] and agents['staleSessionIsolation'],agents
    assert all((directory/name).stat().st_size>1000 for name in ['agents.png','agents-child.png'])
    result['agents']=agents
    router_settings=json.loads((directory/'router-settings.json').read_text())
    assert router_settings.get('ok') and router_settings['crossBackendTargets'] and router_settings['controllerIdentity'] and router_settings['controllerRecovery'] and router_settings['opencodeControllerRecovery'] and router_settings['newerRoleRetention'] and router_settings['staleConfigRetention'] and router_settings['parentCloseGuard'],router_settings
    assert (directory/'router-settings.png').stat().st_size>1000
    result['routerSettings']=router_settings
    agent_lists=[request['params'] for request in requests if request.get('method')=='thread/list' and request.get('params',{}).get('sourceKinds')]
    assert len(agent_lists)>4 and all(params['useStateDbOnly'] and 'ancestorThreadId' not in params for params in agent_lists),agent_lists
    assert any(params.get('cursor')=='agents:中文/+?' for params in agent_lists)
    assert any(request.get('method')=='thread/read' and request.get('params',{}).get('threadId')=='native-agent-child' and request['params'].get('includeTurns') is False for request in requests)
    assert all((directory/name).stat().st_size>1000 for name in ['commands.png','command-status.png','command-status-usage.png','command-diff.png','mcp-servers.png','permissions.png','shell-command.png'])
    mcp_requests=[request['params'] for request in requests if request.get('method')=='mcpServerStatus/list']
    assert len(mcp_requests)==8 and any(params.get('cursor')=='mcp:中文/+?' for params in mcp_requests),mcp_requests
    shell_requests=[request for request in requests if request.get('method')=='thread/shellCommand']
    assert len(shell_requests)==2 and all(request['params']['threadId']=='native-smoke-thread' for request in shell_requests)
    assert [request['params']['command'] for request in shell_requests]==['native-shell-hold','native-shell-fail']
    permission_turns={next((part.get('text','') for part in request['params']['input'] if part['type']=='text'),''):request['params'] for request in requests if request.get('method')=='turn/start' and any(part.get('text','').startswith('native-command-policy') for part in request['params']['input'])}
    assert permission_turns['native-command-policy']['sandboxPolicy']=={'type':'workspaceWrite','writableRoots':[str(project)],'networkAccess':False}
    assert permission_turns['native-command-policy-queued']['sandboxPolicy']=={'type':'readOnly'}
    assert all(params['approvalPolicy']=='on-request' for params in permission_turns.values())
    assert not any(any(part.get('text','')=='Must not send a new message during shell execution' for part in request.get('params',{}).get('input',[])) for request in requests if request.get('method')=='turn/start')
    assert sum(request.get('method')=='thread/compact/start' for request in requests)==1
    reviews=[request for request in requests if request.get('method')=='review/start']
    assert len(reviews)==2 and all(request['params']['delivery']=='inline' and request['params']['target']=={'type':'uncommittedChanges'} for request in reviews)
    assert permission_turns['native-command-policy-review']['sandboxPolicy']=={'type':'readOnly'}
    references=json.loads((directory/'references.json').read_text())
    assert references.get('ok') and references['queuedReferences'] and references['newerReferencesRetained'],references
    result['references']=references
    assert (directory/'references-picker.png').stat().st_size>1000
    assert (directory/'references.png').stat().st_size>10000
    reference_sends=[request['params'] for request in requests if request.get('method')=='turn/start' and any(part.get('type') in ('skill','mention') for part in request.get('params',{}).get('input',[]))]
    assert len(reference_sends)==3
    assert any(part.get('type')=='mention' and part.get('path')=='plugin://native-plugin@fixture' for part in reference_sends[1]['input'])
    assert reference_sends[2]['input'][1]['name']=='native-second'
    attachments=json.loads((directory/'attachments.json').read_text())
    assert attachments.get('ok') and attachments['clipboardPaste'] and attachments['clipboardSnapshot'] and attachments['fileDrop'] and attachments['newerAttachmentsRetained'],attachments
    result['attachments']=attachments
    assert (directory/'attachments.png').stat().st_size>10000
    image_sends=[request for request in requests if request.get('method')=='turn/start' and any(part.get('type')=='image' for part in request.get('params',{}).get('input',[]))]
    assert len(image_sends)==3,image_sends
    assert [sum(part.get('type')=='image' for part in request['params']['input']) for request in image_sends]==[2,2,1]
    import base64
    for request in image_sends:
        for part in request['params']['input']:
            if part.get('type')=='image':
                assert part['url'].startswith('data:image/png;base64,')
                assert base64.b64decode(part['url'].split(',',1)[1])==(directory/'project/native-image-中文.png').read_bytes()

    assert all((directory / name).stat().st_size>10000 for name in ('queue.png','queue-editor.png'))
    queue_sends=[request for request in requests if request.get('method')=='turn/start' and request.get('params',{}).get('input',[{}])[0].get('text') in ('native-queue-one','native-queue-two','native-queue-three')]
    assert [request['params']['input'][0]['text'] for request in queue_sends]==['native-queue-one','native-queue-two','native-queue-three'], queue_sends
    assert all(request['params']['threadId']=='native-smoke-thread' and request['params'].get('model')=='native-smoke-model' and request['params'].get('effort')=='high' and request['params'].get('serviceTier')=='fast' for request in queue_sends), queue_sends
    assert (directory / 'settings.png').stat().st_size > 10000
    saved_environment = json.loads((profile_directory / 'environments.json').read_text())['profiles'][str(project.resolve())]
    assert saved_environment['variables']['STUDIO_NATIVE_UI_TEST'] == ' 保留\n中文😀 ', saved_environment['variables']
    assert saved_environment['secrets'] == {'STUDIO_NATIVE_STORED_SECRET': 'native-original-secret', 'STUDIO_NATIVE_NEW_SECRET': 'native-private-secret'}
    assert saved_environment['networkPolicy'] == 'enabled' and saved_environment['allowedHosts'] == ['example.invalid', 'api.example.invalid']
    assert (profile_directory / 'environments.json').stat().st_mode & 0o777 == 0o600
    router_preferences=json.loads((profile_directory/'settings.json').read_text())
    assert router_preferences['queueDepth']==3 and router_preferences['contentWidth']=='wide'
    assert router_preferences['router']['controllers']=={'codex':'native-managed-router-id','opencode':'native-managed-opencode-id'}
    assert router_preferences['router']['controllerBackend']=='opencode'
    assert router_preferences['router']['fallbacks']==[{'sessionKey':'codex:native-cache-thread','condition':'External Router change'},{'sessionKey':'opencode:native-oc-thread','condition':'Other condition'}],router_preferences['router']
    with sqlite3.connect(profile_directory/'studio.sqlite3') as database:
        opening=json.loads(database.execute('SELECT message_json FROM session_opening_messages WHERE session_key=?',('codex:native-smoke-thread',)).fetchone()[0])
    assert opening=={'text':'Original opening for native Router','responsibility':'Native newer responsibility 中文😀','source':'history','capturedAt':'native-capture-time','truncated':True},opening
    assert sum(request.get('method') == 'thread/delete' for request in requests) == 1
    opencode_result = json.loads((directory / 'opencode.json').read_text())
    assert opencode_result.get('ok') and opencode_result['fastIdleOrdering'] and opencode_result['authoritativeBusy'], opencode_result
    result['opencode'] = opencode_result
    native_preferences = json.loads((directory / 'config/codex-thread-studio/gnome.json').read_text())
    assert native_preferences['theme'] == 'dark' and native_preferences['selectedBackend'] == 'opencode', native_preferences
    assert native_preferences['selectedSession'] == 'native-oc-thread', native_preferences
    assert 640 <= native_preferences['windowWidth'] <= 8192
    result['nativePreferences'] = True
    with sqlite3.connect(directory / 'config/codex-thread-studio/studio.sqlite3') as database:
        expected_flow=json.loads((directory/'flow-comment-saved.json').read_text())
        flow_records=[record for (raw,) in database.execute('SELECT drafts_json FROM session_annotation_drafts') for record in json.loads(raw) if record['id']==expected_flow['id']]
        assert flow_records==[expected_flow],('Flow comment changed across restart',flow_records,expected_flow)
        assert expected_flow['source']['anchor']['textKind']=='flow' and expected_flow['source']['anchor']['startLine']==1 and expected_flow['source']['anchor']['endLine']==9
        turn_options = database.execute('SELECT model, effort, service_tier FROM session_turn_options WHERE session_key = ?', ('codex:native-smoke-thread',)).fetchone()
        saved_favorites = database.execute('SELECT scope, backend, thread_id, turn_id, item_id, title, note, tags_json FROM favorites').fetchall()
        queues={key:json.loads(value) for key,value in database.execute('SELECT session_key,queue_json FROM session_message_queues')}
    assert queues['codex:native-smoke-thread'][0]['text']=='native-queue-after-restart', queues
    assert queues['opencode:native-oc-thread'][0]['text']=='native-oc-restore-do-not-send', queues
    assert len(saved_favorites) == 1, saved_favorites
    assert saved_favorites[0] == ('message', 'codex', 'native-smoke-thread', 'history-turn', 'history-message', 'Native favorite 中文😀', 'Shared native note 中文😀', '["native","review"]'), saved_favorites
    assert turn_options == ('native-smoke-model', 'high', 'fast'), turn_options
    result['modelEffortFast'] = True
    opencode_requests = [json.loads(line) for line in (directory / 'opencode-requests.jsonl').read_text().splitlines()]
    opencode_history = json.loads((directory / 'opencode-history.json').read_text())
    assert opencode_history.get('ok') and opencode_history['legacyScope'] and opencode_history['cursorCycle'] and opencode_history['liveConversationUnchanged'], opencode_history
    opencode_queue=json.loads((directory/'opencode-queue.json').read_text())
    assert opencode_queue.get('ok') and opencode_queue['earlyCompletion'] and opencode_queue['restoredQueueRetained'],opencode_queue
    result['opencodeQueue']=opencode_queue
    oc_commands=json.loads((directory/'opencode-commands.json').read_text())
    assert oc_commands.get('ok') and oc_commands['shellReply'] and oc_commands['pausedQueueRetained'],oc_commands
    result['opencodeCommands']=oc_commands
    oc_references=json.loads((directory/'opencode-references.json').read_text())
    assert oc_references.get('ok') and oc_references['queuedSkillCompletion'],oc_references
    result['opencodeReferences']=oc_references
    oc_images=json.loads((directory/'opencode-attachments.json').read_text())
    assert oc_images.get('ok') and oc_images['serverOwnedIdentity'],oc_images
    result['opencodeAttachments']=oc_images

    assert (directory/'opencode-structured.png').stat().st_size>10000
    result['opencodeHistory'] = opencode_history
    assert (directory / 'opencode-history-context.png').stat().st_size > 10000
    assert any('before=' in request['path'] for request in opencode_requests)
    other_source_reads = [request for request in opencode_requests if '/session/native-oc-other-thread/' in request['path']]
    assert len(other_source_reads) >= 3 and all(request['method'] == 'GET' for request in other_source_reads), other_source_reads
    prompts = [request for request in opencode_requests if request['path'].split('?')[0].endswith('/prompt_async')]
    assert len(prompts) == 9 and all('messageID' not in request['body'] for request in prompts), prompts
    oc_image_prompts=[request for request in prompts if any(part['type']=='file' and part.get('mime','').startswith('image/') for part in request['body']['parts'])]
    assert len(oc_image_prompts)==1
    oc_image=oc_image_prompts[0]['body']['parts'][0]
    assert oc_image['type']=='file' and oc_image['mime']=='image/png'
    assert base64.b64decode(oc_image['url'].split(',',1)[1])==(directory/'project/native-image-中文.png').read_bytes()

    file_parts=[part for request in prompts for part in request['body']['parts'] if part['type']=='file' and part.get('mime')=='text/plain']
    assert len(file_parts)==1 and file_parts[0]['source']['path']=='nested/说明 note.md'
    from urllib.parse import unquote,urlparse
    assert Path(unquote(urlparse(file_parts[0]['url']).path))==directory/'project/nested/说明 note.md'
    command_sends=[request for request in opencode_requests if request['method']=='POST' and request['path'].split('?')[0].endswith('/command')]
    assert len(command_sends)==4 and [request['body']['command'] for request in command_sends]==['native-build','native-build','compact','review']
    assert all('messageID' not in request['body'] for request in command_sends)
    oc_shells=[request for request in opencode_requests if request['method']=='POST' and request['path'].split('?')[0].endswith('/shell')]
    assert len(oc_shells)==1 and oc_shells[0]['body']=={'agent':'build','model':{'providerID':'native-provider','modelID':'native-model'},'command':'native-opencode-shell'}

    assert (directory / 'opencode.png').stat().st_size > 10000
    assert all((directory / name).stat().st_size > 10_000 for name in ['light.png', 'dark.png', 'compact.png', 'reader.png', 'editor.png', 'diagrams.png', 'terminal.png', 'files.png', 'source.png', 'git.png'])
    backend_pid = int((directory / 'backend.pid').read_text())
    try:
        os.kill(backend_pid, 0)
    except ProcessLookupError:
        result['backendReaped'] = True
    else:
        raise RuntimeError(f'Owned fake backend {backend_pid} was not cleaned up')
    opencode_pid = int((directory / 'opencode.pid').read_text())
    try:
        os.kill(opencode_pid, 0)
    except ProcessLookupError:
        result['opencodeReaped'] = True
    else:
        raise RuntimeError(f'Owned fake OpenCode {opencode_pid} was not cleaned up')
    restore_environment = dict(environment, STUDIO_GNOME_SMOKE_RESTORE='1')
    with (directory / 'restore.log').open('w') as log:
        subprocess.run([native, str(project / 'nested/note.md')], env=restore_environment, stdout=log, stderr=log, check=True, timeout=22)
    restored = json.loads((directory / 'restored.json').read_text())
    assert restored.get('ok') and restored['selectionRestored'] and restored['themeRestored'] and restored['queueRestoredPaused'], restored
    result['restoredPreferences'] = restored
    restored_codex_requests = [json.loads(line) for line in (directory / 'requests.jsonl').read_text().splitlines()]
    assert len(restored_codex_requests) == len(requests), 'Restoring OpenCode eagerly started the unused Codex backend'
    restored_opencode_requests=[json.loads(line) for line in (directory/'opencode-requests.jsonl').read_text().splitlines()]
    assert sum(request['path'].split('?')[0].endswith('/prompt_async') for request in restored_opencode_requests)==len(prompts), 'A restored queue dispatched without Resume'
    agent_restarts={}
    for phase in ['seed','verify']:
        agent_environment=dict(environment,STUDIO_GNOME_SMOKE_AGENT_RESTART=phase)
        with (directory/f'agent-restart-{phase}.log').open('w') as log:
            subprocess.run([native],env=agent_environment,stdout=log,stderr=log,check=True,timeout=30)
        checked=json.loads((directory/f'agent-restart-{phase}.json').read_text())
        assert checked.get('ok') and checked['selectedAgent'] and checked['parentMetadata'] and checked['catalogRetention'],checked
        agent_restarts[phase]=checked
    result['agentRestarts']=agent_restarts
    agent_preferences=json.loads((directory/'config/codex-thread-studio/gnome.json').read_text())
    assert agent_preferences['selectedBackend']=='codex' and agent_preferences['selectedSession']=='native-agent-child',agent_preferences
    agent_opencode_requests=[json.loads(line) for line in (directory/'opencode-requests.jsonl').read_text().splitlines()]
    assert sum(request['path'].split('?')[0].endswith('/prompt_async') for request in agent_opencode_requests)==len(prompts),'Agent restart sent a queued message'
    for pid_file in ('backend.pid', 'opencode.pid'):
        pid = int((directory / pid_file).read_text())
        try:
            os.kill(pid, 0)
        except ProcessLookupError:
            pass
        else:
            raise RuntimeError(f'Restored native app did not reap {pid_file}: {pid}')
    result['artifacts'] = str(directory)
    print(json.dumps(result, indent=2))

if __name__ == '__main__':
    main()
