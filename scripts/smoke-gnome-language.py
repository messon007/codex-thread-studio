#!/usr/bin/env python3
"""Scoped shared language API and live native UI/data preservation on private Wayland."""
import argparse,importlib.util,json,os,resource,shutil,sqlite3,subprocess,sys,tempfile,time
from pathlib import Path
from urllib.request import Request
from urllib.error import HTTPError
sys.dont_write_bytecode=True
sys.path.insert(0,str(Path(__file__).with_name('test-support')))
from private_wayland import isolated_environment,OwnedMutterInput
from memory_sampler import ProcessMemory
spec=importlib.util.spec_from_file_location('language_runtime',Path(__file__).with_name('smoke-gnome-router-runtime.py'))
runtime=importlib.util.module_from_spec(spec);spec.loader.exec_module(runtime)

def api_checks(binary,root,queue=False):
    runtime.fixture.BACKEND=runtime.CODEX;runtime.fixture.OPENCODE_BACKEND=runtime.OPENCODE
    api=runtime.fixture.Gateway(Path(binary),root,'codex');endpoint='/studio/native/language'
    try:
        assert api.api(endpoint,authorized=False)[0]==401
        assert api.api(endpoint,{},authorized=False,method='PATCH')[0]==401
        raw=api.preferences();raw['language']='en-US';raw['futureLanguageTest']={'keep':['中文',1]};api.settings.write_text(json.dumps(raw))
        status,base=api.api(endpoint);assert status==200 and base['language']=='en-US'
        current=api.preferences();current['concurrentUnrelated']={'keep':True};api.settings.write_text(json.dumps(current))
        status,saved=api.api(endpoint,dict(expectedRevision=base['revision'],language='zh-CN'),method='PATCH')
        assert status==200 and saved['language']=='zh-CN';current['language']='zh-CN';assert api.preferences()==current
        assert api.api(endpoint,dict(expectedRevision=base['revision'],language='system'),method='PATCH')[0]==409
        assert api.preferences()==current
        for body in [dict(expectedRevision=saved['revision'],language='invalid'),dict(expectedRevision=saved['revision'],language=42),dict(expectedRevision=saved['revision'],language='en-US',extra=True)]:
            assert api.api(endpoint,body,method='PATCH')[0]==400
            assert api.preferences()==current
        assert api.api(endpoint,dict(expectedRevision='f'*9000,language='en-US'),method='PATCH')[0] in (400,413)
        assert api.preferences()==current
        headers={'Authorization':'Bearer '+api.token,'Origin':'https://example.invalid','Content-Type':'application/json'}
        try:api.client.open(Request(api.origin+endpoint,headers=headers),timeout=10);raise AssertionError('Foreign origin accepted')
        except HTTPError as error:assert error.code==403
        assert api.api(endpoint,dict(expectedRevision=saved['revision'],language='en-US'),method='PATCH')[0]==200
        if queue:
            message=dict(id='language-queue',text='Pause',input=[dict(type='skill',name='Send',path=str(root/'project/language.md'),future={'literal':'Pause'})],createdAt=123)
            assert api.api('/studio/session-state/message-queue',dict(sessionKey='codex:target',messages=[message]),method='PUT')[0]==204
        assert not runtime.starts(root)
    finally:api.close()

