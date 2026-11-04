#!/usr/bin/env python3
"""Private native table/header images, nested captions, viewport admission and Save."""
import argparse,base64,hashlib,http.server,importlib.util,json,os,random,resource,shutil,struct,subprocess,sys,tempfile,threading,time,zlib
from pathlib import Path
from urllib.parse import quote
sys.dont_write_bytecode=True
sys.path.insert(0,str(Path(__file__).with_name('test-support')))
from private_wayland import isolated_environment,OwnedMutterInput
from private_file_portal import FilePortal
from memory_sampler import ProcessMemory
spec=importlib.util.spec_from_file_location('table_image_base',Path(__file__).with_name('smoke-gnome-embedded-images.py'));base=importlib.util.module_from_spec(spec);spec.loader.exec_module(base)
def png(seed,height):
    def chunk(kind,data):return struct.pack('>I',len(data))+kind+data+struct.pack('>I',zlib.crc32(kind+data))
    raw=random.Random(seed).randbytes(128*height*4)
    return b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>2I5B',128,height,8,6,0,0,0))+chunk(b'IDAT',zlib.compress(b''.join(b'\0'+raw[i:i+512] for i in range(0,len(raw),512))))+chunk(b'IEND',b'')
def verify_api(gateway,root):
    from urllib.request import Request
    from urllib.error import HTTPError
    runtime=base.outputs.runtime;profile=root/'api-profile';runtime.create_profile(profile)
    runtime.fixture.BACKEND=runtime.CODEX;runtime.fixture.OPENCODE_BACKEND=runtime.OPENCODE
    service=runtime.fixture.Gateway(Path(gateway),profile,'codex')
    def call(path,body=None,auth=True):
        request=Request(service.origin+path,data=None if body is None else json.dumps(body).encode(),headers=({'Authorization':'Bearer '+service.token} if auth else {})|({'Content-Type':'application/json'} if body is not None else {}))
        try:
            with service.client.open(request,timeout=30) as r:return r.status,r.read(),dict(r.headers)
        except HTTPError as e:return e.code,e.read(),dict(e.headers)
    wire_sizes={}
    def value(path,body=None):
        status,data,_=call(path,body);assert status==200,(root,path,status,data[:250]);wire_sizes[path]=len(data);return json.loads(data)
    def opened(name):
        reader=value('/studio/native/markdown/open',dict(root=str(root/'project'),path=str(root/'project'/name)))['readerId'];path='/studio/native/markdown/'+reader
        until=time.monotonic()+25
        while True:
            document=value(path+'/document')
            if document['state']=='ready':break
            assert document['state']=='indexing' and time.monotonic()<until,(root,document.get('error'))
            time.sleep(.03)
        return reader,path,document['index']
    expected={name:hashlib.sha256((root/'project'/f'{name}.png').read_bytes()).hexdigest() for name in ['outer','inner','next','header']}
    try:
        reader,path,index=opened('nested.md');revision=index['revision']
        f=value(path+'/fragment?target=0&revision='+revision);items=f['source']['projection'];assert len(items)==2
        for p,name in zip(items,['outer','next']):
            status,data,headers=call(path+f'/image?page={p["page"]}&image={p["image"]}&revision={revision}&original=true');assert status==200 and data[:8]==b'STIMS001'
            n=struct.unpack('<I',data[8:12])[0];assert hashlib.sha256(data[12:12+n]).hexdigest()==expected[name],(root,name,headers)
        assert call(path+'/image?page=0&image=2&revision='+revision)[0]==400
        call('/studio/markdown/close',dict(id=reader))
        reader,path,index=opened('table.md');revision=index['revision']
        assert index['tables'][0]['rows']==120
        table=value(path+'/table-source?id=0&row=50&columns=0,1&revision='+revision);assert table['rowStart']==50
        f=table['fragments'][0]
        first=value(path+'/fragment?target=0&revision='+revision)
        for p,name in [(first['source']['projection'][0],'header'),(first['source']['projection'][2],'next')]:
            status,data,headers=call(path+f'/image?page={p["page"]}&image={p["image"]}&revision={revision}')
            assert status==200 and headers['x-studio-source-sha256']==expected[name],(root,name,status,headers)
        assert call(path+'/fragment?target=0',auth=False)[0]==401
        assert not runtime.starts(profile),'Table image API started a model turn'
        call('/studio/markdown/close',dict(id=reader))
        return dict(nestedOrdinals=True,exactOriginals=True,tableHeaders=True,pagedRows=True,sourceBytes=(root/'project/table.md').stat().st_size,tableTransportBytes=wire_sizes[path+'/table-source?id=0&row=50&columns=0,1&revision='+revision])
    finally:service.close()
