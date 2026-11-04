#!/usr/bin/env python3
"""Shared comment CAS API and native selection/edit/restart acceptance; private providers only."""
import argparse
import csv
import http.server
import importlib.util
import json
import os
from pathlib import Path
import resource
import shutil
import sqlite3
import threading
import subprocess
import sys
import tempfile
import time
import zipfile
from urllib.request import Request
from urllib.error import HTTPError
sys.dont_write_bytecode=True
spec=importlib.util.spec_from_file_location('annotations_fixture',Path(__file__).with_name('smoke-gnome-router-runtime.py'))
runtime=importlib.util.module_from_spec(spec);spec.loader.exec_module(runtime)
sys.path.insert(0,str(Path(__file__).with_name('test-support')))
from memory_sampler import ProcessMemory
from private_wayland import isolated_environment

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('native');p.add_argument('gateway',nargs='?');p.add_argument('--api-only',action='store_true');p.add_argument('--submit',choices=['codex','opencode','router','reject','cleanup','conflict','cleanup-close']);p.add_argument('--source',choices=['markdown','epub','browser','pdf','table'],default='markdown');p.add_argument('--table-kind',choices=['csv','xlsx','sql'],default='csv');p.add_argument('--language',choices=['en-US','zh-CN'],default='en-US');p.add_argument('--headless',action='store_true');args=p.parse_args()
    if not args.gateway and not args.api_only:p.error('both executables are required for Wayland checks')
    if args.submit and args.source!='markdown':p.error('submission fixture uses the Markdown source')
    native=str(Path(args.native).resolve());gateway=str(Path(args.gateway or args.native).resolve())
    resource.setrlimit(resource.RLIMIT_CORE,(0,0))
    if args.headless and not args.api_only:
        command=[sys.executable,str(Path(__file__).resolve()),native,gateway,'--language',args.language,'--source',args.source,'--table-kind',args.table_kind]
        if args.submit:command+=['--submit',args.submit]
        with tempfile.TemporaryDirectory(prefix='studio-comments-desktop-',dir='/tmp') as directory:
            private_base=Path(directory)
            env=dict(os.environ,GDK_BACKEND='wayland',XDG_CONFIG_HOME=str(private_base/'config'),XDG_DATA_HOME=str(private_base/'data'),XDG_CACHE_HOME=str(private_base/'cache'))
            with isolated_environment(env,headless=True) as private:
                subprocess.run(['dbus-run-session','--',shutil.which('mutter') or 'mutter','--headless','--wayland','--no-x11','--wayland-display','studio-test','--virtual-monitor','1600x1000','--',*command],env=private,check=True,timeout=600)
        return
    base=Path(__file__).resolve().parents[1]/'target/gnome-native-smoke';base.mkdir(parents=True,exist_ok=True);root=Path(tempfile.mkdtemp(prefix='annotations-ui-',dir=base));runtime.create_profile(root)
    (root/'project/annotation.md').write_text('# Native comments\n\nSource **中文😀** selection.\n\nAnother paragraph.\n')
    runtime.fixture.BACKEND=runtime.CODEX;runtime.fixture.OPENCODE_BACKEND=runtime.OPENCODE
    service=runtime.fixture.Gateway(Path(gateway),root,'codex')
    preferences=service.preferences();preferences['language']=args.language;service.settings.write_text(json.dumps(preferences))
    def api(key,body=None,auth=True,status=200,consume=False):
        headers={'Content-Type':'application/json'}
        if auth:headers['Authorization']='Bearer '+service.token
        path='/studio/session-state/annotations'+('/consume' if consume else '' if body is not None else '?sessionKey='+key)
        req=Request(service.origin+path,data=None if body is None else json.dumps(dict(body,sessionKey=key)).encode(),headers=headers,method='GET' if body is None else 'POST' if consume else 'PUT')
        try:r=service.client.open(req,timeout=20)
        except HTTPError as error:r=error
        with r:
            value=r.read();assert r.status==status,(r.status,status,value[:300]);return json.loads(value) if value else None
    record=dict(id='opaque-existing',excerpt='Legacy foreign source',note='existing',createdAt='2026-10-24T00:00:00Z',source=dict(provider='future',version=7,anchor={'custom':['中文',42]}))
    try:
        api('codex:api-one',auth=False,status=401);api('invalid',status=400)
        empty=api('codex:api-one');assert empty['drafts']==[] and len(empty['revision'])==64
        first=api('codex:api-one',dict(drafts=[record],additional='first',expectedRevision=empty['revision']))
        assert first['drafts'][0]['source']==record['source'] and first['revision']!=empty['revision']
        api('codex:api-one',dict(drafts=[],additional='',expectedRevision=empty['revision']),status=409)
        other=api('codex:api-two');assert other['revision']==empty['revision']
        api('codex:api-one',dict(drafts=[dict(record,note='newer browser note')],additional='legacy writer'),status=204)
        api('codex:api-one',dict(drafts=[],expectedRevision=first['revision']),status=409)
        newer=api('codex:api-one');assert newer['drafts'][0]['note']=='newer browser note'
        cleared=api('codex:api-one',dict(drafts=[],additional='',expectedRevision=newer['revision']));assert cleared['revision']==empty['revision']
        api('codex:consume',dict(drafts=[record,dict(record,id='changed',note='old')],additional='old'),status=204)
        api('codex:consume',dict(drafts=[record,dict(record,id='changed',note='new'),dict(record,id='new')],additional='new'),status=204)
        body=dict(drafts=[record,dict(record,id='changed',note='old')],additional='old')
        api('codex:consume',body,auth=False,status=401,consume=True)
        cleaned=api('codex:consume',body,consume=True);assert len(cleaned['drafts'])==2 and cleaned['additional']=='new'
        assert api('codex:consume',body,consume=True)['revision']==cleaned['revision']
        for owner in ['codex:target','opencode:oc-target','codex:router']:
            api(owner,dict(drafts=[record],additional=''),status=204)
    finally:service.close()
    if args.api_only:
        print(json.dumps(dict(ok=True,annotationApi=True,staleSave=True,legacyWriter=True,sourcePreservation=True,artifacts=str(root))));return
    requests=[];server=None
    url=''
    if args.source=='table':
        with (root/'project/comments.csv').open('w',newline='') as stream:
            writer=csv.writer(stream,delimiter=';');writer.writerow(['name','score','note'])
            for n in range(80):writer.writerow(['needle' if n==55 else f'row-{n}',n,'Cell passage 中文😀 '+('multiline, quoted; "note"\n'*180)+'完整结束' if n==55 else 'Other cell value'])
        spec=importlib.util.spec_from_file_location('comment_table_fixture',Path(__file__).with_name('smoke-gnome-tables.py'))
        table=importlib.util.module_from_spec(spec);spec.loader.exec_module(table)
        table.workbook(root/'project/book.xlsx')
        path=root/'project/book.xlsx';updated=path.with_suffix('.updated.xlsx')
        with zipfile.ZipFile(path) as source,zipfile.ZipFile(updated,'w',compression=zipfile.ZIP_DEFLATED) as destination:
            for info in source.infolist():destination.writestr(info,source.read(info).replace(b'Second worksheet','Second worksheet 中文😀'.encode()) if info.filename=='xl/worksheets/sheet2.xml' else source.read(info))
        updated.replace(path)
    if args.source=='pdf':
        spec=importlib.util.spec_from_file_location('comment_pdf_fixture',Path(__file__).with_name('smoke-gnome-pdf.py'))
        pdf=importlib.util.module_from_spec(spec);spec.loader.exec_module(pdf)
        pdf.document(root/'project/sample.pdf')
    if args.source in ['epub','browser']:
        class Source(http.server.BaseHTTPRequestHandler):
            def do_GET(self):
                requests.append(self.path)
                content='<html><head><meta charset="utf-8"><title>Private comment source</title></head><body><h1>Browser comments</h1><p id="quote">Browser excerpt 中文😀</p></body></html>'.encode()
                self.send_response(200);self.send_header('Content-Type','text/html; charset=utf-8');self.send_header('Content-Length',str(len(content)));self.end_headers();self.wfile.write(content)
            def log_message(self,*args):pass
        server=http.server.ThreadingHTTPServer(('127.0.0.1',0),Source)
        threading.Thread(target=server.serve_forever,daemon=True).start()
        url=f'http://127.0.0.1:{server.server_port}/source'
        if args.source=='epub':
            spec=importlib.util.spec_from_file_location('comment_epub_fixture',Path(__file__).with_name('smoke-gnome-epub.py'))
            epub=importlib.util.module_from_spec(spec);spec.loader.exec_module(epub)
            epub.book(root/'project/sample.epub',url)
    backend=runtime.CODEX.replace("assert params['excludeTurns'] is True",'pass').replace("assert params['includeTurns'] is False",'pass').replace("    elif method=='turn/start':","    elif method=='model/list':answer(identity,{'data':[]})\n    elif method=='turn/start':")
    backend=backend.replace("    elif method=='thread/turns/list':", """    elif method=='thread/items/list':
        assert params['limit']==120 and params['sortDirection']=='desc' and params.get('cursor') is None
        with lock:items=[{'turnId':turn['id'],'item':item} for turn in reversed(turns.get(session,[])) for item in reversed(turn.get('items',[]))]
        answer(identity,{'data':items[:120],'nextCursor':None})
    elif method=='thread/turns/list':""")
    backend=backend.replace("    elif method=='turn/start':\n        prompt=", "    elif method=='turn/start':\n        if session!='router':(root/'comment-model-request').touch()\n        if session!='router' and (root/'reject-comment-submit').exists():answer(identity,error='Private rejected comment batch');continue\n        prompt=")
    backend=backend.replace("            time.sleep(.1)\n", "            while session!='router' and (root/'hold-comment-ack').exists():time.sleep(.02)\n            time.sleep(.1)\n")
    opencode=runtime.OPENCODE.replace("        identity=path.split('/')[2]", "        if path=='/config/providers':return self.response({'providers':[{'id':'provider','models':{'oc-target-model':{}}}]})\n        if path=='/global/event':\n            self.send_response(200);self.send_header('Content-Type','text/event-stream');self.end_headers()\n            try:\n                while True:self.wfile.write(b': heartbeat\\n\\n');self.wfile.flush();time.sleep(.2)\n            except (BrokenPipeError,ConnectionResetError):pass\n            return\n        identity=path.split('/')[2]")
    opencode=opencode.replace("        if path.endswith('/prompt_async'):\n", "        if path.endswith('/prompt_async'):\n            (root/'comment-model-request').touch()\n            while (root/'hold-comment-ack').exists():time.sleep(.02)\n")
    for name,source in [('fake-codex.py',backend),('fake-opencode.py',opencode)]:
        f=root/name;f.write_text(source);f.chmod(0o700)
    (root/'config/codex-thread-studio/gnome.json').write_text(json.dumps(dict(selectedBackend='opencode' if args.submit=='opencode' else 'codex',selectedSession='oc-target' if args.submit=='opencode' else 'router' if args.submit=='router' else 'target')))
    environment=dict(os.environ,CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway,CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),CODEX_HOME=str(root/'codex'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),READER_CACHE_DIR=str(root/'reader-cache'),GDK_BACKEND='wayland',STUDIO_GNOME_SMOKE_DIR=str(root),STUDIO_GNOME_SMOKE_ANNOTATIONS='1')
    environment['CODEX_THREAD_STUDIO_BACKENDS_CONFIG']=str(root/'config/codex-thread-studio/backends.json')
    environment.pop('DISPLAY',None);environment.pop('XAUTHORITY',None)
    if args.submit:
        environment['STUDIO_GNOME_SMOKE_COMMENT_SUBMIT']=args.submit
        if args.submit=='reject':(root/'reject-comment-submit').touch()
    if args.source!='markdown':
        environment['STUDIO_GNOME_SMOKE_COMMENT_SOURCE']=args.source
        environment['STUDIO_GNOME_SMOKE_COMMENT_URL']=url
        environment['STUDIO_GNOME_SMOKE_TABLE_KIND']=args.table_kind
    results=[];memory=[];original_table_anchor=None
    phases=[(False,'')] if args.submit else [(False,''),(True,'')]
    if args.source in ['pdf','table']:phases.append((True,'legacy'))
    if args.source in ['epub','pdf','table']:phases.extend([(True,'mismatch'),(True,'changed')])
    if args.source=='table' and args.table_kind=='sql':phases.insert(-1,(True,'query-error'))
    for restore,variant in phases:
        env=dict(environment)
        if restore:env['STUDIO_GNOME_SMOKE_ANNOTATIONS_RESTORE']='1'
        if variant:
            env['STUDIO_GNOME_SMOKE_SOURCE_VARIANT']=variant
            if variant in ['mismatch','legacy','query-error']:
                with sqlite3.connect(root/'config/codex-thread-studio/studio.sqlite3') as database:
                    records=json.loads(database.execute('SELECT drafts_json FROM session_annotation_drafts WHERE session_key=?',('codex:target',)).fetchone()[0])
                    if args.source=='table':
                        if original_table_anchor is None:original_table_anchor=dict(records[1]['source']['anchor'])
                        else:records[1]['source']['anchor']=dict(original_table_anchor)
                    if variant=='query-error':records[1]['source']['anchor']['sql']='DELETE FROM data'
                    elif variant=='legacy':
                        records[1]['source']['anchor'].pop('charRange',None)
                        if args.source=='pdf':records[1]['source']['anchor']['rects']*=128
                        else:
                            for key in ['sql','header','delimiter']:records[1]['source']['anchor'].pop(key,None)
                            if args.table_kind=='sql':records[1]['source']['anchor']['sheet']='Query'
                    elif args.source=='epub':records[1]['source']['anchor']['cfiRange']='epubcfi(/6/4!/4/2,/1:0,/1:6)'
                    elif args.source=='table':
                        if args.table_kind=='sql':records[1]['source']['anchor']['sql']="SELECT 'Other cell value' AS quoted"
                        else:records[1]['source']['anchor']['range']='A1' if args.table_kind=='csv' else 'B1'
                    else:records[1]['source']['anchor']['charRange']=[5,6]
                    database.execute('UPDATE session_annotation_drafts SET drafts_json=? WHERE session_key=?',(json.dumps(records),'codex:target'))
            elif args.source=='table':
                with sqlite3.connect(root/'config/codex-thread-studio/studio.sqlite3') as database:
                    records=json.loads(database.execute('SELECT drafts_json FROM session_annotation_drafts WHERE session_key=?',('codex:target',)).fetchone()[0])
                    records[1]['source']['anchor']=original_table_anchor
                    database.execute('UPDATE session_annotation_drafts SET drafts_json=? WHERE session_key=?',(json.dumps(records),'codex:target'))
                if args.table_kind=='xlsx':
                    with (root/'project/book.xlsx').open('ab') as document:document.write(b'Changed private workbook revision')
                else:
                    with (root/'project/comments.csv').open('a') as document:document.write('new;999;Changed table revision\n')
            elif args.source=='pdf':
                with (root/'project/sample.pdf').open('ab') as document:document.write(b'\n% Changed private PDF revision\n')
            else:
                path=root/'project/sample.epub';updated=path.with_suffix('.updated.epub')
                with zipfile.ZipFile(path) as source,zipfile.ZipFile(updated,'w',compression=zipfile.ZIP_DEFLATED) as destination:
                    for info in source.infolist():
                        content=source.read(info)
                        if info.filename=='OPS/Text/two.xhtml':content=content.replace(b'Paragraph 15.',b'Changed paragraph 15.')
                        destination.writestr(info,content)
                updated.replace(path)
        phase=variant or ('restore' if restore else 'app')
        with (root/(phase+'.log')).open('w') as log:
            child=subprocess.Popen([native],cwd=root,env=env,stdout=log,stderr=log);observer=ProcessMemory(child.pid,gateway,include_web_content=True)
            stop=threading.Event();errors=[]
            def external_writer():
                if restore:return
                flag='comment-submit-edit-ready' if args.submit else 'comment-conflict-ready'
                done='comment-submit-edit-written' if args.submit else 'comment-conflict-written'
                key='opencode:oc-target' if args.submit=='opencode' else 'codex:router' if args.submit=='router' else 'codex:target'
                while not stop.wait(.02):
                    if (root/flag).exists():
                        try:
                            with sqlite3.connect(root/'config/codex-thread-studio/studio.sqlite3',timeout=5) as database:
                                database.execute('BEGIN IMMEDIATE')
                                records=json.loads(database.execute('SELECT drafts_json FROM session_annotation_drafts WHERE session_key=?',(key,)).fetchone()[0])
                                records[0]['note']='newer external writer'
                                if args.submit=='conflict':records[1]['note']='server changed the same comment'
                                if args.submit:records.append(dict(record,id='server-new',note='new server comment'))
                                database.execute('UPDATE session_annotation_drafts SET drafts_json=? WHERE session_key=?',(json.dumps(records),key))
                            (root/done).write_text('done')
                        except Exception as error:errors.append(str(error))
                        return
            writer=threading.Thread(target=external_writer,daemon=True);writer.start()
            try:child.wait(timeout=100)
            except subprocess.TimeoutExpired:child.terminate();child.wait(timeout=10);raise AssertionError(('Annotations timed out',root))
            finally:stop.set();writer.join(timeout=1);memory.append(observer.close())
            assert not errors,(root,errors)
        assert child.returncode==0,(root,child.returncode)
        results.append(json.loads((root/'annotations-ui.json').read_text()))
        (root/(phase+'-result.json')).write_text(json.dumps(results[-1],indent=2))
        for name in ['annotations.png','comment-source.png']:
            if (root/name).exists():shutil.copyfile(root/name,root/(phase+'-'+name))
        for name in ('backend.pid','opencode.pid'):
            f=root/name
            if f.exists():
                pid=f.read_text().strip()
                for _ in range(100):
                    if not Path('/proc',pid).exists():break
                    time.sleep(.05)
                assert not Path('/proc',pid).exists(),('Owned backend survived',root,pid)
        assert not observer.web_content_alive(),('Unexpected content engine remained',root)
        if not results[-1]['ok']:break
    if args.submit:
        if args.submit=='opencode':
            sends=[json.loads(line) for line in (root/'opencode-requests.jsonl').read_text().splitlines() if json.loads(line)['path'].endswith('/prompt_async')]
            assert len(sends)==1,(root,sends)
            prompt=sends[0]['body']['parts'][0]['text']
            assert 'Sent comment 中文😀' in prompt and 'New local note after send' not in prompt and prompt.count('Please address every comment')==1
        else:
            starts=runtime.starts(root);expected=2 if args.submit=='router' else 1
            assert len(starts)==expected,(root,starts)
            assert any('Sent comment 中文😀' in r['params']['input'][0]['text'] for r in starts)
            assert all('New local note after send' not in r['params']['input'][0]['text'] and r['params']['input'][0]['text'].count('Please address every comment')==1 for r in starts)
    else:assert not runtime.starts(root),'Comment insertion sent a model turn'
    if server:server.shutdown();server.server_close()
    if args.source=='epub':assert not requests,('EPUB made remote content requests',root,requests)
    if args.source=='browser':assert '/source' in requests,(root,requests)
    print(json.dumps(dict(ok=all(r['ok'] for r in results),annotationApi=True,language=args.language,controls=results,observedMemory=memory,artifacts=str(root))))
    assert len(results)==len(phases) and all(r['ok'] for r in results),(root,results)

if __name__=='__main__':main()
