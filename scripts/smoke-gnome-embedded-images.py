#!/usr/bin/env python3
"""Embedded Markdown PNG/SVG, real GTK viewer/Save and no remote fetching."""
import argparse, base64, hashlib, http.server, importlib.util, json, os, random, resource, shutil, struct, subprocess, sys, tempfile, threading, time, zlib
from pathlib import Path
from urllib.parse import quote
sys.dont_write_bytecode=True
sys.path.insert(0,str(Path(__file__).with_name('test-support')))
from private_wayland import isolated_environment, OwnedMutterInput
from private_file_portal import FilePortal
from memory_sampler import ProcessMemory
spec=importlib.util.spec_from_file_location('embedded_outputs',Path(__file__).with_name('smoke-gnome-outputs.py'))
outputs=importlib.util.module_from_spec(spec);spec.loader.exec_module(outputs)

def png():
    def chunk(kind,data): return struct.pack('>I',len(data))+kind+data+struct.pack('>I',zlib.crc32(kind+data))
    raw=random.Random(42).randbytes(128*128*4)
    rows=b''.join(b'\0'+raw[i:i+512] for i in range(0,len(raw),512))
    return b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>2I5B',128,128,8,6,0,0,0))+chunk(b'IDAT',zlib.compress(rows))+chunk(b'IEND',b'')

