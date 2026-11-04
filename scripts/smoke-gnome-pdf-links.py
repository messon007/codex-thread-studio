#!/usr/bin/env python3
"""PDF links through sandboxed PDFium, native GTK controls and an owned URI portal."""
import argparse,importlib.util,json,os,resource,shutil,struct,subprocess,sys,tempfile,time,uuid
from pathlib import Path
sys.dont_write_bytecode=True
spec=importlib.util.spec_from_file_location('pdf_link_fixture',Path(__file__).with_name('smoke-gnome-pdf.py'))
fixture=importlib.util.module_from_spec(spec);spec.loader.exec_module(fixture);runtime=fixture.runtime
sys.path.insert(0,str(Path(__file__).with_name('test-support')))
from private_wayland import isolated_environment,OwnedMutterInput
from private_uri_portal import UriPortal
from memory_sampler import ProcessMemory

def document(path,rotation=0,crop=False,dense=False):
    objects=[b'',b'',b'<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>'];kids=[];page_objects=[]
    for index in range(3):
        number=len(objects)+1;kids.append(f'{number} 0 R');page_objects.append(number)
        text=f'BT /F1 22 Tf 50 780 Td (PDF link page {index+1}) Tj ET\nBT /F1 16 Tf 60 660 Td (Go to final page) Tj 0 -50 Td (Go to middle page) Tj 0 -50 Td (Named destination) Tj 0 -50 Td (Open example web link) Tj 0 -50 Td (Open localhost PDF URL) Tj 0 -50 Td (Multi-region web link) Tj ET'.encode()
        objects.extend([b'',b'<< /Length '+str(len(text)).encode()+b' >>\nstream\n'+text+b'\nendstream'])
    links=[]
    def annotation(action,y,extra='',rect=None):
        number=len(objects)+1;rect=rect or f'60 {y} 300 {y+24}';objects.append(f'<< /Type /Annot /Subtype /Link /Rect [{rect}] /Border [0 0 1] /C [0 .4 .7] {action} {extra} >>'.encode());links.append(f'{number} 0 R')
    if dense=='regions':
        quads=' '.join(f'60 {674-n*8} 200 {674-n*8} 60 {668-n*8} 200 {668-n*8}' for n in range(8))
        for _ in range(100):annotation(f'/Dest [{kids[2]} /Fit]',600,extra='/QuadPoints ['+quads+']',rect='60 600 300 674')
    elif dense:
        for _ in range(300):annotation(f'/Dest [{kids[2]} /Fit]',650)
    else:
        annotation(f'/Dest [{kids[2]} /Fit]',650)
        annotation(f'/A << /S /GoTo /D [{kids[1]} /Fit] >>',600)
        annotation('/Dest /chapter2',550)
        annotation('/A << /S /URI /URI (https://example.invalid/docs?ref=pdf#intro) >>',500)
        annotation('/A << /S /URI /URI (http://127.0.0.1:38669/private.pdf) >>',450)
        annotation('/A << /S /URI /URI (https://example.invalid/multi) >>',380,extra='/QuadPoints [60 424 170 424 60 412 170 412 60 395 230 395 60 383 230 383]',rect='60 380 300 424')
        for i,action in enumerate(['/A << /S /URI /URI (file:///etc/passwd) >>','/A << /S /JavaScript /JS (bad) >>','/A << /S /Launch /F (other.pdf) >>','/A << /S /GoToR /F (other.pdf) /D [0 /Fit] >>','/A << /S /URI /URI (https://user:secret@example.invalid/) >>',f'/Dest [99999 /Fit]']):annotation(action,320-i*25)
        for flag in [2,32,64,8,16]:annotation('/A << /S /URI /URI (https://example.invalid/hidden) >>',100,extra=f'/F {flag}')
        annotation('/A << /S /URI /URI ('+'x'*5000+') >>',80)
        annotation(f'/Dest [{kids[2]} /Fit]',50,rect='0 0 0 0')
    for index,number in enumerate(page_objects):
        cropbox='/CropBox [40 100 555 800]' if crop else ''
        objects[number-1]=f'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] {cropbox} /Rotate {rotation} /Resources << /Font << /F1 3 0 R >> >> /Contents {number+1} 0 R {"/Annots [null "+" ".join(links)+"]" if index==0 else ""} >>'.encode()
    dest=len(objects)+1;objects.append(f'<< /D [{kids[1]} /Fit] >>'.encode())
    objects[0]=f'<< /Type /Catalog /Pages 2 0 R /Names << /Dests << /Names [(chapter2) {dest} 0 R] >> >> >>'.encode();objects[1]=f'<< /Type /Pages /Count 3 /Kids [{" ".join(kids)}] >>'.encode()
    data=bytearray(b'%PDF-1.7\n');offsets=[0]
    for index,body in enumerate(objects,1):offsets.append(len(data));data.extend(f'{index} 0 obj\n'.encode()+body+b'\nendobj\n')
    start=len(data);data.extend(f'xref\n0 {len(offsets)}\n0000000000 65535 f \n'.encode())
    for offset in offsets[1:]:data.extend(f'{offset:010d} 00000 n \n'.encode())
    data.extend(f'trailer\n<< /Size {len(offsets)} /Root 1 0 R >>\nstartxref\n{start}\n%%EOF\n'.encode());path.write_bytes(data)