def prepare(root):
    base.outputs.runtime.create_profile(root)
    for name,seed,height in [('outer',42,512),('inner',43,128),('next',44,256),('header',45,128)]: (root/'project'/f'{name}.png').write_bytes(png(seed,height))
    svg='<svg xmlns="http://www.w3.org/2000/svg" width="120" height="80"><rect width="120" height="80" fill="#008866"/><text x="8" y="45" font-size="15" fill="white">SVG 中文</text></svg>'
    (root/'project/vector.svg').write_text(svg)
    svg_url='data:image/svg+xml;charset=utf-8,'+quote(svg,safe='')
    definitions='\n'+''.join('['+name+']: data:image/png;base64,'+base64.b64encode((root/'project'/f'{name}.png').read_bytes()).decode()+'\n' for name in ['outer','inner','next','header'])
    definitions+='[vector]: <'+svg_url+'>\n'
    nested='![Outer ![Inner][inner] tail][outer]\n\n![Next][next]\n'+definitions
    (root/'project/nested.md').write_text(nested)
    body='| ![Header][header] | Image |\n|---|---|\n'+''.join('| Row '+str(i)+' | '+('![SVG caption 中文][vector]' if i==2 else '!['+('Bottom' if i==49 else 'Outer')+' ![Inner][inner] tail]['+('outer' if i%2==0 else 'next')+']')+' |\n' for i in range(120))+definitions
    (root/'project/table.md').write_text(body)
    chat='| ![Header](header.png) | Image |\n|---|---|\n'+''.join('| Row '+str(i)+' | '+('![SVG caption 中文]('+svg_url+')' if i==2 else '![Row image]('+('outer.png' if i%2==0 else 'next.png')+')')+' |\n' for i in range(120))
    (root/'raw-items.json').write_text(json.dumps([dict(id='table-images-assistant',type='agentMessage',text=chat)]))
    (root/'config/codex-thread-studio/gnome.json').write_text(json.dumps(dict(selectedBackend='codex',selectedSession='target')))
    for name,source in zip(['fake-codex.py','fake-opencode.py'],base.outputs.providers()):p=root/name;p.write_text(source);p.chmod(0o700)
def run(native,gateway,root):
    api=verify_api(gateway,root)
    plans=[dict(interface='FileChooser',method='SaveFile',title='Save original image',options=dict(multiple=False,directory=False,current_name=initial),uris=[(root/name).as_uri()]) for initial,name in [('outer.png','saved-chat.png'),('image.png','saved-header.png'),('image.svg','saved-svg.svg'),('image.png','saved-reader.png')]]
    portal=FilePortal(root,plans);controlled=OwnedMutterInput()
    try:
        with (root/'app.log').open('w') as log:
            child=subprocess.Popen([native],cwd=root,env=dict(os.environ),stdout=log,stderr=log);observer=ProcessMemory(child.pid,gateway,include_web_content=True)
            try:
                deadline=time.monotonic()+110
                while child.poll() is None:
                    portal.iterate();controlled.respond(root)
                    assert not portal.failure,(root,portal.failure)
                    if time.monotonic()>deadline:raise TimeoutError(root)
                    time.sleep(.02)
                portal.iterate()
            except BaseException:child.terminate();child.wait(timeout=10);raise
            finally:memory=observer.close()
    finally:portal.close();controlled.close()
    result=json.loads((root/'table-images.json').read_text());assert child.returncode==0 and result['ok'],(root,result,(root/'app.log').read_text()[-2500:])
    assert not portal.plan and len(portal.calls)==4,(root,portal.plan)
    for output,source in [('saved-chat.png','outer.png'),('saved-header.png','header.png'),('saved-svg.svg','vector.svg'),('saved-reader.png','outer.png')]:assert (root/output).read_bytes()==(root/'project'/source).read_bytes(),(root,output)
    assert not base.outputs.runtime.starts(root),'Table image reading started a model turn'
    assert memory['webContentProcessesObserved']==0
    assert not any(s in (root/'app.log').read_text() for s in ['Gtk-WARNING','CRITICAL','panicked','Theme parser error'])
    for name in ['backend.pid','opencode.pid']:
        if (root/name).exists():assert not Path('/proc',(root/name).read_text().strip()).exists(),'Owned provider survived'
    print(json.dumps(dict(result,api=api,memory=memory,artifacts=str(root))))
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('native');p.add_argument('gateway');p.add_argument('--api-only',action='store_true');p.add_argument('--private-root',type=Path);args=p.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE,(0,0));native,gateway=[str(Path(v).resolve()) for v in [args.native,args.gateway]]
    if args.private_root:return run(native,gateway,args.private_root)
    directory=Path(__file__).resolve().parents[1]/'target/gnome-native-smoke';directory.mkdir(exist_ok=True,parents=True)
    root=Path(tempfile.mkdtemp(prefix='table-images-',dir=directory));prepare(root)
    if args.api_only:print(json.dumps(dict(ok=True,api=verify_api(gateway,root),artifacts=str(root))));return
    env=dict(os.environ,CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway,CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),CODEX_HOME=str(root/'codex'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),READER_CACHE_DIR=str(root/'reader-cache'),GDK_BACKEND='wayland',GTK_USE_PORTAL='1',GIO_USE_VFS='local',GIO_USE_VOLUME_MONITOR='unix',STUDIO_GNOME_SMOKE_DIR=str(root),STUDIO_GNOME_SMOKE_TABLE_IMAGES='1',STUDIO_OWNED_MUTTER_INPUT='1')
    with isolated_environment(env,headless=True) as private:subprocess.run(['dbus-run-session','--',shutil.which('mutter') or 'mutter','--headless','--wayland','--no-x11','--wayland-display','studio-test','--virtual-monitor','1600x1000','--',sys.executable,str(Path(__file__).resolve()),native,gateway,'--private-root',str(root)],env=private,cwd=root,check=True,timeout=140)
if __name__=='__main__':main()
