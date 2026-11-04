#!/usr/bin/env python3
"""Native lazy tree, bounded directory pages and real filesystem notifications.

Only private fixture files and fake providers; no model prompts or user-file writes.
"""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import resource
import shutil
import subprocess
import sys
import tempfile
import time
sys.dont_write_bytecode=True
sys.path.insert(0,str(Path(__file__).with_name('test-support')))
from memory_sampler import ProcessMemory
from private_wayland import isolated_environment, OwnedMutterInput
spec=importlib.util.spec_from_file_location('files_fixture',Path(__file__).with_name('smoke-gnome-router-runtime.py'))
runtime=importlib.util.module_from_spec(spec);spec.loader.exec_module(runtime)

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('native');parser.add_argument('gateway');parser.add_argument('--api-only',action='store_true');parser.add_argument('--large-folder',action='store_true');parser.add_argument('--shared',action='store_true');parser.add_argument('--headless',action='store_true');parser.add_argument('--private-root');args=parser.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE,(0,0))
    native,gateway=map(lambda p:str(Path(p).resolve()),[args.native,args.gateway])
    if args.private_root:return run_native(native,gateway,Path(args.private_root),dict(os.environ))
    base=Path(__file__).resolve().parents[1]/'target/gnome-native-smoke';base.mkdir(parents=True,exist_ok=True)
    root=Path(tempfile.mkdtemp(prefix='files-ui-',dir=base));runtime.create_profile(root)
    project=root/'project';(project/'nested').mkdir();(project/'nested/note.md').write_text('# Nested note\n')
    (root/'outside.txt').write_text('outside selected project')
    (project/'outside-link.txt').symlink_to(root/'outside.txt')
    api_project=root/'api-project';api_project.mkdir()
    (api_project/'folder').mkdir();(api_project/'.gitignore').write_text('*.tmp\n!keep.tmp\n')
    (api_project/'.ignore').write_text('custom.log\n')
    for name in ['ignored.tmp','keep.tmp','custom.log','.hidden','back\\slash.md']:(api_project/name).write_text(name)
    for n in range(240):(api_project/f'file-{n:03}.md').write_text('# Page\n')
    (api_project/'escape').symlink_to(root/'outside.txt');(api_project/'loop').symlink_to(api_project)
    shared=root/'shared docs 中文 #';shared.mkdir();(shared/'nested').mkdir();shared_file=shared/'notes.md';shared_file.write_text('# Shared notes\n\nRead-only 中文😀.\n')
    (shared/'escape.txt').symlink_to(root/'outside.txt')
    settings=root/'config/codex-thread-studio/settings.json';preferences=json.loads(settings.read_text());preferences['sharedDocumentDirectories']=[str(shared/'nested'),str(shared),str(shared)];settings.write_text(json.dumps(preferences))
    runtime.fixture.BACKEND=runtime.CODEX;runtime.fixture.OPENCODE_BACKEND=runtime.OPENCODE
    service=runtime.fixture.Gateway(Path(gateway),root,'codex')
    def listing(**options):return service.api('/studio/native/files',{'root':str(api_project),**options})
    try:
        assert service.api('/studio/native/files',{'root':str(api_project)},authorized=False)[0]==401
        code,first=listing();assert code==200,(code,first)
        assert len(first['entries'])==200 and first['entries'][0]['kind']=='directory' and first['total']==243 and not first['scanTruncated'],first
        code,tail=listing(cursor=first['nextCursor']);assert code==200 and len(tail['entries'])==43 and tail['nextCursor'] is None,(code,tail)
        entries=first['entries']+tail['entries'];names={e['name'] for e in entries}
        assert 'keep.tmp' in names and 'back\\slash.md' in names and not names.intersection({'ignored.tmp','custom.log','.hidden','escape','loop'})
        assert next(e for e in entries if e['name']=='back\\slash.md')['path']=='back\\slash.md'
        assert listing(query='file-239.md ')[1]['total']==1,'Trailing whitespace was trimmed'
        assert listing(query='FILE-239.md')[1]['total']==2
        assert listing(showHidden=True,showIgnored=True)[1]['total']==248
        assert listing(path='..')[0]==403
        assert listing(path=str(api_project))[0]==400
        assert listing(query='x'*257)[0]==400
        assert listing(cursor='invalid')[0]==400
        (api_project/'changed.md').write_text('# Changed\n')
        assert listing(cursor=first['nextCursor'])[0]==409
        for route in ['/studio/native/files/roots','/studio/native/files/location']:
            assert service.api(route,{'root':str(api_project),'path':''} if route.endswith('location') else {'root':str(api_project)},authorized=False)[0]==401
        code,scopes=service.api('/studio/native/files/roots',{'root':str(api_project)});assert code==200 and len(scopes['roots'])==2 and scopes['roots'][1]['readOnly'],scopes
        code,page=listing(path=str(shared));assert code==200 and page['readOnly'] and all(e['readOnly'] for e in page['entries'])
        assert not any(e['name']=='escape.txt' for e in page['entries'])
        code,location=service.api('/studio/native/files/location',{'root':str(api_project),'path':str(shared_file)});assert code==200 and location['readOnly'] and not location['directory'] and location['path']==str(shared_file)
        assert service.api('/studio/native/files/location',{'root':str(api_project),'path':str(root/'outside.txt')})[0]==403
        code,document=service.api('/studio/review-file',{'root':str(api_project),'path':str(shared_file)});assert code==200 and document['readOnly'] and document['root']==str(api_project)
        assert service.api('/studio/workspace/save',{'root':str(api_project),'path':str(shared_file),'content':'Must not save','expectedHash':document['hash']})[0]==403
        assert shared_file.read_text()=='# Shared notes\n\nRead-only 中文😀.\n'
        current=service.api('/studio/preferences')[1];assert service.api('/studio/preferences',{**current,'sharedDocumentDirectories':[]},method='PUT')[0]==200
        assert listing(path=str(shared))[0]==403
        assert service.api('/studio/native/files/location',{'root':str(api_project),'path':str(shared_file)})[0]==403
        assert len(service.api('/studio/native/files/roots',{'root':str(api_project)})[1]['roots'])==1
        assert service.api('/studio/preferences',current,method='PUT')[0]==200
        if args.large_folder:
            (api_project/'many').mkdir()
            for n in range(10020):(api_project/'many'/f'file-{n:05}.md').write_text('# Large directory\n')
            started=time.monotonic();code,large=listing(path='many')
            assert code==200 and len(large['entries'])==200 and large['scanTruncated'] and 200<=large['total']<=10000,(code,large)
            api_measurement={'sourceEntries':10020,'pageEntries':len(large['entries']),'retainedEntries':large['total'],'scanTruncated':True,'endToEndMs':round((time.monotonic()-started)*1000),'scanMs':large['elapsedMs']}
        assert not runtime.starts(root),'Listing triggered a model prompt'
    finally:service.close()
    if args.api_only:print(json.dumps({'ok':True,'directoryApi':True,'largeDirectory':api_measurement if args.large_folder else None,'artifacts':str(root)}));return
    backend=runtime.CODEX.replace("    elif method=='thread/list':","    elif method=='model/list':answer(identity,{'data':[]})\n    elif method=='thread/list':",1)
    for name,source in [('fake-codex.py',backend),('fake-opencode.py',runtime.OPENCODE)]:
        path=root/name;path.write_text(source);path.chmod(0o700)
    environment=dict(os.environ,CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway,CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),CODEX_HOME=str(root/'codex'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),READER_CACHE_DIR=str(root/'reader-cache'),GDK_BACKEND='wayland',STUDIO_GNOME_SMOKE_DIR=str(root),STUDIO_GNOME_SMOKE_FILES='1')
    environment.pop('DISPLAY',None);environment.pop('XAUTHORITY',None)
    if args.large_folder:environment['STUDIO_GNOME_SMOKE_FILES_LARGE']='1'
    if args.shared:environment.update(STUDIO_GNOME_SMOKE_FILES_SHARED='1',STUDIO_GNOME_SMOKE_REVEAL='1')
    (root/'files-api.json').write_text(json.dumps({'largeDirectory':api_measurement if args.large_folder else None}))
    if args.headless:
        environment['STUDIO_OWNED_MUTTER_INPUT']='1'
        command=[shutil.which('mutter') or 'mutter','--headless','--wayland','--no-x11','--wayland-display','studio-test','--virtual-monitor','1600x1000','--']
        with isolated_environment(environment,headless=True) as private:
            subprocess.run(['dbus-run-session','--',*command,sys.executable,str(Path(__file__).resolve()),native,gateway,'--private-root',str(root)],cwd=root,env=private,check=True,timeout=130)
        return
    return run_native(native,gateway,root,environment)

