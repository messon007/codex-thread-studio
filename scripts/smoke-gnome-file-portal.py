#!/usr/bin/env python3
"""Exercise native file chooser and FD handoff against private XDG portal services."""
import argparse,fnmatch,hashlib,importlib.util,json,os,resource,shutil,subprocess,sys,tempfile,time
from pathlib import Path
sys.dont_write_bytecode=True
sys.path.insert(0,str(Path(__file__).with_name('test-support')))
from private_wayland import isolated_environment
from private_file_portal import FilePortal
from memory_sampler import ProcessMemory
spec=importlib.util.spec_from_file_location('file_portal_runtime',Path(__file__).with_name('smoke-gnome-router-runtime.py'))
runtime=importlib.util.module_from_spec(spec);spec.loader.exec_module(runtime)

def plan(root):
    chooser={'interface':'FileChooser','method':'OpenFile','title':'Open Markdown','options':{'directory':False,'multiple':False}}
    folder={'interface':'FileChooser','method':'OpenFile','title':'Add shared folder','options':{'directory':True,'multiple':False}}
    shell=dict(chooser,title='Open document')
    shell_cases=[dict(shell,status=1),dict(shell,uris=[(root/'project/picked.rs').as_uri()]),dict(shell,uris=[]),dict(shell,uris=['https://example.invalid/remote.txt']),dict(shell,hold=True,lateUris=[(root/'project/picked.rs').as_uri()]),dict(shell,hold=True,lateUris=[(root/'project/picked.rs').as_uri()]),dict(shell,hold=True)]
    shared=root/'picked shared 中文 # '
    return [dict(chooser,status=1),dict(chooser,uris=[(root/'project/selected 中文 #.md').as_uri()]),dict(chooser,uris=[]),dict(chooser,uris=['https://example.invalid/remote.md']),*[dict(chooser,hold=True) for _ in range(3)],*shell_cases,dict(folder,uris=[shared.as_uri()]),dict(folder,hold=True),{'interface':'OpenURI','method':'OpenDirectory','path':str(shared/'notes.md'),'options':{'writable':False}},{'interface':'OpenURI','method':'OpenFile','path':str(root/'project'),'options':{'writable':False}}]