def api_checks(gateway,root):
    runtime.fixture.BACKEND=runtime.CODEX;runtime.fixture.OPENCODE_BACKEND=runtime.OPENCODE;service=runtime.fixture.Gateway(Path(gateway),root,'codex');timings=[]
    def opened(name):
        at=time.monotonic()
        # Raster responses use the binary transport, not Gateway.api's JSON helper.
        from urllib.request import Request
        request=Request(service.origin+'/studio/native/pdf',data=json.dumps({'operation':'open','id':str(uuid.uuid4()),'root':str(root/'project'),'path':name}).encode(),headers={'Authorization':'Bearer '+service.token,'Content-Type':'application/json'})
        with service.client.open(request,timeout=35) as response:meta,pixels=fixture.unpack(response.read())
        timings.append(round((time.monotonic()-at)*1000,2));return meta
    def close(meta):status,data=service.api('/studio/native/pdf',{'operation':'close','id':meta['documentId']},method='POST');assert status==200
    try:
        baseline=None
        for rotation in [0,90,180,270]:
            meta=opened(f'links-{rotation}.pdf');assert len(meta['links'])==6 and meta['linksUnavailable'] and not meta['linksLimited'],(rotation,meta)
            targets=[link['target'] for link in meta['links']];assert targets[:3]==[{'kind':'page','page':2},{'kind':'page','page':1},{'kind':'page','page':1}],targets
            assert [t['url'] for t in targets[3:]]==['https://example.invalid/docs?ref=pdf#intro','http://127.0.0.1:38669/private.pdf','https://example.invalid/multi']
            assert len(meta['links'][-1]['rects'])==2
            rect=meta['links'][0]['rects'][0]
            actual=[rect[k] for k in ['x','y','width','height']]
            if rotation==0:baseline=actual
            x,y,w,h=baseline
            expected={0:[x,y,w,h],90:[1-y-h,x,h,w],180:[1-x-w,1-y-h,w,h],270:[y,1-x-w,h,w]}[rotation]
            assert all(abs(a-b)<.001 for a,b in zip(actual,expected)),(rotation,actual,expected)
            assert all(0<=r['x']<r['x']+r['width']<=1.0002 and 0<=r['y']<r['y']+r['height']<=1.0002 for link in meta['links'] for r in link['rects'])
            close(meta)
        meta=opened('cropped.pdf');r=meta['links'][0]['rects'][0];expected=[20/515,126/700,240/515,24/700];assert all(abs(r[k]-v)<.001 for k,v in zip(['x','y','width','height'],expected)),(r,expected);close(meta)
        dense=opened('dense.pdf');assert len(dense['links'])==256 and dense['linksLimited'];close(dense)
        regions=opened('regions.pdf');assert len(regions['links'])==64 and sum(len(link['rects']) for link in regions['links'])==512 and regions['linksLimited'];close(regions)
        assert not runtime.starts(root)
        return {'ok':True,'rotations':4,'namedAndActionDestinations':True,'cropping':True,'quadRegions':True,'schemeAndFlagFiltering':True,'boundedLinks':True,'maximumOpenMs':max(timings)}
    finally:service.close()

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('native');p.add_argument('gateway',nargs='?');p.add_argument('--api-only',action='store_true');p.add_argument('--headless',action='store_true');p.add_argument('--private-root');args=p.parse_args();resource.setrlimit(resource.RLIMIT_CORE,(0,0));native,gateway=str(Path(args.native).resolve()),str(Path(args.gateway or args.native).resolve())
    if args.private_root:return run(native,gateway,Path(args.private_root),dict(os.environ))
    base=Path(__file__).resolve().parents[1]/'target/gnome-native-smoke';base.mkdir(parents=True,exist_ok=True);root=Path(tempfile.mkdtemp(prefix='pdf-links-ui-',dir=base));runtime.create_profile(root)
    for rotation in [0,90,180,270]:document(root/f'project/links-{rotation}.pdf',rotation=rotation)
    document(root/'project/cropped.pdf',crop=True);document(root/'project/dense.pdf',dense=True);document(root/'project/regions.pdf',dense='regions');fixture.document(root/'project/plain.pdf')
    result=api_checks(gateway,root);(root/'api.json').write_text(json.dumps(result))
    if args.api_only:print(json.dumps(dict(result,artifacts=str(root))));return
    backend=runtime.CODEX.replace("    elif method=='thread/read':","    elif method=='model/list':answer(identity,{'data':[]})\n    elif method=='thread/items/list':answer(identity,{'data':[],'nextCursor':None})\n    elif method=='thread/read':")
    for name,source in [('fake-codex.py',backend),('fake-opencode.py',runtime.OPENCODE)]:file=root/name;file.write_text(source);file.chmod(0o700)
    env=dict(os.environ,CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway,CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),CODEX_HOME=str(root/'codex'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),READER_CACHE_DIR=str(root/'reader-cache'),GDK_BACKEND='wayland',STUDIO_GNOME_SMOKE_DIR=str(root),STUDIO_GNOME_SMOKE_PDF_LINKS='1',GTK_USE_PORTAL='1')
    command=[]
    if args.headless:
        command=[shutil.which('mutter') or 'mutter','--headless','--wayland','--no-x11','--wayland-display','studio-test','--virtual-monitor','1600x1000','--'];env['STUDIO_OWNED_MUTTER_INPUT']='1'
    with isolated_environment(env,headless=args.headless) as private:subprocess.run(['dbus-run-session','--',*command,sys.executable,str(Path(__file__).resolve()),native,gateway,'--private-root',str(root)],cwd=root,env=private,check=True,timeout=210)

