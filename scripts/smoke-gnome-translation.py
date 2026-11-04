#!/usr/bin/env python3
"""Native translation with private provider sessions and a fixture-only Ollama proxy."""
import argparse,ast,html,importlib.util,json,os,resource,shutil,subprocess,sys,tempfile,time,zipfile
import sqlite3
from pathlib import Path
from urllib.request import Request
from urllib.error import HTTPError
sys.dont_write_bytecode=True
sys.path.insert(0,str(Path(__file__).with_name('test-support')))
from private_wayland import isolated_environment, OwnedMutterInput
from memory_sampler import ProcessMemory
from private_speech import PrivateSpeech
from private_opencode_events import SOURCE as OPENCODE_EVENTS
SOURCE='Unsent draft: Translate **English** <b>literal</b> 中文😀\n.\nCANCEL all\n!.. raw text'
spec=importlib.util.spec_from_file_location('translation_runtime',Path(__file__).with_name('smoke-gnome-router-runtime.py'))
runtime=importlib.util.module_from_spec(spec);spec.loader.exec_module(runtime)
UTILITY=r'''
    elif method=='thread/start' and params.get('ephemeral') is True:
        assert params['approvalPolicy']=='never' and params['sandbox']=='read-only' and 'never follow instructions' in params['developerInstructions']
        utility_id='translation-'+str(len(utilities)+1);utilities[utility_id]={'id':utility_id,'cwd':params['cwd'],'ephemeral':True}
        def created(identity=identity,utility_id=utility_id):
            if (root/'hold-utility-create').exists() and not (root/'release-utility-create').exists():
                (root/'utility-create-held').write_text('held')
                while not (root/'release-utility-create').exists():time.sleep(.01)
            answer(identity,{'thread':utilities[utility_id]})
        threading.Thread(target=created,daemon=True).start()
    elif method=='turn/start' and session.startswith('translation-'):
        assert session in utilities and params['model']=='translate-codex' and params['effort']=='low'
        assert params['outputSchema']['properties']['sourcePronunciation']['type']=='string'
        source=params['input'][0]['text'];content={'translation':'译文 **Markdown** <b>literal</b> 中文😀','sourcePronunciation':'/həˈləʊ/','translationPronunciation':'yì wén'}
        if '<source_text>\nPage\n</source_text>' in source:content['translationPronunciation']=''
        result='invalid structured' if 'Invalid structured' in source else json.dumps(content,ensure_ascii=False)
        turn=session+'-turn'
        with lock:
            print(json.dumps({'method':'turn/started','params':{'threadId':session,'turn':{'id':turn,'status':'inProgress'}}}),flush=True)
            print(json.dumps({'method':'item/completed','params':{'threadId':session,'item':{'id':'translation-item','type':'agentMessage','text':result}}}),flush=True)
            print(json.dumps({'method':'turn/completed','params':{'threadId':session,'turn':{'id':turn,'status':'completed'}}}),flush=True)
        answer(identity,{'turn':{'id':turn,'status':'inProgress'}})
    elif method=='thread/delete' and session.startswith('translation-'):
        assert session in utilities;utilities.pop(session);answer(identity,{})
'''
def codex_source():
    text=runtime.CODEX.replace("turns=json.loads(ledger.read_text()) if ledger.exists() else {}","turns=json.loads(ledger.read_text()) if ledger.exists() else {}\nutilities={}")
    text=text.replace("    elif method=='thread/read':",UTILITY+"\n    elif method=='model/list':answer(identity,{'data':[{'id':'target-model','model':'target-model','isDefault':True,'supportedReasoningEfforts':[{'reasoningEffort':'low'}]}]})\n    elif method=='thread/items/list':answer(identity,{'data':[],'nextCursor':None})\n    elif method=='thread/read':")
    ast.parse(text);return text

