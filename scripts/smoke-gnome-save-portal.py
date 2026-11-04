#!/usr/bin/env python3
"""Actual SaveFile dialogs, cancelled-result isolation and immutable output exports."""
import argparse,importlib.util,json,os,resource,shutil,subprocess,sys,tempfile,time
from pathlib import Path
sys.dont_write_bytecode=True
sys.path.insert(0,str(Path(__file__).with_name('test-support')))
from private_wayland import isolated_environment
from private_file_portal import FilePortal
from memory_sampler import ProcessMemory
spec=importlib.util.spec_from_file_location('save_runtime',Path(__file__).with_name('smoke-gnome-router-runtime.py'))
runtime=importlib.util.module_from_spec(spec);spec.loader.exec_module(runtime)

def plan(root):
    save={'interface':'FileChooser','method':'SaveFile','title':'Export complete output snapshot','options':{'multiple':False,'directory':False,'current_name':'conversation-output.txt'}}
    favorites=dict(save,title='Export favorites',options={'multiple':False,'directory':False,'current_name':'codex-thread-studio-favorites.md'})
    return [dict(save,status=1),dict(save,uris=['https://example.invalid/snapshot.txt']),dict(save,uris=[]),dict(save,uris=[(root/'first-output 中文 #.txt').as_uri()]),dict(save,hold=True,lateUris=[(root/'parent-cancelled-output.txt').as_uri()]),dict(save,hold=True,lateUris=[(root/'cancelled-output.txt').as_uri()]),dict(save,hold=True,lateUris=[(root/'replaced-output.txt').as_uri()]),dict(save,uris=[(root/'confirmed-output.txt').as_uri()]),dict(favorites,status=1),dict(favorites,uris=[]),dict(favorites,hold=True,lateUris=[(root/'favorites-window-cancelled.md').as_uri()]),dict(favorites,uris=[(root/'favorites saved 中文 #.md').as_uri()]),dict(favorites,hold=True,lateUris=[(root/'favorites-closed-cancelled.md').as_uri()])]

def run(native,gateway,root):
    portal=FilePortal(root,plan(root));pending=[]
    try:
        with (root/'app.log').open('w') as log:
            child=subprocess.Popen([native],cwd=root,env=dict(os.environ),stdout=log,stderr=log);memory=ProcessMemory(child.pid,gateway)
            try:
                deadline=time.monotonic()+100
                while child.poll() is None:
                    portal.iterate()
                    if portal.failure:raise AssertionError((root,portal.failure))
                    if time.monotonic()>deadline:raise TimeoutError(('Native Save fixture timed out',root))
                    if 'panicked' in (root/'app.log').read_text():raise AssertionError((root,(root/'app.log').read_text()[-2000:]))
                    time.sleep(.015)
                portal.iterate();pending=list(portal.objects)
            except BaseException:child.terminate();child.wait(timeout=10);raise
            finally:measured=memory.close()
    finally:portal.close()
    result=json.loads((root/'save-portal-ui.json').read_text())
    assert not (root/'save-portal-close-blocked.json').exists(),(root,(root/'save-portal-close-blocked.json').read_text())
    assert child.returncode==0 and result['ok'],(root,child.returncode,result,(root/'app.log').read_text()[-2000:])
    assert not portal.failure and not portal.plan and not pending,(root,portal.failure,portal.plan,pending)
    assert len(portal.calls)==13 and len(portal.closed)==5,(root,portal.calls,portal.closed)
    assert sum(v['event']=='late-response' for v in portal.trace)==5,portal.trace
    assert (root/'first-output 中文 #.txt').read_bytes()==(root/'snapshot-a.txt').read_bytes(),'First export lost original source bytes'
    assert (root/'confirmed-output.txt').read_bytes()==(root/'snapshot-b.txt').read_bytes(),'Accepted export changed after viewer close'
    for name in ['parent-cancelled-output.txt','cancelled-output.txt','replaced-output.txt','favorites-window-cancelled.md','favorites-closed-cancelled.md']:assert (root/name).read_bytes()==b'unchanged destination\n','Cancelled chooser wrote its late result'
    favorites=(root/'favorites saved 中文 #.md').read_text()
    assert 'Saved portal favorite 中文😀' in favorites and 'Persisted note 中文😀' in favorites and 'UNSAVED portal title' not in favorites,'Library export saved an unsaved editor draft'
    assert not runtime.starts(root),'Export started a model turn'
    for name in ['backend.pid','opencode.pid']:
        if (root/name).exists():assert not Path('/proc',(root/name).read_text().strip()).exists(),'Owned provider survived'
    text=(root/'app.log').read_text();assert not any(s in text for s in ['CRITICAL','panicked','Theme parser error']),(root,text[-2000:])
    print(json.dumps(dict(result,saveCalls=len(portal.calls),cancellations=len(portal.closed),lateReplies=5,exactOriginalExports=True,cancelledDestinationsUnchanged=True,memory=measured,artifacts=str(root))))

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('native');parser.add_argument('gateway');parser.add_argument('--private-root');args=parser.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE,(0,0));native,gateway=str(Path(args.native).resolve()),str(Path(args.gateway).resolve())
    if args.private_root:return run(native,gateway,Path(args.private_root))
    base=Path(__file__).resolve().parents[1]/'target/gnome-native-smoke';base.mkdir(parents=True,exist_ok=True);root=Path(tempfile.mkdtemp(prefix='save-portal-ui-',dir=base));runtime.create_profile(root)
    for variant in ['a','b']:
        source='# Snapshot '+variant+' 中文😀\r\n'+('Literal '+variant+' Unicode 中文😀 and trailing space \r\n'*2100)+'control\0tail\r\n'
        (root/f'snapshot-{variant}.txt').write_bytes(source.encode())
    for name in ['parent-cancelled-output.txt','cancelled-output.txt','replaced-output.txt','favorites-window-cancelled.md','favorites-closed-cancelled.md']:(root/name).write_bytes(b'unchanged destination\n')
    (root/'config/codex-thread-studio/gnome.json').write_text(json.dumps({'selectedBackend':'codex','selectedSession':'target'}))
    backend=runtime.CODEX.replace("    elif method=='thread/read':","    elif method=='model/list':answer(identity,{'data':[]})\n    elif method=='thread/items/list':answer(identity,{'data':[],'nextCursor':None})\n    elif method=='thread/read':")
    for name,source in [('fake-codex.py',backend),('fake-opencode.py',runtime.OPENCODE)]:path=root/name;path.write_text(source);path.chmod(0o700)
    env=dict(os.environ,CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway,CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),CODEX_HOME=str(root/'codex'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),READER_CACHE_DIR=str(root/'reader-cache'),GDK_BACKEND='wayland',GTK_USE_PORTAL='1',GIO_USE_VFS='local',GIO_USE_VOLUME_MONITOR='unix',STUDIO_GNOME_SMOKE_DIR=str(root),STUDIO_GNOME_SMOKE_SAVE_PORTAL='1')
    compositor=[shutil.which('mutter') or 'mutter','--headless','--wayland','--no-x11','--wayland-display','studio-test','--virtual-monitor','1600x1000','--']
    with isolated_environment(env,headless=True) as private:subprocess.run(['dbus-run-session','--',*compositor,sys.executable,str(Path(__file__).resolve()),native,gateway,'--private-root',str(root)],cwd=root,env=private,check=True,timeout=130)
if __name__=='__main__':main()
