#!/usr/bin/env python3
"""Search unopened EPUB sections with bounded Rust pages and actual native controls."""
import argparse,importlib.util,json,os,resource,shutil,subprocess,sys,tempfile,time,uuid,zipfile
from pathlib import Path
sys.dont_write_bytecode=True
spec=importlib.util.spec_from_file_location('epub_search_runtime',Path(__file__).with_name('smoke-gnome-router-runtime.py'))
runtime=importlib.util.module_from_spec(spec);spec.loader.exec_module(runtime)
sys.path.insert(0,str(Path(__file__).with_name('test-support')))
from private_wayland import isolated_environment,OwnedMutterInput
from memory_sampler import ProcessMemory

def book(path,count=140,filler=0):
    with zipfile.ZipFile(path,'w',compression=zipfile.ZIP_DEFLATED) as archive:
        archive.writestr('mimetype','application/epub+zip',compress_type=zipfile.ZIP_STORED)
        archive.writestr('META-INF/container.xml','<container><rootfiles><rootfile full-path="OPS/package.opf"/></rootfiles></container>')
        items=''.join(f'<item id="c{i}" href="c{i}.xhtml" media-type="application/xhtml+xml"/>' for i in range(count))
        spine=''.join(f'<itemref idref="c{i}"/>' for i in range(count))
        archive.writestr('OPS/package.opf',f'<package><metadata><title>Whole book 中文😀</title></metadata><manifest>{items}</manifest><spine>{spine}</spine></package>')
        for i in range(count):
            content=f'<p>Section {i+1}. ordinary text</p>'
            if i>=17:content+='<p>Need<em>le</em> &amp; 中文😀 literal .+? trailing word</p>'
            if i==20:content+='<p>'+'repeat '*10002+'</p>'
            if i==21:content+='<p>NEEDLE</p>'
            if i==30:content+='<p>last-context marker 中文😀</p>'
            if filler:content+='<p>large needle context '+'x '*(filler//2)+'</p>'
            content+='<p hidden>private-canary</p><script>private-canary</script><style>.private-canary{}</style>'
            source=f'<html xmlns="http://www.w3.org/1999/xhtml"><head><title>private-canary</title></head><body><h1>Section {i+1}</h1>{content}</body></html>'.encode()
            if i==19:source=b'\xff invalid utf8'
            archive.writestr(f'OPS/c{i}.xhtml',source)

def api_checks(gateway,root):
    runtime.fixture.BACKEND=runtime.CODEX;runtime.fixture.OPENCODE_BACKEND=runtime.OPENCODE
    service=runtime.fixture.Gateway(Path(gateway),root,'codex')
    def api(operation,status=200,**fields):
        actual,value=service.api('/studio/native/epub',dict(operation=operation,**fields),method='POST',timeout=35)
        assert actual==status,(actual,status,value)
        return value
    sampler=ProcessMemory(service.process.pid,gateway)
    summary={}
    try:
        unauthorized,_=service.api('/studio/native/epub',{'operation':'search','id':str(uuid.uuid4()),'query':'needle','start':0},authorized=False,method='POST')
        assert unauthorized==401
        meta=api('open',id=str(uuid.uuid4()),root=str(root/'project'),path='sample.epub');identity=meta['documentId']
        timings=[]
        def search(query,case=False):
            start=0;sections=[];issues=[];pages=0
            while start is not None:
                at=time.monotonic();page=api('search',id=identity,query=query,caseSensitive=case,start=start);timings.append(round((time.monotonic()-at)*1000,2));pages+=1
                assert page['start']==start and 0<page['scanned']<=16 and len(page['sections'])<=16 and len(json.dumps(page).encode())<160000
                sections+=page['sections'];issues+=page['issues'];start=page['next']
            return sections,issues,pages
        sections,issues,pages=search('needle');assert pages>=9 and len(sections)==122 and [s['chapter'] for s in sections]==[i for i in range(17,140) if i!=19];assert len(issues)==1 and issues[0]['chapter']==19
        exact,_,_=search('needle',True);assert exact==[]
        trailing,_,_=search('needle ');assert len(trailing)==122
        entity,_,_=search('& 中文😀');assert len(entity)==122 and 'Needle & 中文😀' in entity[0]['snippet']
        literal,_,_=search('.+?');assert len(literal)==122
        secret,_,_=search('private-canary');assert secret==[]
        repeat,_,_=search('repeat ');assert len(repeat)==1 and repeat[0]['count']==10000 and repeat[0]['limited']
        first=api('search',id=identity,query='needle',start=0);assert first['sections']==[] and first['next']==16
        for query in ['', ' '*3,'x'*513,'bad\0query']:api('search',status=400,id=identity,query=query,start=0)
        api('search',status=400,id=identity,query='needle',start=140)
        api('search',status=400,id=identity,query='needle',start=-1)
        api('search',status=400,id=identity,query='needle',start=0,unexpected=True)
        api('close',id=identity);api('search',status=410,id=identity,query='needle',start=0)
        assert not runtime.starts(root)
        summary={'pages':pages,'matchingSections':len(sections),'maximumRequestMs':max(timings),'skippedSections':len(issues)}
        large=api('open',id=str(uuid.uuid4()),root=str(root/'project'),path='large-search.epub');identity=large['documentId']
        before=len(timings);large_sections,large_issues,large_pages=search('large needle context')
        assert len(large_sections)==6 and not large_issues and large_pages>=2
        summary['large']={'fixtureFillerBytes':6*4*1024*1024,'pages':large_pages,'maximumRequestMs':max(timings[before:])}
        api('close',id=identity)
        references=[]
        for path in sorted((root/'project').glob('reference-*.epub')):
            meta=api('open',id=str(uuid.uuid4()),root=str(root/'project'),path=path.name);identity=meta['documentId'];before=len(timings)
            hits,skipped,pages=search('the');assert hits,(path,skipped)
            references.append({'file':path.name,'spine':len(meta['spine']),'matchingSections':len(hits),'pages':pages,'skipped':len(skipped),'maximumRequestMs':max(timings[before:])});api('close',id=identity)
        summary['references']=references
        return summary
    finally:
        sampled=sampler.close();summary['gatewayObservedPeakRssKiB']=sampled['nativePeakRssKiB'];service.close()

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('native');p.add_argument('gateway',nargs='?');p.add_argument('--api-only',action='store_true');p.add_argument('--reference',action='append',default=[]);p.add_argument('--headless',action='store_true');p.add_argument('--private-root');args=p.parse_args();resource.setrlimit(resource.RLIMIT_CORE,(0,0))
    native,gateway=str(Path(args.native).resolve()),str(Path(args.gateway or args.native).resolve())
    if args.private_root:return run(native,gateway,Path(args.private_root),dict(os.environ))
    base=Path(__file__).resolve().parents[1]/'target/gnome-native-smoke';base.mkdir(parents=True,exist_ok=True);root=Path(tempfile.mkdtemp(prefix='epub-search-ui-',dir=base));runtime.create_profile(root);book(root/'project/sample.epub');book(root/'project/replacement.epub',count=3);book(root/'project/large-search.epub',count=6,filler=4*1024*1024)
    for index,source in enumerate(args.reference):shutil.copyfile(Path(source).resolve(),root/f'project/reference-{index}.epub')
    result=api_checks(gateway,root);(root/'api.json').write_text(json.dumps(result))
    if args.api_only:print(json.dumps(dict(result,ok=True,artifacts=str(root))));return
    code=runtime.CODEX.replace("    elif method=='thread/read':","    elif method=='model/list':answer(identity,{'data':[]})\n    elif method=='thread/items/list':answer(identity,{'data':[],'nextCursor':None})\n    elif method=='thread/read':")
    for name,source in [('fake-codex.py',code),('fake-opencode.py',runtime.OPENCODE)]:path=root/name;path.write_text(source);path.chmod(0o700)
    env=dict(os.environ,CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway,CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),CODEX_HOME=str(root/'codex'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),READER_CACHE_DIR=str(root/'reader-cache'),GDK_BACKEND='wayland',STUDIO_GNOME_SMOKE_DIR=str(root),STUDIO_GNOME_SMOKE_EPUB_SEARCH='1')
    command=[]
    if args.headless:
        mutter=shutil.which('mutter')
        if not mutter:raise RuntimeError('Mutter is required for --headless')
        command=[mutter,'--headless','--wayland','--no-x11','--wayland-display','studio-test','--virtual-monitor','1600x1000','--'];env['STUDIO_OWNED_MUTTER_INPUT']='1'
    with isolated_environment(env,headless=args.headless) as private:subprocess.run(['dbus-run-session','--',*command,sys.executable,str(Path(__file__).resolve()),native,gateway,'--private-root',str(root)],env=private,cwd=root,check=True,timeout=180)