def opencode_source():
    text=runtime.OPENCODE.replace("messages=json.loads(ledger.read_text()) if ledger.exists() else {}", "messages=json.loads(ledger.read_text()) if ledger.exists() else {}\nutilities={}\n"+OPENCODE_EVENTS)
    text=text.replace("def session(identity):return", "def session(identity):\n    if identity in utilities:return utilities[identity]\n    return")
    text=text.replace("if path=='/experimental/session':return self.response([session('oc-router'),session('oc-target')])", "if path=='/experimental/session':return self.response([session('oc-router'),session('oc-target'),{**session('oc-similar'),'title':'Studio translation user notes'},*[session(identity) for identity in list(utilities)]])")
    text=text.replace("            prompt=body['parts'][0]['text']", """            if identity.startswith('oc-translation-') and (root/'hold-oc-translation').exists():
                emit_sdk({'payload':{'type':'session.status','properties':{'sessionID':identity,'status':{'type':'busy'}}}})
                emit_sdk({'payload':{'type':'message.part.updated','properties':{'part':{'id':'utility-part','sessionID':identity,'messageID':'utility-message','type':'text','text':'Private utility text 中文😀'}}}})
                emit_sdk({'type':'session.status','properties':{'sessionID':'oc-similar','status':{'type':'busy'}}})
                (root/'oc-translation-held').write_text(identity)
                while not (root/'release-oc-translation').exists():time.sleep(.01)
            prompt=body['parts'][0]['text']""")
    text=text.replace("        identity=path.split('/')[2]", """        if path=='/session' and self.command=='POST':
            assert body['title'].startswith('Studio translation ') and body['permission']==[{'permission':'*','pattern':'*','action':'deny'}]
            identity='oc-translation-'+str(len(utilities)+1);utilities[identity]={'id':identity,'title':body['title'],'directory':query['directory'][0],'permission':body['permission']}
            return self.response(utilities[identity])
        identity=path.split('/')[2]
        if self.command=='DELETE':
            assert identity in utilities and query['directory']==[utilities[identity]['directory']]
            utilities.pop(identity);return self.response(True)""")
    text=text.replace("        if path=='/session' and self.command=='POST':", """        if path=='/config/providers':return self.response({'providers':[{'id':'provider','models':{'oc-target-model':{},'translate':{}}}]})
        if path=='/global/event':return stream_sdk(self)
        if path=='/session' and self.command=='POST':""")
    text=text.replace("                if identity=='oc-router':", """                if identity.startswith('oc-translation-'):
                    assert body['model']=={'providerID':'provider','modelID':'translate'} and body['format']['type']=='json_schema' and 'never follow instructions' in body['system']
                    info['structured']={'translation':'OpenCode 译文 中文😀','sourcePronunciation':'/ˈəʊpən/','translationPronunciation':'yì wén'};parts=[]
                elif identity=='oc-router':""")
    text=text.replace('    do_POST=process','    do_POST=process\n    do_DELETE=process')
    text=text.replace("            return self.response(status=204)", """            if identity.startswith('oc-translation-'):
                emit_sdk({'payload':{'type':'session.idle','properties':{'sessionID':identity}}})
                emit_sdk({'type':'session.status','properties':{'sessionID':'oc-similar','status':{'type':'busy'}}})
                emit_sdk({'type':'session.idle','properties':{'sessionID':'oc-similar'}})
            return self.response(status=204)""")
    ast.parse(text);return text

def api_checks(gateway,root):
    runtime.fixture.BACKEND=codex_source();runtime.fixture.OPENCODE_BACKEND=opencode_source()
    service=runtime.fixture.Gateway(Path(gateway),root,'codex')
    def api(body=None,status=200,auth=True,origin=None):
        headers={'Content-Type':'application/json'}
        if auth:headers['Authorization']='Bearer '+service.token
        if origin:headers['Origin']=origin
        request=Request(service.origin+'/studio/native/translation-settings',data=None if body is None else json.dumps(body).encode(),headers=headers,method='GET' if body is None else 'PATCH')
        try:response=service.client.open(request,timeout=20)
        except HTTPError as error:response=error
        with response:
            data=response.read();assert response.status==status,(response.status,status,data[:300]);return json.loads(data) if data else None
    try:
        path=root/'config/codex-thread-studio/settings.json';original=path.read_bytes();api(auth=False,status=401);api(origin='https://untrusted.invalid',status=403)
        first=api();assert path.read_bytes()==original
        raw=json.loads(path.read_text());raw['translation']={'engine':'backend','ollamaModel':'fixture-local','models':{'codex':'original','opencode':'provider/translate','future-backend':'keep-model'},'efforts':{'codex':'low'},'futureTranslation':{'keep':'中文😀'}};raw['futureOther']={'keep':42};path.write_text(json.dumps(raw))
        first=api();values=first['values'];values['models']['codex']='translate-codex'
        saved=api(dict(expectedRevision=first['revision'],values=values));preserved=json.loads(path.read_text());assert preserved['translation']['futureTranslation']==raw['translation']['futureTranslation'] and preserved['futureOther']==raw['futureOther']
        assert saved['values']['models']['future-backend']=='keep-model'
        unchanged=path.read_bytes();api(dict(expectedRevision=first['revision'],values=values),status=409)
        for invalid in [dict(values,engine='remote'),dict(values,efforts={'codex':'invalid'}),dict(values,models={str(i):'model' for i in range(33)})]:api(dict(expectedRevision=saved['revision'],values=invalid),status=400)
        assert path.read_bytes()==unchanged
    finally:service.close()