def child(native,gateway,root):
    input=OwnedMutterInput();results=[]
    try:
        for phase in ['live','restart']:
            env=dict(os.environ,STUDIO_GNOME_SMOKE_LANGUAGE=phase)
            with (root/f'{phase}.log').open('w') as log:
                app=subprocess.Popen([native],cwd=root,env=env,stdout=log,stderr=log);sampler=ProcessMemory(app.pid,gateway)
                try:
                    until=time.monotonic()+100
                    while app.poll() is None:
                        input.respond(root)
                        if time.monotonic()>until:raise TimeoutError((phase,root))
                        time.sleep(.015)
                except BaseException:app.terminate();app.wait(timeout=10);raise
                finally:memory=sampler.close()
            result=json.loads((root/'language-result.json').read_text());assert app.returncode==0 and result['ok'],(phase,root,result)
            text=(root/f'{phase}.log').read_text();assert not any(x in text for x in ['CRITICAL','panicked','Gtk-WARNING']),(phase,text[-1500:])
            results.append(dict(phase=phase,checks=result,memory=memory));(root/'language-result.json').rename(root/f'{phase}.json')
            for name in ['backend.pid','opencode.pid']:
                if (root/name).exists():assert not Path('/proc',(root/name).read_text().strip()).exists(),'Owned provider survived'
        raw=json.loads((root/'config/codex-thread-studio/settings.json').read_text())
        assert raw['language']=='zh-CN' and raw['futureLanguageTest']=={'keep':['中文',1]} and raw['concurrentClient']=={'keep':True}
        assert not runtime.starts(root),'Language controls started a model turn'
        with sqlite3.connect(root/'config/codex-thread-studio/studio.sqlite3') as database:
            queue=json.loads(database.execute('SELECT queue_json FROM session_message_queues WHERE session_key=?',('codex:target',)).fetchone()[0])
            favorite=database.execute("SELECT title,note,tags_json,content,backend,thread_id,turn_id,item_id FROM favorites WHERE id='favorite-language-probe'").fetchone()
            comments=json.loads(database.execute('SELECT drafts_json FROM session_annotation_drafts WHERE session_key=?',('codex:target',)).fetchone()[0])
            instructions=database.execute('SELECT content FROM session_annotation_additional WHERE session_key=?',('codex:target',)).fetchone()[0]
        assert favorite==('Send','Note Save <b>Settings</b> 中文😀 {count}\n','["Save","Settings"]','# Save\r\n\r\nSource Settings 中文😀 {count}\r\n','codex','target','favorite-language-turn','favorite-language-answer'),('Live locale/save changed literal favorite data',favorite)
        expected=dict(id='language-queue',text='Send {count} 中文😀',input=[dict(type='skill',name='Send',path=str(root/'project/language.md'),future={'literal':'Pause'})],createdAt=123)
        assert queue==[expected],('Queue edit changed identity/reference content',queue)
        assert results[0]['checks']['favoritesLanguage'] and results[0]['checks']['favoriteDraftAndPromptPreserved']
        assert len(comments)==1 and comments[0]['excerpt']=='Select text in a document and choose Comment to add it here.' and comments[0]['note']=='Saved comments <b>Source</b> 中文😀 {value}\n'
        assert comments[0]['source']==dict(provider='chat',version=1,anchor=dict(sessionKey='codex:target',turnId='Saved comments',itemId='line {value}<b>Source</b>',future=dict(literal='Comments')))
        assert instructions=='Keep comments and close / Saved comments'
        assert results[0]['checks']['commentsLanguage'] and results[0]['checks']['commentLocationParts'] and results[0]['checks']['commentPromptAndDraftPreserved']
        print(json.dumps(dict(ok=True,languageApi=True,phases=results,commentsExplicitSaveAndRestart=True,favoriteExplicitSaveAndRestart=True,queueExplicitSaveAndRestart=True,unknownPreferencesPreserved=True,artifacts=str(root))))
    finally:input.close()

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('native');parser.add_argument('gateway');parser.add_argument('--api-only',action='store_true');parser.add_argument('--private-root');args=parser.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE,(0,0));native,gateway=str(Path(args.native).resolve()),str(Path(args.gateway).resolve())
    if args.private_root:return child(native,gateway,Path(args.private_root))
    base=Path(__file__).resolve().parents[1]/'target/gnome-native-smoke';base.mkdir(parents=True,exist_ok=True);root=Path(tempfile.mkdtemp(prefix='language-',dir=base));runtime.create_profile(root)
    api_checks(gateway,root,queue=not args.api_only)
    if args.api_only:print(json.dumps(dict(ok=True,languageApi=True,artifacts=str(root))));return
    (root/'project/language.md').write_bytes(b'# Language document\r\n\r\nSend Settings Stop\r\n')
    code=runtime.CODEX.replace("    elif method=='thread/read':","    elif method=='model/list':answer(identity,{'data':[]})\n    elif method=='thread/items/list':answer(identity,{'data':[],'nextCursor':None})\n    elif method=='thread/read':")
    for name,value in [('fake-codex.py',code),('fake-opencode.py',runtime.OPENCODE)]:path=root/name;path.write_text(value);path.chmod(0o700)
    (root/'config/codex-thread-studio/gnome.json').write_text(json.dumps(dict(selectedBackend='codex',selectedSession='target')))
    env=dict(os.environ,CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway,CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),CODEX_HOME=str(root/'codex'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),READER_CACHE_DIR=str(root/'reader-cache'),GDK_BACKEND='wayland',GIO_USE_VFS='local',GIO_USE_VOLUME_MONITOR='unix',STUDIO_GNOME_SMOKE_DIR=str(root),STUDIO_OWNED_MUTTER_INPUT='1')
    with isolated_environment(env,headless=True) as private:
        subprocess.run(['dbus-run-session','--',shutil.which('mutter') or 'mutter','--headless','--wayland','--no-x11','--wayland-display','studio-test','--virtual-monitor','1600x1000','--',sys.executable,str(Path(__file__).resolve()),native,gateway,'--private-root',str(root)],cwd=root,env=private,check=True,timeout=230)
if __name__=='__main__':main()