def run(native,gateway,root,env):
    portal=UriPortal(root);controlled=None
    with (root/'app.log').open('w') as log:
        child=subprocess.Popen([native],cwd=root,env=env,stdout=log,stderr=log);memory=ProcessMemory(child.pid,gateway,include_web_content=True)
        try:
            if env.get('STUDIO_OWNED_MUTTER_INPUT')=='1':controlled=OwnedMutterInput();time.sleep(1);controlled.focus_only_window()
            deadline=time.monotonic()+170
            while child.poll() is None:
                portal.iterate()
                pointer=root/'pdf-links-pointer.json'
                if pointer.exists():
                    request=json.loads(pointer.read_text());pointer.unlink()
                    if not controlled:raise RuntimeError('Pointer marker requires the owned compositor')
                    controlled.pointer(request['x'],request['y'],request.get('dx',0),request.get('dy',0))
                key=root/'pdf-links-key'
                if key.exists():
                    key.unlink()
                    if not controlled:raise RuntimeError('Keyboard marker requires the owned compositor')
                    for pressed in [True,False]:controlled.call('NotifyKeyboardKeysym',controlled.GLib.Variant('(ub)',(0xff0d,pressed)))
                if time.monotonic()>deadline:raise TimeoutError(('PDF links fixture timed out',root))
                if 'panicked' in (root/'app.log').read_text():raise AssertionError((root,(root/'app.log').read_text()[-3000:]))
                time.sleep(.02)
        except BaseException:child.terminate();child.wait(timeout=10);raise
        finally:
            if controlled:controlled.close()
            measure=memory.close();portal.close()
    value=json.loads((root/'pdf-links.json').read_text());assert value['ok'] and child.returncode==0,(root,value,(root/'app.log').read_text()[-4000:]);assert not runtime.starts(root)
    assert portal.requests==['https://example.invalid/docs?ref=pdf#intro','http://127.0.0.1:38669/private.pdf','https://example.invalid/docs?ref=pdf#intro'],portal.requests
    assert portal.closed,'Pending URI portal was not cancelled'
    assert measure.get('webContentProcessesObserved',0)==0,'PDF started a browser content process'
    for name in ['backend.pid','opencode.pid']:
        if (root/name).exists():assert not Path('/proc',(root/name).read_text().strip()).exists(),'Owned backend survived'
    text=(root/'app.log').read_text();assert not any(s in text for s in ['CRITICAL','panicked','Theme parser error']),(root,text[-3000:])
    print(json.dumps(dict(value,api=json.loads((root/'api.json').read_text()),uriPortalRequests=portal.requests,portalCancellation=True,memory=measure,artifacts=str(root))))

if __name__=='__main__':main()