def run(native,gateway,root,env):
    with (root/'app.log').open('w') as log:
        child=subprocess.Popen([native],cwd=root,env=env,stdout=log,stderr=log);memory=ProcessMemory(child.pid,gateway,include_web_content=True);controlled=None
        try:
            if env.get('STUDIO_OWNED_MUTTER_INPUT')=='1':controlled=OwnedMutterInput();time.sleep(1);controlled.focus_only_window()
            deadline=time.monotonic()+145
            while child.poll() is None:
                if time.monotonic()>deadline:raise TimeoutError(('EPUB search test timed out',root))
                if 'panicked' in (root/'app.log').read_text():raise AssertionError((root,(root/'app.log').read_text()[-3000:]))
                time.sleep(.03)
        except BaseException:child.terminate();child.wait(timeout=10);raise
        finally:
            if controlled:controlled.close()
            measure=memory.close()
    value=json.loads((root/'epub-search.json').read_text());assert child.returncode==0 and value['ok'],(root,value,(root/'app.log').read_text()[-3000:])
    assert not runtime.starts(root),'Search started a model turn'
    for name in ['backend.pid','opencode.pid']:
        if (root/name).exists():assert not Path('/proc',(root/name).read_text().strip()).exists(),'Owned backend survived'
    text=(root/'app.log').read_text();assert not any(s in text for s in ['CRITICAL','panicked','Theme parser error']),(root,text[-3000:])
    print(json.dumps(dict(value,api=json.loads((root/'api.json').read_text()),memory=measure,artifacts=str(root))))

if __name__=='__main__':main()