def run_native(native,gateway,root,environment):
    controlled=OwnedMutterInput() if environment.get('STUDIO_OWNED_MUTTER_INPUT') else None
    if controlled:controlled.enable_clipboard_reader()
    try:
        with (root/'app.log').open('w') as log:
            child=subprocess.Popen([native],cwd=root,env=environment,stdout=log,stderr=log)
            observer=ProcessMemory(child.pid,gateway)
            try:
                deadline=time.monotonic()+100
                while child.poll() is None:
                    if controlled:controlled.respond(root)
                    if time.monotonic()>deadline:raise TimeoutError(f'Files test timed out: {root}')
                    if 'panicked' in (root/'app.log').read_text():raise AssertionError((root,(root/'app.log').read_text()[-2000:]))
                    time.sleep(.02)
            except BaseException:child.terminate();child.wait(timeout=10);raise
            finally:memory=observer.close()
    finally:
        if controlled:controlled.close()
    assert child.returncode==0,(root,child.returncode)
    result=json.loads((root/'files-ui.json').read_text());assert result['ok'],(root,result)
    assert all(result[key] for key in ['liveLanguage','literalRowBindings','menuTargetPreserved']), result
    log=(root/'app.log').read_text()
    assert not any(marker in log for marker in ['Gtk-CRITICAL','GLib-GObject-CRITICAL','panicked at']),root
    assert not runtime.starts(root),'Files controls triggered a model prompt'
    for name in ['backend.pid','opencode.pid']:
        if not (root/name).exists():continue
        identity=(root/name).read_text().strip()
        for _ in range(100):
            if not Path('/proc',identity).exists():break
            time.sleep(.05)
        assert not Path('/proc',identity).exists(),f'Owned provider {identity} survived Files test'
    print(json.dumps({'ok':True,'directoryApi':True,'nativeControls':result,'largeDirectory':json.loads((root/'files-api.json').read_text())['largeDirectory'],'observedMemory':memory,'backendCleanup':True,'artifacts':str(root)}))

if __name__=='__main__':main()
