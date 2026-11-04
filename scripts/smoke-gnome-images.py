#!/usr/bin/env python3
"""Bounded native image API/Wayland controls; private files and fake providers only."""
import argparse
import base64
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import resource
import struct
import subprocess
import sys
import tempfile
import time
from urllib.request import Request
from urllib.error import HTTPError
import zlib
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
sys.dont_write_bytecode=True
sys.path.insert(0,str(Path(__file__).with_name('test-support')))
from memory_sampler import ProcessMemory
spec=importlib.util.spec_from_file_location('image_fixture',Path(__file__).with_name('smoke-gnome-router-runtime.py'))
runtime=importlib.util.module_from_spec(spec);spec.loader.exec_module(runtime)

def png(width,height,alpha=255):
    def chunk(kind,data):return struct.pack('>I',len(data))+kind+data+struct.pack('>I',zlib.crc32(kind+data))
    row=bytes(channel for x in range(width) for channel in (20+x*180//width,110,210-x*140//width,alpha))
    return b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>2I5B',width,height,8,6,0,0,0))+chunk(b'IDAT',zlib.compress((b'\0'+row)*height))+chunk(b'IEND',b'')

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('native');parser.add_argument('gateway');parser.add_argument('--api-only',action='store_true');args=parser.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE,(0,0))
    native,gateway=map(lambda value:str(Path(value).resolve()),[args.native,args.gateway])
    base=Path(__file__).resolve().parents[1]/'target/gnome-native-smoke';base.mkdir(parents=True,exist_ok=True)
    root=Path(tempfile.mkdtemp(prefix='images-ui-',dir=base));runtime.create_profile(root);project=root/'project';(project/'nested').mkdir(exist_ok=True)
    (project/'sample.png').write_bytes(png(3000,1000));(project/'broken.png').write_bytes(b'\x89PNG\r\n\x1a\ninvalid')
    (project/'alpha.png').write_bytes(png(2,1,128))
    (project/'fresh.png').write_bytes(png(3000,1000,240))
    (project/'escape.png').symlink_to('/etc/passwd')
    fetched=[]
    class Canary(BaseHTTPRequestHandler):
        def do_GET(self):
            fetched.append(self.path);self.send_response(200);self.end_headers();self.wfile.write(png(2,1))
        def log_message(self,*args):pass
    canary=ThreadingHTTPServer(('127.0.0.1',0),Canary)
    thread=threading.Thread(target=canary.serve_forever,daemon=True);thread.start()
    bitmap='iVBORw0KGgoAAAANSUhEUgAAAAIAAAABCAYAAAD0In+KAAAADklEQVR4nGO4o8H1H4QBEjMEGyP2UiEAAAAASUVORK5CYII='
    svg=f'<svg xmlns="http://www.w3.org/2000/svg" width="400" height="240"><rect width="400" height="240" fill="#008866"/><text x="30" y="100" font-family="sans-serif" font-size="34" fill="white">Native SVG 中文</text><image x="300" y="150" width="80" height="40" href="data:image/png;base64,{bitmap}"/><image href="{project/"sample.png"}" width="20" height="20"/><image href="{(project/"sample.png").as_uri()}" width="20" height="20"/><image href="http://127.0.0.1:{canary.server_port}/unwanted.png" width="20" height="20"/></svg>'
    (project/'vector.svg').write_text(svg)
    (project/'nested/images.md').write_text('# Native Markdown images\n\nA relative image with source anchors.\n\n![Wide image](../sample.png)\n\n![Vector preview](../vector.svg)\n\n![External image](https://example.invalid/private.png)\n')
    runtime.fixture.BACKEND=runtime.CODEX;runtime.fixture.OPENCODE_BACKEND=runtime.OPENCODE
    service=runtime.fixture.Gateway(Path(gateway),root,'codex')
    last_headers={}
    def image(path,authorized=True,animation=False):
        nonlocal last_headers
        # The gateway fixture owns this token; never print its URL or headers.
        request=Request(service.origin+'/studio/native/image',data=json.dumps({'root':str(project),'path':path,**({'animation':True} if animation else {})}).encode(),headers={'Content-Type':'application/json',**({'Authorization':'Bearer '+service.token} if authorized else {})})
        try:
            with service.client.open(request,timeout=30) as response:
                last_headers=dict(response.headers);return response.status,response.read()
        except HTTPError as error:return error.code,error.read()
    def inline(data,authorized=True,origin=None,animation=False):
        request=Request(service.origin+'/studio/native/image/inline',data=data,headers={'Content-Type':'application/octet-stream',**({'Authorization':'Bearer '+service.token} if authorized else {}),**({'Origin':origin} if origin else {}),**({'X-Studio-Animation':'1'} if animation else {})})
        try:
            with service.client.open(request,timeout=30) as response:
                if response.status==200:
                    assert response.headers['x-studio-source-sha256']==hashlib.sha256(data).hexdigest()
                    assert response.headers['Cache-Control']=='no-store'
                return response.status,response.read()
        except HTTPError as error:return error.code,error.read()
    def original(path,sha256,authorized=True,origin=None,extra=None):
        request=Request(service.origin+'/studio/native/image/original',data=json.dumps(dict(root=str(project),path=path,sha256=sha256,**(extra or {}))).encode(),headers={'Content-Type':'application/json',**({'Authorization':'Bearer '+service.token} if authorized else {}),**({'Origin':origin} if origin else {})})
        try:
            with service.client.open(request,timeout=30) as response:
                assert response.headers['Content-Type']=='application/octet-stream'
                assert response.headers['Content-Disposition']=='attachment'
                assert response.headers['Cache-Control']=='no-store'
                return response.status,response.read()
        except HTTPError as error:return error.code,error.read()
    def document_request(method,path,data=None):
        request=Request(service.origin+path,data=None if data is None else json.dumps(data).encode(),method=method,headers={'Authorization':'Bearer '+service.token,**({'Content-Type':'application/json'} if data is not None else {})})
        try:
            with service.client.open(request,timeout=30) as response:return response.status,response.read()
        except HTTPError as error:return error.code,error.read()
    def document_value(method,path,data=None):
        status,raw=document_request(method,path,data);assert status==200,(path,status,raw[:200]);return json.loads(raw)
    try:
        animation_formats=[]
        fixtures=Path(__file__).resolve().parents[1]/'src-tauri/tests/fixtures/native-images'
        (project/'infinite.gif').write_bytes((fixtures/'two-frame-infinite.gif').read_bytes())
        (project/'over-budget.gif').write_bytes((fixtures/'over-budget.gif').read_bytes())
        status,limited=image('over-budget.gif',animation=True)
        assert status==200 and limited[:8]==b'STIMG001' and next(v for k,v in last_headers.items() if k.lower()=='x-studio-animation-limited')=='1'
        assert image('over-budget.gif')[1]==limited
        assert original('over-budget.gif',hashlib.sha256((project/'over-budget.gif').read_bytes()).hexdigest())==(200,(project/'over-budget.gif').read_bytes())
        for extension in ['gif','png','webp']:
            content=(fixtures/f'two-frame.{extension}').read_bytes()
            name=f'animated.{extension}';(project/name).write_bytes(content)
            status,still=image(name);assert status==200 and still[:8]==b'STIMG001'
            status,timeline=image(name,animation=True);assert status==200 and timeline[:8]==b'STIMA001',(extension,status,timeline[:100])
            assert struct.unpack('<6I',timeline[8:32])==(5,3,5,3,2,3 if extension=='gif' else 2)
            assert struct.unpack('<I',timeline[32:36])[0]==40 and struct.unpack('<I',timeline[96:100])[0]==90
            assert timeline[36:40]==bytes([255,0,0,255] if extension=='gif' else [100,40,20,128])
            assert inline(content,animation=True)==(200,timeline)
            assert inline(content)[1][:8]==b'STIMG001'
            assert original(name,hashlib.sha256(content).hexdigest())==(200,content)
            assert image(name)[1]==still
            assert image(name,authorized=False,animation=True)[0]==401
            assert inline(content,origin='https://example.invalid',animation=True)[0]==403
            mime='image/'+extension
            # Snapshot image routes own long projected data URIs. An ignored
            # fragment exercises that path without changing original image bytes.
            data_uri=f'data:{mime};base64,{base64.b64encode(content).decode()}#'+('animation-fixture-'*600)
            body=f'# Animation\n\n![Animation]({data_uri})\n'
            document=project/f'animation-{extension}.md';document.write_text(body)
            reader=document_value('POST','/studio/native/markdown/open',dict(root=str(project),path=str(document)))['readerId']
            base='/studio/native/markdown/'+reader;deadline=time.monotonic()+20
            while True:
                snapshot=document_value('GET',base+'/document')
                if snapshot['state']=='ready':break
                assert snapshot['state']=='indexing' and time.monotonic()<deadline
                time.sleep(.03)
            index=snapshot['index'];position=body.encode().index(b'![Animation')
            page=next(i for i,p in enumerate(index['pages']) if p['start']<=position<p['end'])
            uri=base+f'/image?page={page}&image=0&revision='+index['revision']
            status,preview=document_request('GET',uri)
            assert status==200 and preview[:8]==b'STIMG001',(root,extension,page,status,preview[:200])
            assert document_request('GET',uri+'&animation=true')==(200,timeline)
            status,envelope=document_request('GET',uri+'&animation=true&original=true')
            assert status==200 and envelope[:8]==b'STIMS001'
            length=struct.unpack('<I',envelope[8:12])[0]
            assert envelope[12:12+length]==content and envelope[12+length:]==timeline
            document_value('POST','/studio/markdown/close',dict(id=reader))
            assert document_request('GET',uri+'&animation=true')[0]==410
            assert document.read_text()==body
            animation_formats.append(extension)
        assert image('sample.png',False)[0]==401
        status,data=image('sample.png');assert status==200,(status,data[:120]);assert data[:8]==b'STIMG001';assert struct.unpack('<4I',data[8:24])==(3000,1000,2048,683);assert len(data)==24+2048*683*4
        expected=(project/'sample.png').read_bytes();sha256=hashlib.sha256(expected).hexdigest()
        assert next(v for k,v in last_headers.items() if k.lower()=='x-studio-source-sha256')==sha256
        assert next(v for k,v in last_headers.items() if k.lower()=='x-studio-source-mime')=='image/png'
        assert original('sample.png',sha256)==(200,expected),'Original image was replaced by its scaled preview'
        assert original('sample.png',sha256,False)[0]==401
        assert original('sample.png',sha256,origin='https://example.invalid')[0]==403
        assert original('sample.png','invalid')[0]==400
        assert original('sample.png',sha256,extra={'unexpected':True})[0]==400
        (project/'sample.png').write_bytes(png(3,2))
        assert original('sample.png',sha256)[0]==409,'Changed source silently substituted a different image'
        (project/'sample.png').write_bytes(expected)
        assert original('escape.png',sha256)[0]==403
        assert image('sample.png')[1]==data,'Identical content cache changed pixels'
        assert image('alpha.png')[1][24:28]==bytes([10,55,105,128])
        svg_started=time.monotonic()
        status,data=image('vector.svg');assert status==200 and data[24:28]==bytes([0,136,102,255]),'SVG resolved external raster data'
        svg_api_ms=round((time.monotonic()-svg_started)*1000)
        width=struct.unpack('<I',data[16:20])[0]
        private_offset=24+(51*width+51)*4
        assert data[private_offset:private_offset+4]==bytes([0,136,102,255]),'SVG loaded a private bitmap inside its letterboxed region'
        offset=24+(870*width+1740)*4
        assert data[offset:offset+4]==bytes([220,40,10,255]),'SVG embedded bitmap was omitted'
        vector=(project/'vector.svg').read_bytes();assert original('vector.svg',hashlib.sha256(vector).hexdigest())==(200,vector)
        status,inline_pixels=inline(vector)
        assert status==200 and inline_pixels[24:28]==bytes([0,136,102,255]),'Inline SVG resolved private or remote images'
        assert inline_pixels[private_offset:private_offset+4]==bytes([0,136,102,255]),'Inline SVG loaded a private bitmap'
        assert inline_pixels[offset:offset+4]==bytes([220,40,10,255]),'Inline SVG lost embedded bitmap pixels'
        assert not fetched,'SVG resolver fetched an external image'
        element=f'<image width="2" height="1" href="data:image/png;base64,{bitmap}"/>'
        excessive=('<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10">'+element*65+'</svg>').encode()
        status,body=inline(excessive);assert status==400 and b'64 embedded' in body,(status,body[:160])
        huge=base64.b64encode(png(32769,1)).decode()
        oversized=f'<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><image width="20" height="10" href="data:image/png;base64,{huge}"/></svg>'.encode()
        assert inline(oversized)[0]==400,'SVG bitmap dimension limit was silently ignored'
        assert inline(vector,authorized=False)[0]==401
        assert inline(vector,origin='https://example.invalid')[0]==403
        assert inline(b'<html><script>alert(1)</script></html>')[0]==400
        assert inline(b'<svg>'+b' '* (2*1024*1024)+b'</svg>')[0]==400
        assert image('escape.png')[0]==403;assert image('broken.png')[0]==400
        with (project/'oversize.png').open('wb') as output:output.truncate(25*1024*1024+1)
        assert image('oversize.png')[0]==413
        assert not runtime.starts(root),'Image API triggered a model prompt'
    finally:service.close();canary.shutdown();canary.server_close();thread.join(timeout=2)
    if args.api_only:print(json.dumps({'ok':True,'artifacts':str(root),'imageApi':True,'animationTransport':animation_formats,'documentAnimationTransport':True,'originalIdentity':True,'originalBytes':True,'changedSourceRejected':True,'svgBitmaps':True,'svgResourceBounds':True,'noSvgResourceFetch':True,'svgApiMs':svg_api_ms}));return
    backend=runtime.CODEX.replace("assert params['excludeTurns'] is True","pass").replace("    elif method=='turn/start':","    elif method=='model/list':answer(identity,{'data':[]})\n    elif method=='turn/start':")
    for name,source in [('fake-codex.py',backend),('fake-opencode.py',runtime.OPENCODE)]:
        path=root/name;path.write_text(source);path.chmod(0o700)
    environment=dict(os.environ,CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway,CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),CODEX_HOME=str(root/'codex'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),READER_CACHE_DIR=str(root/'reader-cache'),GDK_BACKEND='wayland',STUDIO_GNOME_SMOKE_DIR=str(root),STUDIO_GNOME_SMOKE_IMAGE_VIEWER='1')
    environment.pop('DISPLAY',None);environment.pop('XAUTHORITY',None)
    with (root/'app.log').open('w') as log:
        child=subprocess.Popen([native],cwd=root,env=environment,stdout=log,stderr=log)
        observer=ProcessMemory(child.pid,gateway)
        try:
            try:child.wait(timeout=70)
            except subprocess.TimeoutExpired:child.terminate();child.wait(timeout=10);raise AssertionError(f'Image fixture timed out: {root}')
        finally:memory=observer.close()
    assert child.returncode==0,(root,child.returncode)
    assert (root/'images-ui.json').is_file(),(root,'Native image fixture did not publish a result',(root/'app.log').read_text()[-2000:])
    result=json.loads((root/'images-ui.json').read_text());assert result['ok'],(root,result)
    assert result['animation']['ok'] and result['animation']['currentFrameCopy'] and result['animation']['finitePlays']
    assert result['animation']['hiddenWindowSuspends'] and result['animation']['nativeTranslation']
    assert result['snapshotAnimation']['saveAfterSnapshotReplacement'] and result['animatedChat']['offscreenSuspends']
    assert not runtime.starts(root),'Image viewing triggered a model prompt'
    for name in ['backend.pid','opencode.pid']:
        if not (root/name).exists():continue
        identity=(root/name).read_text().strip()
        for _ in range(100):
            if not Path('/proc',identity).exists():break
            time.sleep(.05)
        assert not Path('/proc',identity).exists(),f'Owned backend {identity} survived image fixture'
    print(json.dumps({'ok':True,'artifacts':str(root),'imageApi':True,'svgBitmaps':True,'svgResourceBounds':True,'noSvgResourceFetch':True,'svgApiMs':svg_api_ms,'nativeControls':result,'backendCleanup':True,'observedMemory':memory}))
if __name__=='__main__':main()