def verify_projection_api(gateway,root,body,png_bytes):
    from urllib.request import Request
    from urllib.error import HTTPError
    spec=importlib.util.spec_from_file_location('projected_image_api',Path(__file__).with_name('smoke-gnome-images.py'))
    fixture=importlib.util.module_from_spec(spec);spec.loader.exec_module(fixture)
    api_root=root/'api-profile';fixture.runtime.create_profile(api_root)
    fixture.runtime.fixture.BACKEND=fixture.runtime.CODEX;fixture.runtime.fixture.OPENCODE_BACKEND=fixture.runtime.OPENCODE
    service=fixture.runtime.fixture.Gateway(Path(gateway),api_root,'codex')
    def call(method,path,data=None,auth=True,extra=None):
        headers={'Authorization':'Bearer '+service.token} if auth else {}
        if data is not None:headers['Content-Type']='application/json'
        request=Request(service.origin+path,data=None if data is None else json.dumps(data).encode(),method=method,headers=dict(headers,**(extra or {})))
        try:
            with service.client.open(request,timeout=40) as r:return r.status,r.read(),dict(r.headers)
        except HTTPError as e:return e.code,e.read(),dict(e.headers)
    def value(method,path,data=None):
        status,raw,_=call(method,path,data);assert status==200,(root,path,status,raw[:400]);return json.loads(raw)
    try:
        opened_at=time.monotonic()
        opened=value('POST','/studio/native/markdown/open',dict(root=str(root/'project'),path=str(root/'project/embedded.md')))
        reader=opened['readerId'];base='/studio/native/markdown/'+reader
        deadline=time.monotonic()+25
        while True:
            document=value('GET',base+'/document')
            if document['state']=='ready':break
            assert document['state']=='indexing' and time.monotonic()<deadline,(root,document.get('error'))
            time.sleep(.05)
        index_ready_ms=round((time.monotonic()-opened_at)*1000)
        index=document['index'];revision=index['revision']
        assert revision==hashlib.sha256(body.encode()).hexdigest()
        target=next(i for i,p in enumerate(index['pages']) if p['start']<=body.encode().index(b'![Embedded PNG')<p['end'])
        fragment_at=time.monotonic()
        status,raw,_=call('GET',base+f'/fragment?target={target}&revision={revision}')
        fragment_ms=round((time.monotonic()-fragment_at)*1000)
        assert status==200,(root,status,raw[:400])
        projected=json.loads(raw);assert len(raw)<10000 and projected['source']['projection'],(root,len(raw))
        descriptor=projected['source']['projection'][0];page=descriptor['page'];image=descriptor['image']
        assert projected['documentId']==document['documentId']
        assert descriptor['line']==3 and descriptor['endLine']==3
        assert 'data:image/png;base64,' not in projected['source']['text']
        image_path=base+f'/image?page={page}&image={image}&revision={revision}'
        status,pixels,headers=call('GET',image_path)
        assert status==200 and pixels[:8]==b'STIMG001' and len(pixels)==24+128*128*4,(root,status,len(pixels))
        assert headers['x-studio-source-sha256']==hashlib.sha256(png_bytes).hexdigest()
        status,original,_=call('GET',image_path+'&original=true')
        assert status==200 and original[:8]==b'STIMS001'
        count=struct.unpack('<I',original[8:12])[0]
        assert count==len(png_bytes) and original[12:12+count]==png_bytes and original[12+count:]==pixels
        assert call('GET',image_path,auth=False)[0]==401
        assert call('GET',image_path,extra={'Origin':'https://example.invalid'})[0]==403
        assert call('GET',base+f'/image?page={page}&image={image+1}&revision={revision}')[0]==400
        assert call('GET',base+f'/image?page={page}&image=0&revision='+('0'*64))[0]==409
        assert call('GET',base+f'/fragment?target={target}&revision={revision}',extra={'x-reader-document':'wrong-document'})[0]==409
        source=value('GET',base+'/source?line=3')['result']
        assert source['text'].startswith('![Embedded PNG 中文😀]') and 'studio-doc-image:' not in source['text']
        if (root/'reference-source').exists():
            assert source['text'].startswith('![Embedded PNG 中文😀][picture]')
            assert body.encode().index(b'\n[picture]:')+1>=projected['source']['end']
            definitions={d['label']:d for d in projected['source']['references']}
            assert definitions['picture']['destination']=='data:,' and definitions['guide']['destination']=='guide.md:5'
            assert len(raw)<10000 and not any('base64,' in d['destination'] for d in definitions.values())
        assert (root/'project/embedded.md').read_text()==body
        located=value('POST',base+'/locate-quote',dict(revision=revision,query=dict(quote='Embedded PNG 中文😀',anchor=dict(baseHash=revision,startLine=3,endLine=3,textKind='preview'))))['located']
        assert located['status']=='unique' and located['position']['line']==3 and located['position']['endLine']==3,(root,located)
        if (root/'reference-source').exists():
            table_body='| [Guide][guide] | Evidence |\n|---|---|\n'+''.join('| Row '+str(n)+' | [details][guide] '+'wide evidence '*8+' |\n' for n in range(200))+'\n[guide]: table-guide.md:9\n'
            (root/'project/reference-table.md').write_text(table_body)
            table_id=value('POST','/studio/native/markdown/open',dict(root=str(root/'project'),path=str(root/'project/reference-table.md')))['readerId']
            table_base='/studio/native/markdown/'+table_id
            deadline=time.monotonic()+25
            while True:
                table_document=value('GET',table_base+'/document')
                if table_document['state']=='ready':break
                assert table_document['state']=='indexing' and time.monotonic()<deadline,(root,table_document.get('error'))
                time.sleep(.05)
            table_revision=table_document['index']['revision']
            table=value('GET',table_base+'/table-source?id=0&row=150&columns=0,1&revision='+table_revision)
            assert table['rowStart']<=150<table['rowEnd'] and table['fragments']
            assert any(f['header'] for f in table['fragments'])
            for f in table['fragments']:
                for window in [f['source']]+([f['header']] if f['header'] else []):
                    assert next(d for d in window['references'] if d['label']=='guide')['destination']=='table-guide.md:9'
                if f['header']: assert f['header']['line']==1 and f['header']['endLine']==2
            # A different document's equal label cannot replace this source's definition.
            restored=value('GET',base+f'/fragment?target={target}&revision={revision}')
            assert next(d for d in restored['source']['references'] if d['label']=='guide')['destination']=='guide.md:5'
            assert call('POST','/studio/markdown/close',dict(id=table_id))[0]==200
        assert call('POST','/studio/markdown/close',dict(id=reader))[0]==200
        assert call('GET',image_path)[0]==410
        assert not fixture.runtime.starts(api_root),'Reference image API started a model turn'
        return dict(compactTransport=True,indexReadyMs=index_ready_ms,indexWorkerMs=index['indexMs'],firstFragmentMs=fragment_ms,fragmentBytes=len(raw),previewBytes=len(pixels),sourceBytes=len(body.encode()),originalLease=True,sourceUnchanged=True,revisionAndOriginGuards=True,closedReaderRejected=True,quoteSourceLines=True,referenceTableHeaders=(root/'reference-source').exists())
    finally:service.close()

