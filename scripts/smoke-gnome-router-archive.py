#!/usr/bin/env python3
"""Browser-created Router history: real SQLite, private providers, native controls."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import resource
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import time
import uuid
from urllib.parse import urlencode

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).with_name('test-support')))
from private_wayland import isolated_environment, OwnedMutterInput
from native_accessibility import Client
from memory_sampler import ProcessMemory

spec = importlib.util.spec_from_file_location('archive_runtime', Path(__file__).with_name('smoke-gnome-router-runtime.py'))
runtime = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runtime)
ENDPOINT = '/studio/native/router/browser-history'

CODEX_READS = r'''
    elif method=='model/list':answer(identity,{'data':[{'id':'target-model','model':'target-model','isDefault':True}]})
    elif method=='thread/turns/list':
        offset=int(params.get('cursor','archive:0').split(':')[1]);limit=params['limit'];assert limit==4
        history=list(reversed(turns.get(session,[])))
        cursor=('archive:0' if session=='target' and (root/'archive-repeated-cursor').exists() else 'archive:'+str(offset+limit)) if offset+limit<len(history) else None
        answer(identity,{'data':history[offset:offset+limit],'nextCursor':cursor})
    elif method=='thread/items/list':
        entries=[{'turnId':t['id'],'item':i} for t in reversed(turns.get(session,[])) for i in reversed(t['items'])]
        answer(identity,{'data':entries[:120],'nextCursor':None})
'''


def codex():
    value=runtime.CODEX.replace("    elif method=='thread/read':", CODEX_READS + "\n    elif method=='thread/read':")
    value=value.replace("busy=any(turn['status']=='inProgress' for turn in turns.get(identity,[]))", "busy=any(turn['status']=='inProgress' for turn in turns.get(identity,[])) or (identity=='target' and (root/'archive-language').exists())")
    return value.replace("'name':'Router' if identity=='router' else 'Native '+identity", "'name':'Router' if identity=='router' else 'Search results {count}' if identity=='target' and (root/'archive-language').exists() else 'Native '+identity")


def opencode():
    value = runtime.OPENCODE.replace('def response(self,value=None,status=200):', 'def response(self,value=None,status=200,cursor=None):')
    # ID-addressed session metadata discovers the directory of an old browser
    # record. All subsequent message pages must use that verified directory.
    value = value.replace("        assert query.get('directory')==[session(identity)['directory']],query", "        if not (self.command=='GET' and path=='/session/'+identity and not query):assert query.get('directory')==[session(identity)['directory']],query")
    value = value.replace("self.send_header('Content-Length',str(len(body)));self.end_headers();", "self.send_header('Content-Length',str(len(body)));\n        if cursor:self.send_header('x-next-cursor',cursor)\n        self.end_headers();")
    start = value.index("        if path.endswith('/message'):")
    end = value.index("        if path.endswith('/prompt_async'):", start)
    value = value[:start] + r'''
        if path.endswith('/message'):
            limit=int(query.get('limit',['100'])[0]);assert limit<=100
            history=messages.get(identity,[]);end=int(query['before'][0].split(':')[1]) if query.get('before') else len(history)
            begin=max(0,end-limit)
            selected=json.loads(json.dumps(history[begin:end]))
            if identity=='oc-target':
                for message in selected:
                    if message['info']['id']=='oc-old-answer' and (root/'archive-foreign-part').exists():message['parts'][0]['sessionID']='foreign-source'
                    if message['info']['id']=='oc-newer-answer' and (root/'archive-structured').exists():message['info']['structured']={'archive':'structured 中文😀'};message['parts']=[]
            return self.response(selected,cursor='archive:'+str(begin) if begin else None)
''' + value[end:]
    return value


def create(root):
    runtime.create_profile(root)
    def turn(identity, prompt, answer=''):
        return dict(id=identity, status='completed', items=[dict(id=identity+'-user', type='userMessage', content=[dict(type='text',text=prompt)]), *([dict(id='old-answer',type='agentMessage',text=answer)] if answer else [])])
    turns = dict(router=[turn('router-turn-'+str(n), 'Original controller request '+str(n)) for n in range(53)],
        target=[turn('target-old','Actual accepted source request','# Archived Codex answer 中文😀'),
                *[turn('target-filler-'+str(n),'Later unrelated source task') for n in range(12)]])
    (root/'codex-turns.json').write_text(json.dumps(turns))
    def message(session, identity, role, value, parent=None):
        info=dict(id=identity,role=role,sessionID=session)
        if parent:info.update(parentID=parent,time=dict(completed=1))
        return dict(info=info,parts=[dict(id=identity+'-part',type='text',text=value,messageID=identity,sessionID=session)])
    oc = {}
    for session in ['oc-router','oc-target']:
        original = 'oc-controller-old' if session=='oc-router' else 'oc-old-user'
        oc[session] = [message(session,original,'user','Original OpenCode controller request' if session=='oc-router' else 'Actual accepted OpenCode request'),
            message(session,'oc-old-answer','assistant','Archived OpenCode answer first',original),
            *[message(session,'filler-'+str(n),'user','Unrelated old message') for n in range(120)],
            message(session,'oc-newer-answer','assistant','Archived OpenCode answer last',original)]
    (root/'oc-turns.json').write_text(json.dumps(oc))


def database(root):
    return root/'config/codex-thread-studio/studio.sqlite3'


def stored(root):
    with sqlite3.connect(f'file:{database(root)}?mode=ro',uri=True) as db:
        return db.execute('SELECT controller,turn_key,dispatch FROM router_dispatches ORDER BY controller,turn_key').fetchall()


def api(gateway, root):
    samples=[]
    def request(**query):
        started=time.monotonic()
        status,value=gateway.api(ENDPOINT+'?'+urlencode(query),timeout=35)
        samples.append(dict(status=status,ms=round((time.monotonic()-started)*1000,2),bytes=len(json.dumps(value).encode())))
        return status,value
    def save(controller,key,value):
        assert gateway.api('/studio/router-history',dict(controller=controller,turnKey=key,dispatch=value),method='PUT')[0]==204
    assert gateway.api(ENDPOINT+'?controller=codex:router',authorized=False)[0]==401
    assert gateway.api('/studio/native/router/jobs?controller=codex:router')[0]==200
    native_id=str(uuid.uuid4())
    job=dict(id=native_id,controller='codex:router',createdAt=1,prompt='Native current request',status='clarify',controllerTurnId='',target=None,targetTurnId='',decision=None,response='Native current answer',error='',unread=False)
    with sqlite3.connect(database(root)) as db:
        db.execute('INSERT INTO native_router_jobs(id,controller,created,owner,phase,json,receipt,unread) VALUES(?,?,?,?,?,?,?,?)',
            (native_id,'codex:router',1,'private-archive-test','clarify',json.dumps(job),uuid.uuid4().hex,0))
    for n in range(53):
        value=dict(status='completed' if n%2 else 'running',targetTurnId='target-old',unread=True,futureMetadata=dict(keep='literal future value'),
            decision=dict(action='dispatch',targetSessionKey='codex:target',forwardedPrompt='Forwarded task '+str(n)+(' rare-old中文 ' if n==0 else ''),reason='Existing responsibility',message=''))
        if n:value['directTurn']=dict(id='router-turn-'+str(n),items=[dict(type='userMessage',content=[dict(type='text',text='Direct browser request '+str(n))])])
        save('codex:router','codex:router-turn-'+str(n),value)
    save('codex:router','codex:cross-provider',dict(status='completed',targetTurnId='oc-old-user',
        directTurn=dict(items=[dict(type='userMessage',content=[dict(type='text',text='Cross-provider browser request')])]),
        decision=dict(action='dispatch',targetSessionKey='opencode:oc-target',forwardedPrompt='Recorded OpenCode question',reason='',message='')))
    save('opencode:oc-router','opencode:oc-controller-old',dict(status='completed',targetTurnId='target-old',
        decision=dict(action='dispatch',targetSessionKey='codex:target',forwardedPrompt='Recorded source question',reason='',message='')))
    save('codex:router','codex:native-'+native_id,dict(nativeJobId=native_id,status='clarify'))
    original=stored(root)
    ledger=runtime.requests(root)
    first=request(controller='codex:router')[1]
    assert len(first['data'])==20 and first['nextCursor'] and not first['newerCursor']
    middle=request(controller='codex:router',before=first['nextCursor'])[1]
    last=request(controller='codex:router',before=middle['nextCursor'])[1]
    assert len(middle['data'])==20 and len(last['data'])==14 and not last['nextCursor']
    assert request(controller='codex:router',after=last['newerCursor'])[1]['data']==middle['data']
    assert not first['activeIds'] and not first['controllerBusy'] and first['unreadCount']==0
    assert ledger==runtime.requests(root),'Metadata browsing started a provider read'
    rare=request(controller='codex:router',query='rare-old中文 ')[1]
    assert len(rare['data'])==1
    assert not request(controller='codex:router',query='rare-old中文  ')[1]['data']
    oldest_id=rare['data'][0]['id']
    status,oldest=request(id=oldest_id)
    assert status==200 and oldest['browserHistory'] and oldest['prompt']=='Original controller request 0',oldest
    assert oldest['target']['cwd']==str(root/'project') and oldest['target']['sessionKey']=='codex:target'
    assert oldest['targetTurnId']=='target-old' and oldest['responseItems']==[dict(id='old-answer',text='# Archived Codex answer 中文😀',excerpt=False)]
    assert oldest['decision']['forwardedPrompt']=='Actual accepted source request'
    status,cross=request(id=first['data'][0]['id'])
    assert status==200 and cross['prompt']=='Cross-provider browser request',cross
    assert cross['target']['cwd']==str(root/'project-oc')
    assert [p['id'] for p in cross['responseItems']]==['oc-old-answer-part','oc-newer-answer-part'],cross
    assert cross['decision']['forwardedPrompt']=='Actual accepted OpenCode request'
    for flag,identity,needle in [('archive-repeated-cursor',oldest_id,'cursor'),('archive-foreign-part',first['data'][0]['id'],'another reply or conversation')]:
        (root/flag).touch()
        try:
            status,rejected=request(id=identity)
            assert status==200 and not rejected['responseItems'] and needle in rejected['dispatchNotice'],rejected
        finally:(root/flag).unlink()
    (root/'archive-structured').touch()
    try:
        status,structured=request(id=first['data'][0]['id'])
        assert status==200 and [p['id'] for p in structured['responseItems']]==['oc-old-answer-part','oc-newer-answer-structured'],structured
        assert json.loads(structured['responseItems'][-1]['text'])=={'archive':'structured 中文😀'}
    finally:(root/'archive-structured').unlink()
    oc=request(controller='opencode:oc-router')[1]
    assert len(oc['data'])==1
    assert request(id=oc['data'][0]['id'])[1]['prompt']=='Original OpenCode controller request'
    for bad in [dict(controller='codex:router',before=1,after=2),dict(controller='codex:router',query='x'*513),dict(id='browser-0-not-an-identity'),dict(id=oldest_id,controller='codex:router')]:
        assert request(**bad)[0]==400
    assert stored(root)==original,'Read-only archive rewrote shared browser data'
    assert not runtime.starts(root),'Archive reading started inference'
    assert any(r.get('method')=='thread/turns/list' and r['params'].get('cursor')=='archive:52' for r in runtime.requests(root))
    oc_requests=[json.loads(s) for s in (root/'opencode-requests.jsonl').read_text().splitlines()]
    assert any(r['query'].get('before') and r['query'].get('directory')==[str(root/'project-oc')] for r in oc_requests)
    assert all(r['method']=='GET' for r in oc_requests),'Archive OpenCode source read mutated a provider'
    assert not any(r.get('method') in ['thread/resume','thread/start','turn/start','turn/interrupt'] for r in runtime.requests(root))
    with sqlite3.connect(database(root)) as db:
        plan=db.execute('EXPLAIN QUERY PLAN SELECT rowid FROM router_dispatches WHERE controller=? ORDER BY rowid DESC LIMIT 21',('codex:router',)).fetchall()
    assert any('router_dispatches_page' in str(row) for row in plan),plan
    result=dict(ok=True,browserRecords=55,codexHistory=54,metadataOnlyPages=True,sourceReadsOnly=True,
        oldControllerAndSourcePages=True,crossProvider=True,exactSourceQuestion=True,paging=True,
        spacesSignificant=True,nativeMirrorsExcluded=True,originalRecordsUnchanged=True,
        noInference=True,sourceIdentities=True,indexedPages=True,samples=samples,artifacts=str(root))
    result.update(repeatedCursorRejected=True,foreignPartRejected=True,structuredOpenCodeSource=True)
    (root/'archive-api-result.json').write_text(json.dumps(result,indent=2)+'\n')
    return result


def gui(native,gateway,root):
    owned=OwnedMutterInput()
    log=(root/'native.log').open('w')
    child=subprocess.Popen([native],cwd=root,env=os.environ,stdout=log,stderr=log)
    desktop=Client(child);memory=ProcessMemory(child.pid,gateway)
    def keys(values):
        for k in values:owned.call('NotifyKeyboardKeysym',owned.GLib.Variant('(ub)',(k,True)))
        time.sleep(.06)
        for k in reversed(values):owned.call('NotifyKeyboardKeysym',owned.GLib.Variant('(ub)',(k,False)))
    def choose(index):
        node=desktop.wait('Router history selector',lambda:desktop.find(lambda n:n.get_role_name()=='combo box'
            and n.get_name() in ['Current requests','Earlier browser requests'] and desktop.showing(n)))
        for _ in range(80):
            focused=desktop.find(lambda n:n.get_state_set().contains(desktop.api.StateType.FOCUSED))
            if focused and desktop.within(focused,node):break
            keys([0xff09])
        else:raise RuntimeError('Router history selector is not keyboard reachable')
        keys([0x20])
        wanted=['Current requests','Earlier browser requests'][index]
        def option():
            for label in desktop.tree():
                if label.get_name()!=wanted or label.get_role_name()!='label' or not desktop.showing(label):continue
                current=label
                for _ in range(8):
                    if current.get_role_name()=='list item':return current
                    current=current.get_parent()
                    if current is None:break
        row=desktop.wait('history selector option: '+wanted,option)
        (root/('archive-selector-'+str(index)+'.json')).write_text(json.dumps(desktop.describe(),indent=2))
        def trace(phase):
            nodes=[]
            for n in desktop.tree():
                try:
                    if n.get_state_set().contains(desktop.api.StateType.FOCUSED):nodes.append(dict(name=n.get_name(),role=n.get_role_name()))
                except desktop.glib.Error:pass
            with (root/'archive-focus.jsonl').open('a') as out:out.write(json.dumps(dict(phase=phase,index=index,focused=nodes))+'\n')
        trace('mapped popup')
        keys([0xff57 if index else 0xff50])
        trace('edge key')
        keys([0xff0d])
        trace('Enter')
        desktop.wait('selected history source: '+wanted,lambda:desktop.find(lambda n:
            n.get_role_name()=='combo box' and n.get_name()==wanted and desktop.showing(n)))
    def language(chinese):
        keys([0xffe3,ord(',')])
        desktop.wait('language settings',lambda:desktop.text('Apply language') or desktop.text('应用语言'))
        combo=desktop.wait('language picker',lambda:desktop.find(lambda n:n.get_role_name()=='combo box'
            and n.get_name() in ['Language','语言'] and desktop.showing(n)))
        for _ in range(80):
            current=desktop.find(lambda n:n.get_state_set().contains(desktop.api.StateType.FOCUSED))
            parent=combo.get_parent()
            if current and (desktop.within(current,combo) or (parent and parent.get_role_name()=='list item' and desktop.within(current,parent))):break
            keys([0xff09])
        else:raise RuntimeError('Language picker is not keyboard reachable')
        keys([0x20]);desktop.wait('language choices',lambda:desktop.text('简体中文') and desktop.text('English'))
        keys([0xff50])
        for _ in range(1 if chinese else 2):keys([0xff54])
        keys([0xff0d]);desktop.click('Apply language' if chinese else '应用语言')
        desktop.wait('saved locale',lambda:json.loads((root/'config/codex-thread-studio/settings.json').read_text()).get('language')==('zh-CN' if chinese else 'en-US'))
        dialog=desktop.wait('translated Settings dialog',lambda:desktop.find(lambda n:n.get_role_name()=='dialog'
            and n.get_name()==('设置' if chinese else 'Settings') and desktop.showing(n)))
        desktop.activate(desktop.wait('Settings Close',lambda:desktop.find(lambda n:n.get_name() in ['Close','关闭']
            and 'button' in n.get_role_name() and desktop.within(n,dialog) and desktop.showing(n))))
        desktop.wait('closed Settings',lambda:not desktop.find(lambda n:n.get_role_name()=='dialog'
            and n.get_name() in ['Settings','设置'] and desktop.showing(n)))
    original=stored(root)
    def capture(name):
        if os.environ.get('STUDIO_GNOME_SMOKE_CAPTURE_ONLY')!='1':return
        (root/'native-capture-request').write_text(name)
        desktop.wait('mapped screenshot: '+name,lambda:(root/(name+'.png')).is_file())
    try:
        desktop.wait('current native request',lambda:desktop.text('Native current request'))
        owned.focus_only_window()
        composer=desktop.wait('native composer',lambda:desktop.find(lambda n:n.get_role_name()=='text' and n.get_name().startswith('Write a message.')))
        desktop.edit(composer,'Unsent archive draft 中文😀 ')
        choose(1)
        desktop.wait('cross-provider archived reply',lambda:desktop.text('Archived OpenCode answer last'),seconds=45)
        capture('router-browser-history')
        assert desktop.contents(composer)=='Unsent archive draft 中文😀 '
        desktop.click('Search Router requests and replies')
        search=desktop.wait('archive search entry',lambda:desktop.find(lambda n:desktop.showing(n) and n.get_role_name()=='entry' and 'Search saved' in n.get_name()))
        desktop.edit(search,'rare-old中文 ')
        desktop.wait('oldest original request',lambda:desktop.text('Original controller request 0'),seconds=45)
        desktop.wait('old source answer',lambda:desktop.text('Archived Codex answer 中文😀'))
        capture('router-browser-search')
        if (root/'archive-language').exists():
            desktop.wait('English sidebar activity and pin',lambda:desktop.text('Working · Pinned · Search results {count}'))
            language(True)
            desktop.wait('translated Router result count',lambda:desktop.text('搜索结果 · 1 条请求 · 0 条未读'))
            desktop.wait('translated sidebar prefixes',lambda:desktop.text('正在处理 · 已置顶 · Search results {count}'))
            desktop.wait('translated history source',lambda:desktop.find(lambda n:n.get_role_name()=='combo box' and n.get_name()=='早期浏览器请求'))
            assert desktop.contents(search)=='rare-old中文 ' and desktop.contents(composer)=='Unsent archive draft 中文😀 '
            assert desktop.text('Original controller request 0') and desktop.text('Archived Codex answer 中文😀')
            capture('router-browser-language-zh')
            desktop.edit(search,'rare-old中文  ')
            desktop.wait('translated empty count',lambda:desktop.text('搜索结果 · 0 条请求 · 0 条未读'))
            desktop.edit(search,'Direct browser request')
            desktop.wait('translated multiple count',lambda:desktop.text('搜索结果 · 20 条请求 · 0 条未读'))
            desktop.edit(search,'')
            desktop.wait('translated archive caption',lambda:desktop.text('早期浏览器请求 · 0 条未读'))
            desktop.edit(search,'rare-old中文 ')
            desktop.wait('retained original source',lambda:desktop.text('搜索结果 · 1 条请求 · 0 条未读') and desktop.text('Original controller request 0'))
            language(False)
            desktop.wait('English Router count restored',lambda:desktop.text('Search results · 1 request · 0 unread'))
            desktop.wait('English sidebar prefixes restored',lambda:desktop.text('Working · Pinned · Search results {count}'))
            assert desktop.contents(search)=='rare-old中文 ' and desktop.contents(composer)=='Unsent archive draft 中文😀 '
        desktop.edit(search,'rare-old中文  ')
        desktop.wait('significant trailing space',lambda:desktop.text('Search results · 0 requests'))
        choose(0)
        desktop.edit(search,'')
        desktop.wait('returned current request',lambda:desktop.text('Native current request'))
        assert desktop.contents(composer)=='Unsent archive draft 中文😀 '
        assert stored(root)==original
        owned.focus_only_window();keys([0xffe9,0xffc1]);child.wait(timeout=25);assert child.returncode==0
    except BaseException:
        (root/'archive-accessibility.json').write_text(json.dumps(desktop.describe(),indent=2,ensure_ascii=False));raise
    finally:
        if child.poll() is None:child.terminate();child.wait(timeout=10)
        measured=memory.close();owned.close();log.close()
    assert not runtime.starts(root)
    assert not any(t in (root/'native.log').read_text() for t in ['CRITICAL','Gtk-WARNING','panicked'])
    result=dict(ok=True,realNativeControls=True,noApplicationHooks=not bool(os.environ.get('STUDIO_GNOME_SMOKE_CAPTURE_ONLY')),
        passiveCapture=bool(os.environ.get('STUDIO_GNOME_SMOKE_CAPTURE_ONLY')),installedPair=os.environ.get('STUDIO_ARCHIVE_INSTALLED')=='1',
        noGatewayOverride='CODEX_THREAD_STUDIO_GATEWAY_BIN' not in os.environ,historySourceSwitch=True,
        archivedReplies=True,search=True,significantSpaces=True,currentViewReturn=True,
        draftPreserved=True,originalRecordsUnchanged=True,noInference=True,memory=measured,artifacts=str(root))
    result['liveRouterSidebarLanguage']=(root/'archive-language').exists()
    (root/'archive-ui-result.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('native');p.add_argument('gateway');p.add_argument('--api-only',action='store_true');p.add_argument('--private-root');p.add_argument('--capture',action='store_true');p.add_argument('--installed',action='store_true');p.add_argument('--language',action='store_true');args=p.parse_args()
    if args.capture and args.installed:p.error('Passive screenshot capture is debug-only; installed checks use no application hooks')
    resource.setrlimit(resource.RLIMIT_CORE,(0,0));native,gateway=str(Path(args.native).absolute()),str(Path(args.gateway).absolute())
    if args.private_root:return gui(native,gateway,Path(args.private_root))
    base=Path(__file__).resolve().parents[1]/'target/gnome-native-smoke';base.mkdir(parents=True,exist_ok=True)
    root=Path(tempfile.mkdtemp(prefix='router-archive-',dir=base));create(root)
    if args.installed:
        bundle=Path(native).parent;prefix=root/'installed native 中文 % archive'
        assert Path(gateway).parent==bundle and (bundle/'SHA256SUMS').is_file()
        subprocess.run(['bash',str(bundle/'install.sh'),str(bundle),str(prefix)],check=True,stdout=subprocess.DEVNULL)
        native,gateway=str(prefix/'bin/codex-thread-studio-gnome'),str(prefix/'bin/codex-thread-studio-gateway')
    runtime.fixture.BACKEND=codex();runtime.fixture.OPENCODE_BACKEND=opencode()
    process=runtime.fixture.Gateway(Path(gateway),root,'opencode')
    try:result=api(process,root)
    finally:process.close()
    print(json.dumps(result),flush=True)
    if args.api_only:return
    if args.language:
        (root/'archive-language').touch()
        with sqlite3.connect(database(root)) as db:db.execute("INSERT INTO session_pins(session_key,pinned_at) VALUES('codex:target',1)")
    for name,code in [('fake-codex.py',codex()),('fake-opencode.py',opencode())]:
        path=root/name;path.write_text(code);path.chmod(0o700)
    (root/'config/codex-thread-studio/gnome.json').write_text(json.dumps(dict(selectedBackend='codex',selectedSession='router')))
    env=dict(os.environ,CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway,CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),CODEX_THREAD_STUDIO_BACKENDS_CONFIG=str(root/'config/codex-thread-studio/backends.json'),CODEX_HOME=str(root/'codex'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),READER_CACHE_DIR=str(root/'reader-cache'),GDK_BACKEND='wayland',GIO_USE_VFS='local',GIO_USE_VOLUME_MONITOR='unix',GTK_A11Y='atspi')
    for key in list(env):
        if key.startswith('STUDIO_GNOME_SMOKE'):env.pop(key)
    if args.capture:env.update(STUDIO_GNOME_SMOKE_DIR=str(root),STUDIO_GNOME_SMOKE_CAPTURE_ONLY='1')
    if args.installed:env.pop('CODEX_THREAD_STUDIO_GATEWAY_BIN',None);env['STUDIO_ARCHIVE_INSTALLED']='1'
    with isolated_environment(env,headless=True) as private:
        subprocess.run(['dbus-run-session','--',shutil.which('mutter') or 'mutter','--headless','--wayland','--no-x11','--wayland-display','studio-test','--virtual-monitor','1600x1000','--',sys.executable,str(Path(__file__).resolve()),native,gateway,'--private-root',str(root)],cwd=root,env=private,check=True,timeout=160)


if __name__=='__main__':main()