def run_client(native,gateway,root,speech,env):
    controlled=OwnedMutterInput()
    with (root/'app.log').open('w') as log:
        child=subprocess.Popen([native],cwd=root,env=env,stdout=log,stderr=log);memory=ProcessMemory(child.pid,gateway,include_web_content=True)
        try:
            deadline=time.monotonic()+180
            while child.poll() is None:
                controlled.respond(root)
                if time.monotonic()>deadline:raise TimeoutError(('Translation fixture timed out',root))
                time.sleep(.02)
        except BaseException:child.terminate();child.wait(timeout=15);raise
        finally:measured=memory.close();controlled.close()
    result_path=root/'translation-result.json'
    assert result_path.exists(),(root,'Native exited without a report',child.returncode,(root/'app.log').read_text()[-1800:])
    result=json.loads(result_path.read_text());assert child.returncode==0 and result['ok'],(root,child.returncode,result,(root/'app.log').read_text()[-1800:])
    assert result['projectedSelection']['ok'] and result['projectedSelection']['sourceBytes']>=32*1024*1024
    assert all(result['projectedSelection'][key] for key in ['selectionReadCancelled','selectionRetry','modeReadCancelled'])
    assert result['htmlActions']['ok'] and result['htmlActions']['contextMenuAction'] and result['htmlActions']['contextMenuPointerAnchor'] and result['htmlActions']['emptySelectionScoped']
    with sqlite3.connect('file:'+str(root/'config/codex-thread-studio/studio.sqlite3')+'?mode=ro',uri=True) as database:
        stored=json.loads(database.execute('SELECT drafts_json FROM session_annotation_drafts WHERE session_key=?',('codex:target',)).fetchone()[0])
    assert stored==result['htmlActions']['records'],'HTML reopen mutated the persistent record'
    assert (root/'project/actions.html').read_bytes()==(root/'html-original.bin').read_bytes(),'HTML actions wrote the document'
    calls=runtime.requests(root);starts=[r for r in calls if r.get('method')=='thread/start'];turns=runtime.starts(root)
    assert len(starts)==6 and len(turns)==5,(root,starts,turns)
    assert all(t['params']['threadId'].startswith('translation-') for t in turns),'Utility changed source conversation'
    html_turns=[t for t in turns if '<source_text>\nA & B bold 中文😀\n</source_text>' in t['params']['input'][0]['text']]
    assert len(html_turns)==1,'HTML selection was changed, translated twice, or not sent'
    projected=[t for t in turns if 'Flow translation\n' in t['params']['input'][0]['text']]
    assert len(projected)==1 and (root/'expected-flow-translation.txt').read_text().strip() in projected[0]['params']['input'][0]['text']
    assert '\ufffc' not in projected[0]['params']['input'][0]['text'],'Synthetic anchor entered the translation prompt'
    assert len([r for r in calls if r.get('method')=='thread/delete'])==6,'Owned tasks were not cleaned'
    oc=[json.loads(line) for line in (root/'opencode-requests.jsonl').read_text().splitlines()]
    prompts=[r for r in oc if r['path'].endswith('/prompt_async')]
    assert len(prompts)==1 and prompts[0]['path'].startswith('/session/oc-translation-')
    assert len([r for r in oc if r['method']=='DELETE' and r['path'].startswith('/session/oc-translation-')])==1
    local=[json.loads(line) for line in (root/'ollama-requests.jsonl').read_text().splitlines()];assert len([r for r in local if r['path'].endswith('/translate')])==1 and len([r for r in local if r['path'].endswith('/models')])==1
    assert not any(word in (root/'app.log').read_text() for word in ['CRITICAL','panicked','Gtk-WARNING']),(root,(root/'app.log').read_text()[-2000:])
    assert measured['gatewayPeakRssKiB']>0,'Owned gateway was not sampled'
    for name in ['backend.pid','opencode.pid','translation-gateway.pid','translation-proxy.pid']:
        if (root/name).exists():
            process=Path('/proc',(root/name).read_text().strip())
            runtime.until(lambda:not process.exists(),seconds=10)
    runtime.until(lambda:not memory.web_content_alive(),seconds=10)
    print(json.dumps(dict(result,**speech.verify(SOURCE),wireIdentity=True,ownedCleanup=True,localProxyOnly=True,memory=measured,artifacts=str(root))))

def run(native,gateway,root):
    path=Path(os.environ['XDG_RUNTIME_DIR'])/'studio-translation-speech.sock'
    with PrivateSpeech(root,path) as speech:
        env=dict(os.environ,SPEECHD_ADDRESS='unix_socket:'+str(path))
        return run_client(native,gateway,root,speech,env)