def run(native,gateway,root,api_only=False):
    requests=[]
    class Canary(http.server.BaseHTTPRequestHandler):
        def do_GET(self): requests.append(self.path);self.send_response(404);self.end_headers()
        def log_message(self,*args): pass
    server=http.server.ThreadingHTTPServer(('127.0.0.1',0),Canary)
    thread=threading.Thread(target=server.serve_forever,daemon=True);thread.start()
    png_bytes=(root/'expected.png').read_bytes();chat_png=(root/'expected-chat.png').read_bytes();port=server.server_port
    svg=f'<svg xmlns="http://www.w3.org/2000/svg" width="120" height="80"><rect width="120" height="80" fill="#008866"/><image href="{(root/"expected.png").as_uri()}" width="120" height="80"/><image href="http://127.0.0.1:{port}/svg-canary.png" width="120" height="80"/></svg>'
    (root/'expected.svg').write_text(svg)
    body='# Embedded images\n\n![Embedded PNG 中文😀](data:image/png;base64,'+base64.b64encode(png_bytes).decode()+')\n\n![Embedded SVG](data:image/svg+xml;charset=utf-8,'+quote(svg,safe='')+')\n\n![Rejected HTML](data:text/html,%3Cscript%3E)\n\n![Remote image](http://127.0.0.1:'+str(port)+'/markdown-canary.png)\n'
    if (root/'reference-source').exists():
        image_url='data:image/png;base64,'+base64.b64encode(png_bytes).decode()
        svg_url='data:image/svg+xml;charset=utf-8,'+quote(svg,safe='')
        body=body.replace('# Embedded images\n','# Embedded images [Guide][guide]\n').replace('('+image_url+')','[picture]').replace('('+svg_url+')','[vector]')
        body+='\n[picture]: '+image_url+'\n[vector]: <'+svg_url+'>\n[guide]: guide.md:5 \"Guide title\"\n'
        (root/'project/guide.md').write_text('# Guide\n\nOpening reference links.\n\n## Target\n\nNative reader destination.\n')
    (root/'project/embedded.md').write_text(body)
    chat_body=body.replace(base64.b64encode(png_bytes).decode(),base64.b64encode(chat_png).decode())
    (root/'raw-items.json').write_text(json.dumps([dict(id='embedded-assistant',type='agentMessage',text=chat_body)]))
    (root/'project/replacement.md').write_text('# Replacement\n\nNo embedded images.\n')
    try:
        api=verify_projection_api(gateway,root,body,png_bytes)
    except BaseException:
        server.shutdown();server.server_close();thread.join(timeout=5)
        raise
    if api_only:
        server.shutdown();server.server_close();thread.join(timeout=5)
        assert not requests,('Image API fetched remote resources',requests)
        print(json.dumps(dict(ok=True,api=api,noRemoteFetch=True,referenceDefinitions=(root/'reference-source').exists(),artifacts=str(root))))
        return
    plans=[dict(interface='FileChooser',method='SaveFile',title='Save original image',options=dict(multiple=False,directory=False,current_name='image.'+extension),uris=[(root/name).as_uri()]) for extension,name in [('png','saved-chat.png'),('svg','saved-reader.svg')]]
    if (root/'large-source').exists(): plans.append(dict(interface='FileChooser',method='SaveFile',title='Save original image',options=dict(multiple=False,directory=False,current_name='image.png'),uris=[(root/'saved-reader.png').as_uri()]))
    controlled=OwnedMutterInput();portal=FilePortal(root,plans)
    try:
        with (root/'app.log').open('w') as log:
            child=subprocess.Popen([native],cwd=root,env=dict(os.environ),stdout=log,stderr=log)
            memory=ProcessMemory(child.pid,gateway,include_web_content=True)
            try:
                deadline=time.monotonic()+100
                while child.poll() is None:
                    portal.iterate();controlled.respond(root)
                    if portal.failure: raise AssertionError((root,portal.failure))
                    if time.monotonic()>deadline: raise TimeoutError(('Embedded image fixture timed out',root))
                    time.sleep(.02)
                portal.iterate()
            except BaseException: child.terminate();child.wait(timeout=10);raise
            finally: measured=memory.close()
    finally: portal.close();controlled.close();server.shutdown();server.server_close();thread.join(timeout=5)
    result=json.loads((root/'embedded-images.json').read_text())
    assert child.returncode==0 and result['ok'],(root,result,(root/'app.log').read_text()[-2000:])
    assert not portal.failure and not portal.plan and len(portal.calls)==(3 if (root/'large-source').exists() else 2),(root,portal.failure,portal.plan)
    assert (root/'saved-chat.png').read_bytes()==chat_png,'Saved preview instead of original PNG'
    assert (root/'saved-reader.svg').read_bytes()==(root/'expected.svg').read_bytes(),'Saved preview instead of original SVG'
    if (root/'large-source').exists(): assert (root/'saved-reader.png').read_bytes()==png_bytes,'Lost original after reader closure'
    assert not requests,('Embedded or Markdown image fetched remote content',requests)
    assert not outputs.runtime.starts(root),'Image reading started a model prompt'
    assert measured['webContentProcessesObserved']==0,'Markdown image started a web engine'
    assert not any(s in (root/'app.log').read_text() for s in ['Gtk-WARNING','CRITICAL','panicked','Theme parser error']),(root,(root/'app.log').read_text()[-2000:])
    for name in ['backend.pid','opencode.pid']:
        if (root/name).exists(): assert not Path('/proc',(root/name).read_text().strip()).exists(),'Owned provider survived'
    print(json.dumps(dict(result,api=api,noRemoteFetch=True,noWebEngine=True,exactOriginalBytes=True,sourceBytes=len(body.encode()),referenceDefinitions=(root/'reference-source').exists(),pngBytes=len(png_bytes),memory=measured,artifacts=str(root))))

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('native');parser.add_argument('gateway');parser.add_argument('--private-root',type=Path);parser.add_argument('--large-source',action='store_true');parser.add_argument('--references',action='store_true');parser.add_argument('--api-only',action='store_true');args=parser.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE,(0,0));native,gateway=[str(Path(p).resolve()) for p in [args.native,args.gateway]]
    if args.private_root: return run(native,gateway,args.private_root)
    base=Path(__file__).resolve().parents[1]/'target/gnome-native-smoke';base.mkdir(parents=True,exist_ok=True)
    root=Path(tempfile.mkdtemp(prefix='embedded-images-',dir=base));outputs.runtime.create_profile(root)
    original=png();(root/'expected-chat.png').write_bytes(original)
    (root/'expected.png').write_bytes(original if not args.large_source else original+b'\0'*(10*1024*1024-len(original)))
    if args.large_source: (root/'large-source').write_text('1')
    if args.references: (root/'reference-source').write_text('1')
    if args.api_only: return run(native,gateway,root,api_only=True)
    (root/'config/codex-thread-studio/gnome.json').write_text(json.dumps(dict(selectedBackend='codex',selectedSession='target')))
    for name,source in zip(['fake-codex.py','fake-opencode.py'],outputs.providers()):
        path=root/name;path.write_text(source);path.chmod(0o700)
    env=dict(os.environ,CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway,CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),CODEX_HOME=str(root/'codex'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),READER_CACHE_DIR=str(root/'reader-cache'),GDK_BACKEND='wayland',GTK_USE_PORTAL='1',GIO_USE_VFS='local',GIO_USE_VOLUME_MONITOR='unix',STUDIO_GNOME_SMOKE_DIR=str(root),STUDIO_GNOME_SMOKE_EMBEDDED_IMAGES='1',STUDIO_OWNED_MUTTER_INPUT='1')
    with isolated_environment(env,headless=True) as private:
        subprocess.run(['dbus-run-session','--',shutil.which('mutter') or 'mutter','--headless','--wayland','--no-x11','--wayland-display','studio-test','--virtual-monitor','1600x1000','--',sys.executable,str(Path(__file__).resolve()),native,gateway,'--private-root',str(root)],env=private,cwd=root,check=True,timeout=120)
if __name__=='__main__': main()