def run(native,gateway,root):
    portal=FilePortal(root,plan(root));pending=[]
    try:
        with (root/'app.log').open('w') as log:
            child=subprocess.Popen([native],cwd=root,env=dict(os.environ),stdout=log,stderr=log)
            memory=ProcessMemory(child.pid,gateway)
            try:
                deadline=time.monotonic()+140
                while child.poll() is None:
                    portal.iterate()
                    if portal.failure:raise AssertionError((root,portal.failure))
                    if time.monotonic()>deadline:raise TimeoutError(('Native file portal fixture timed out',root))
                    if 'panicked' in (root/'app.log').read_text():raise AssertionError((root,(root/'app.log').read_text()[-2000:]))
                    time.sleep(.015)
                portal.iterate();pending=list(portal.objects)
            except BaseException:child.terminate();child.wait(timeout=10);raise
            finally:measured=memory.close()
    finally:portal.close()
    result=json.loads((root/'file-portal-ui.json').read_text())
    assert child.returncode==0 and result['ok'],(root,child.returncode,result,(root/'app.log').read_text()[-1800:])
    assert not portal.failure and not portal.plan and not pending,(root,portal.failure,portal.plan,pending)
    assert len(portal.calls)==18 and len(portal.closed)==7,(root,portal.calls,portal.closed)
    for call in portal.calls[:7]:
        patterns={pattern for name,entries in call['options'].get('filters',[]) for kind,pattern in entries if kind==0}
        assert '*.md' in patterns and '*.MD' in patterns,(root,call)
    for call in portal.calls[7:14]:
        patterns={pattern for name,entries in call['options'].get('filters',[]) for kind,pattern in entries if kind==0}
        filters=call['options'].get('filters',[])
        assert len(filters)==2 and '*' in patterns,(root,call)
        assert all(any(kind==0 and fnmatch.fnmatchcase(name,pattern) for kind,pattern in filters[0][1]) for name in ['notes.md','NOTES.MD']),(root,call)
    assert sum(t.get('event')=='late-response' for t in portal.trace)==2,(root,portal.trace)
    for relative,expected in json.loads((root/'original-file-hashes.json').read_text()).items():
        assert hashlib.sha256((root/relative).read_bytes()).hexdigest()==expected,'Opening changed original bytes'
    assert not runtime.starts(root),'File operations started a model turn'
    for name in ['backend.pid','opencode.pid']:
        if (root/name).exists():assert not Path('/proc',(root/name).read_text().strip()).exists(),'Owned provider survived'
    text=(root/'app.log').read_text();assert not any(s in text for s in ['CRITICAL','panicked','Theme parser error']),(root,text[-2000:])
    print(json.dumps(dict(result,portalCalls=len(portal.calls),portalCancellations=len(portal.closed),parentedWaylandRequests=True,fdTargetsConfined=True,memory=measured,artifacts=str(root))))

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('native');parser.add_argument('gateway');parser.add_argument('--private-root');args=parser.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE,(0,0));native,gateway=str(Path(args.native).resolve()),str(Path(args.gateway).resolve())
    if args.private_root:return run(native,gateway,Path(args.private_root))
    base=Path(__file__).resolve().parents[1]/'target/gnome-native-smoke';base.mkdir(parents=True,exist_ok=True);root=Path(tempfile.mkdtemp(prefix='file-portal-ui-',dir=base));runtime.create_profile(root)
    (root/'project/selected 中文 #.md').write_bytes('# Selected 中文😀\r\n\r\nNative portal content.\r\n'.encode())
    (root/'project/alternate.md').write_text('# Alternate\n\nOpened while a chooser was pending.\n')
    shared=root/'picked shared 中文 # ';shared.mkdir();(shared/'notes.md').write_text('# Shared file\n\nRead only outside the project.\n')
    (root/'project/picked.rs').write_text('fn main() { println!("native 中文😀"); }\n')
    (root/'other directory').mkdir();(root/'other directory/picked.sh').write_text('#!/bin/sh\nprintf "native 中文😀\\n"\n')
    originals={str(p.relative_to(root)):hashlib.sha256(p.read_bytes()).hexdigest() for p in [root/'project/selected 中文 #.md',root/'project/alternate.md',shared/'notes.md',root/'project/picked.rs',root/'other directory/picked.sh']}
    (root/'original-file-hashes.json').write_text(json.dumps(originals))
    (root/'config/codex-thread-studio/gnome.json').write_text(json.dumps({'selectedBackend':'codex','selectedSession':'target'}))
    backend=runtime.CODEX.replace("    elif method=='thread/read':","    elif method=='model/list':answer(identity,{'data':[]})\n    elif method=='thread/items/list':answer(identity,{'data':[],'nextCursor':None})\n    elif method=='thread/read':")
    for name,code in [('fake-codex.py',backend),('fake-opencode.py',runtime.OPENCODE)]:p=root/name;p.write_text(code);p.chmod(0o700)
    env=dict(os.environ,CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway,CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),CODEX_HOME=str(root/'codex'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),READER_CACHE_DIR=str(root/'reader-cache'),GDK_BACKEND='wayland',GTK_USE_PORTAL='1',GIO_USE_VFS='local',GIO_USE_VOLUME_MONITOR='unix',STUDIO_GNOME_SMOKE_DIR=str(root),STUDIO_GNOME_SMOKE_FILE_PORTAL='1')
    command=[shutil.which('mutter') or 'mutter','--headless','--wayland','--no-x11','--wayland-display','studio-test','--virtual-monitor','1600x1000','--']
    with isolated_environment(env,headless=True) as private:
        subprocess.run(['dbus-run-session','--',*command,sys.executable,str(Path(__file__).resolve()),native,gateway,'--private-root',str(root)],cwd=root,env=private,check=True,timeout=170)
if __name__=='__main__':main()