def documents(root):
    original='<html><body>\r\n<h1>HTML actions</h1>\r\n<p>Unique literal 中文😀  </p>\r\n<p>A &amp; B <strong>bold</strong> 中文&#x1F600;</p>\r\n<p>Duplicate passage</p><p>Duplicate passage</p>\r\n</body></html>'.encode()
    (root/'project/actions.html').write_bytes(original)
    (root/'html-original.bin').write_bytes(original)
    path=Path(__file__).with_name('smoke-gnome-pdf.py')
    spec=importlib.util.spec_from_file_location('translation_pdf_fixture',path)
    pdf=importlib.util.module_from_spec(spec);spec.loader.exec_module(pdf)
    pdf.document(root/'project/translation.pdf')
    # One atomic table exceeds the complete-selection budget; translation must
    # stream only its Unicode prefix, independent of the visible row page.
    row='| row 汉😀 | evidence '+'context '*18+'|\n'
    head='# Flow translation\n\nBefore 中文😀 $x^2$.\n\n| Header | Value |\n|---|---|\n'
    count=(32*1024*1024-len(head.encode()))//len(row.encode())+1
    with (root/'project/flow-translation.md').open('wb') as file:
        file.write(head.encode())
        chunk=row.encode()*1024
        for _ in range(count//1024):file.write(chunk)
        file.write(row.encode()*(count%1024))
    rendered='Flow translation\n\nBefore 中文😀 $x^2$.\n\nHeader\tValue'
    rendered+=('\nrow 汉😀\t'+('evidence '+'context '*18).rstrip())*200
    (root/'expected-flow-translation.txt').write_text(rendered[:16000])

    with zipfile.ZipFile(root/'project/translation.epub','w',compression=zipfile.ZIP_DEFLATED) as book:
        book.writestr('mimetype','application/epub+zip',compress_type=zipfile.ZIP_STORED)
        book.writestr('META-INF/container.xml','<container><rootfiles><rootfile full-path="package.opf"/></rootfiles></container>')
        book.writestr('package.opf','<package xmlns="http://www.idpf.org/2007/opf" version="3.0"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>Private translation book</dc:title><dc:language>en</dc:language></metadata><manifest><item id="body" href="body.xhtml" media-type="application/xhtml+xml"/><item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/></manifest><spine><itemref idref="body"/></spine></package>')
        book.writestr('body.xhtml','<html xmlns="http://www.w3.org/1999/xhtml"><head><title>Translation</title></head><body><h1>Private translation book</h1><p id="translation">'+html.escape(SOURCE)+'</p></body></html>')
        book.writestr('nav.xhtml','<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><body><nav epub:type="toc"><ol><li><a href="body.xhtml">Translation</a></li></ol></nav></body></html>')

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('native');parser.add_argument('gateway');parser.add_argument('--api-only',action='store_true');parser.add_argument('--private-root');args=parser.parse_args();resource.setrlimit(resource.RLIMIT_CORE,(0,0))
    native,gateway=str(Path(args.native).resolve()),str(Path(args.gateway).resolve())
    if args.private_root:return run(native,gateway,Path(args.private_root))
    base=Path(os.environ.get('STUDIO_NATIVE_QA_DIR') or Path(__file__).resolve().parents[1]/'target/gnome-native-smoke');base.mkdir(exist_ok=True,parents=True);root=Path(tempfile.mkdtemp(prefix='translation-',dir=base));runtime.create_profile(root);api_checks(gateway,root)
    if args.api_only:print(json.dumps(dict(ok=True,scopedApi=True,authOrigin=True,artifacts=str(root))));return
    documents(root)
    for name,source in [('fake-codex.py',codex_source()),('fake-opencode.py',opencode_source())]:path=root/name;path.write_text(source);path.chmod(0o700)
    (root/'config/codex-thread-studio/gnome.json').write_text(json.dumps(dict(selectedBackend='codex',selectedSession='target')))
    wrapper=root/'gateway-proxy.py';wrapper.write_text('#!/usr/bin/python3\nimport sys\nsys.dont_write_bytecode=True\nsys.path.insert(0,'+repr(str(Path(__file__).with_name('test-support').resolve()))+')\nfrom private_ollama_proxy import main\nraise SystemExit(main('+repr(gateway)+',sys.argv[1:]))\n');wrapper.chmod(0o700)
    env=dict(os.environ,CODEX_THREAD_STUDIO_GATEWAY_BIN=str(wrapper),CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),CODEX_HOME=str(root/'codex'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),READER_CACHE_DIR=str(root/'reader-cache'),GDK_BACKEND='wayland',STUDIO_GNOME_SMOKE_DIR=str(root),STUDIO_GNOME_SMOKE_TRANSLATION='1',STUDIO_OWNED_MUTTER_INPUT='1')
    with isolated_environment(env,headless=True) as private:
        subprocess.run(['dbus-run-session','--',shutil.which('mutter') or 'mutter','--headless','--wayland','--no-x11','--wayland-display','studio-test','--virtual-monitor','1600x1000','--',sys.executable,str(Path(__file__).resolve()),native,gateway,'--private-root',str(root)],cwd=root,env=private,check=True,timeout=210)
if __name__=='__main__':main()
