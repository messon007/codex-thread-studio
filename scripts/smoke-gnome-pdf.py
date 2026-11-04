#!/usr/bin/env python3
"""Native PDF service/Wayland fixture; private documents, no model prompts."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import resource
import struct
import hashlib
import subprocess
import sys
import tempfile
import time
import uuid
from urllib.error import HTTPError
from urllib.request import Request
sys.dont_write_bytecode=True
sys.path.insert(0,str(Path(__file__).with_name('test-support')))
from memory_sampler import ProcessMemory
spec=importlib.util.spec_from_file_location('pdf_fixture',Path(__file__).with_name('smoke-gnome-router-runtime.py'))
runtime=importlib.util.module_from_spec(spec);spec.loader.exec_module(runtime)

def rc4(key,data):
    state=list(range(256));j=0
    for i in range(256):j=(j+state[i]+key[i%len(key)])%256;state[i],state[j]=state[j],state[i]
    out=bytearray();i=j=0
    for value in data:
        i=(i+1)%256;j=(j+state[i])%256;state[i],state[j]=state[j],state[i];out.append(value^state[(state[i]+state[j])%256])
    return bytes(out)

def document(path,pages=3,password=None,rotation=0):
    key=None;identity=b'private-pdf-test'
    if password is not None:
        padding=bytes.fromhex('28bf4e5e4e758a4164004e56fffa01082e2e00b6d0683e802f0ca9fe6453697a')
        pad=lambda value:(value.encode()+padding)[:32]
        owner=rc4(hashlib.md5(pad('test-owner')).digest()[:5],pad(password))
        key=hashlib.md5(pad(password)+owner+struct.pack('<i',-4)+identity).digest()[:5];user=rc4(key,padding)
    def encrypt(data,number):
        return rc4(hashlib.md5(key+number.to_bytes(3,'little')+b'\0\0').digest()[:10],data) if key else data
    def title(value,number):return '<'+encrypt(value.encode(),number).hex()+'>' if key else '('+value+')'
    objects=[b'',b'',b'<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>']
    kids=[]
    for index in range(pages):
        identity=len(objects)+1;kids.append(f'{identity} 0 R')
        text=f'Page {index+1} - Native PDF reading'
        body=f'0.08 0.45 0.38 rg 40 620 515 130 re f\nBT /F1 25 Tf 1 1 1 rg 60 700 Td ({text}) Tj ET\nBT /F1 15 Tf 0 0 0 rg 60 580 Td (Searchable content page {index+1}) Tj 0 -28 Td (Case sensitive Needle and trailing space ) Tj ET'.encode()
        body=encrypt(body,identity+1)
        objects.extend([f'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Rotate {rotation} /Resources << /Font << /F1 3 0 R >> >> /Contents {identity+1} 0 R >>'.encode(),b'<< /Length '+str(len(body)).encode()+b' >>\nstream\n'+body+b'\nendstream'])
    outline=len(objects)+1;first=outline+1
    objects.extend([f'<< /Type /Outlines /First {first} 0 R /Last {first+1} 0 R /Count 2 >>'.encode(),f'<< /Title {title("Introduction",first)} /Parent {outline} 0 R /Dest [{kids[0]} /Fit] /Next {first+1} 0 R >>'.encode(),f'<< /Title {title("Final page",first+1)} /Parent {outline} 0 R /Dest [{kids[-1]} /Fit] /Prev {first} 0 R >>'.encode()])
    encryption=''
    if key:
        objects.append(f'<< /Filter /Standard /V 1 /R 2 /Length 40 /O <{owner.hex()}> /U <{user.hex()}> /P -4 >>'.encode())
        encryption=f'/Encrypt {len(objects)} 0 R /ID [<{b"private-pdf-test".hex()}> <{b"private-pdf-test".hex()}>]'
    objects[0]=f'<< /Type /Catalog /Pages 2 0 R /Outlines {outline} 0 R >>'.encode();objects[1]=f'<< /Type /Pages /Count {pages} /Kids [{" ".join(kids)}] >>'.encode()
    data=bytearray(b'%PDF-1.4\n%\xe2\xe3\xcf\xd3\n');offsets=[0]
    for index,value in enumerate(objects,1):offsets.append(len(data));data.extend(f'{index} 0 obj\n'.encode()+value+b'\nendobj\n')
    start=len(data);data.extend(f'xref\n0 {len(offsets)}\n0000000000 65535 f \n'.encode())
    for offset in offsets[1:]:data.extend(f'{offset:010d} 00000 n \n'.encode())
    data.extend(f'trailer\n<< /Size {len(offsets)} /Root 1 0 R {encryption} >>\nstartxref\n{start}\n%%EOF\n'.encode());path.write_bytes(data)

def unpack(data):
    assert data[:8]==b'STPDF001',data[:300]
    size=struct.unpack('<I',data[8:12])[0];meta=json.loads(data[12:12+size]);pixels=data[12+size:]
    assert pixels[:8]==b'STIMG001';ow,oh,width,height=struct.unpack('<4I',pixels[8:24]);assert 0<width<=2048 and 0<height<=2048 and len(pixels)==24+width*height*4
    return meta,pixels

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('native',help='native executable, or gateway when --api-only');parser.add_argument('gateway',nargs='?');parser.add_argument('--api-only',action='store_true');args=parser.parse_args()
    if args.gateway is None and not args.api_only:parser.error('native and gateway executables are required for Wayland checks')
    resource.setrlimit(resource.RLIMIT_CORE,(0,0));native,gateway=map(lambda value:str(Path(value).resolve()),[args.native,args.gateway or args.native])
    base=Path(__file__).resolve().parents[1]/'target/gnome-native-smoke';base.mkdir(parents=True,exist_ok=True);root=Path(tempfile.mkdtemp(prefix='pdf-ui-',dir=base));runtime.create_profile(root);project=root/'project'
    document(project/'sample.pdf');document(project/'rotated.pdf',rotation=90);document(project/'many.pdf',70);document(project/'locked.pdf',password='open-sesame');(project/'escape.pdf').symlink_to('/etc/passwd');(project/'broken.pdf').write_bytes(b'%PDF-1.7\nnot a document')
    runtime.fixture.BACKEND=runtime.CODEX;runtime.fixture.OPENCODE_BACKEND=runtime.OPENCODE
    service=runtime.fixture.Gateway(Path(gateway),root,'codex')
    def request(operation,authorized=True,**fields):
        if operation=='open':fields.setdefault('id',str(uuid.uuid4()))
        request=Request(service.origin+'/studio/native/pdf',data=json.dumps({'operation':operation,**fields}).encode(),headers={'Content-Type':'application/json',**({'Authorization':'Bearer '+service.token} if authorized else {})})
        try:
            with service.client.open(request,timeout=30) as response:return response.status,response.read()
        except HTTPError as error:return error.code,error.read()
    try:
        assert request('open',False,root=str(project),path='sample.pdf')[0]==401
        status,data=request('open',root=str(project),path='sample.pdf');assert status==200,(status,data[:400]);opened,pixels=unpack(data);identity=opened['documentId'];assert opened['pages']==3 and opened['page']==0 and len(opened['outline'])==2 and 'Searchable' in opened['text'],opened
        late=str(uuid.uuid4());assert request('close',id=late)[0]==200;assert request('open',id=late,root=str(project),path='sample.pdf')[0]==410
        page=unpack(request('page',id=identity,page=2)[1])[0];assert page['page']==2 and 'page 3' in page['text']
        def selected(snapshot,word='Page'):
            identity=snapshot['documentId']
            assert request('select',False,id=identity,page=1,range=[0,4])[0]==401
            status,data=request('select',id=identity,page=1,range=[0,4]);assert status==200,(status,data[:400])
            selection=json.loads(data);assert selection['excerpt']==word and selection['hash']==snapshot['hash'] and selection['range']==[0,4] and len(selection['rects'])==1,selection
            r=selection['rects'][0]
            assert 0<=r['x']<r['x']+r['width']<=1.0002 and 0<=r['y']<r['y']+r['height']<=1.0002,r
            if r['width']>r['height']:points=[[r['x']+.001,r['y']+r['height']/2],[r['x']+r['width']-.001,r['y']+r['height']/2]]
            else:points=[[r['x']+r['width']/2,r['y']+.001],[r['x']+r['width']/2,r['y']+r['height']-.001]]
            for endpoints in [points,list(reversed(points))]:
                status,data=request('select',id=identity,page=1,points=endpoints);assert status==200,(status,data[:400])
                assert json.loads(data)['excerpt']==word,data
            for fields in [dict(range=[0,128001]),dict(range=[0,4],points=points),dict(points=[[-1,0],[.1,.2]]),dict(points=[[0,0],[0,0]]),dict(range=[2,1])]:
                assert request('select',id=identity,page=1,**fields)[0]==400,fields
        selected(opened)
        assert request('page',id=identity,page=3)[0]==400
        found=json.loads(request('search',id=identity,query='Needle',**{'from':0})[1]);assert len(found['hits'])==3 and found['next'] is None,found
        assert not json.loads(request('search',id=identity,query='needle',case_sensitive=True,**{'from':0})[1])['hits']
        assert request('open',root=str(project),path='escape.pdf')[0]==403
        assert request('open',root=str(project),path='broken.pdf')[0]==400
        request('close',id=identity);assert request('page',id=identity,page=0)[0]==410
        assert request('select',id=identity,page=0,range=[0,4])[0]==410
        status,data=request('open',root=str(project),path='rotated.pdf');assert status==200,(status,data[:400]);rotated=unpack(data)[0]
        # PDFium's reading order starts at the lower text object on this rotated page.
        selected(rotated,'Case');request('close',id=rotated['documentId'])
        status,data=request('open',root=str(project),path='many.pdf');assert status==200,(status,data[:400]);many=unpack(data)[0];identity=many['documentId']
        first=json.loads(request('search',id=identity,query='Searchable',**{'from':0})[1]);assert len(first['hits'])==32 and first['next']==32,first
        request('close',id=identity)
        assert request('open',root=str(project),path='locked.pdf')[0]==400
        status,data=request('open',root=str(project),path='locked.pdf',password='open-sesame');assert status==200,(status,data[:400]);locked=unpack(data)[0];assert 'Searchable' in locked['text'];request('close',id=locked['documentId'])
        assert not runtime.starts(root),'PDF API triggered a model prompt'
    finally:service.close()
    if args.api_only:print(json.dumps({'ok':True,'artifacts':str(root),'pdfApi':True}));return
    backend=runtime.CODEX.replace("assert params['excludeTurns'] is True","pass").replace("    elif method=='turn/start':","    elif method=='model/list':answer(identity,{'data':[]})\n    elif method=='turn/start':")
    for name,source in [('fake-codex.py',backend),('fake-opencode.py',runtime.OPENCODE)]:
        path=root/name;path.write_text(source);path.chmod(0o700)
    environment=dict(os.environ,CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway,CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),CODEX_HOME=str(root/'codex'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),READER_CACHE_DIR=str(root/'reader-cache'),GDK_BACKEND='wayland',STUDIO_GNOME_SMOKE_DIR=str(root),STUDIO_GNOME_SMOKE_PDF='1')
    environment.pop('DISPLAY',None);environment.pop('XAUTHORITY',None)
    with (root/'app.log').open('w') as log:
        child=subprocess.Popen([native],cwd=root,env=environment,stdout=log,stderr=log);observer=ProcessMemory(child.pid,gateway)
        try:
            try:child.wait(timeout=90)
            except subprocess.TimeoutExpired:child.terminate();child.wait(timeout=10);raise AssertionError(f'PDF fixture timed out: {root}')
        finally:memory=observer.close()
    assert child.returncode==0,(root,child.returncode)
    result=json.loads((root/'pdf-ui.json').read_text());assert result['ok'],(root,result)
    for check in ['liveLanguage','literalDataRetained','passwordLiveLanguage']:assert result.get(check) is True,(root,check,result)
    assert not runtime.starts(root),'PDF viewing triggered a model prompt'
    for name in ['backend.pid','opencode.pid']:
        if not (root/name).exists():continue
        identity=(root/name).read_text().strip()
        for _ in range(100):
            if not Path('/proc',identity).exists():break
            time.sleep(.05)
        assert not Path('/proc',identity).exists(),f'Owned backend {identity} survived PDF fixture'
    print(json.dumps({'ok':True,'artifacts':str(root),'pdfApi':True,'nativeControls':result,'backendCleanup':True,'observedMemory':memory}))
if __name__=='__main__':main()
