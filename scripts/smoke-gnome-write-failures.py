#!/usr/bin/env python3
"""Actual failed exports after viewer dismissal, and admitted local-write identity."""
import argparse,importlib.util,json,os,resource,shutil,sqlite3,subprocess,sys,tempfile,time
from pathlib import Path
sys.dont_write_bytecode=True
sys.path.insert(0,str(Path(__file__).with_name('test-support')))
from private_wayland import isolated_environment,OwnedMutterInput
from private_file_portal import FilePortal
from memory_sampler import ProcessMemory
spec=importlib.util.spec_from_file_location('failure_runtime',Path(__file__).with_name('smoke-gnome-router-runtime.py'))
runtime=importlib.util.module_from_spec(spec);spec.loader.exec_module(runtime)
OUTPUTS=['failed-output-closed','failed-output-replaced','failed-output-queue-a','failed-output-queue-b']

def plan(root):
    output=dict(interface='FileChooser',method='SaveFile',title='Export complete output snapshot',options=dict(multiple=False,directory=False,current_name='conversation-output.txt'))
    favorite=dict(output,title='Export favorites',options=dict(multiple=False,directory=False,current_name='codex-thread-studio-favorites.md'))
    return [dict(output,uris=[(root/(name+'.txt')).as_uri()]) for name in OUTPUTS]+[dict(favorite,uris=[(root/(name+'.md')).as_uri()]) for name in ['failed-favorites','interrupted-favorites']]

def run(native,gateway,root):
    controlled=OwnedMutterInput()
    portal=FilePortal(root,plan(root));pending=[]
    try:
        with (root/'app.log').open('w') as log:
            child=subprocess.Popen([native],cwd=root,env=dict(os.environ),stdout=log,stderr=log);memory=ProcessMemory(child.pid,gateway)
            try:
                deadline=time.monotonic()+110
                while child.poll() is None:
                    portal.iterate()
                    controlled.respond(root)
                    if portal.failure:raise AssertionError((root,portal.failure))
                    if time.monotonic()>deadline:raise TimeoutError(('Write failure fixture timed out',root))
                    if 'panicked' in (root/'app.log').read_text():raise AssertionError((root,(root/'app.log').read_text()[-2000:]))
                    time.sleep(.015)
                portal.iterate();pending=list(portal.objects)
            except BaseException:child.terminate();child.wait(timeout=10);raise
            finally:measured=memory.close()
    finally:portal.close();controlled.close()
    result=json.loads((root/'write-failures.json').read_text())
    assert child.returncode==0 and result['ok'],(root,child.returncode,result,(root/'app.log').read_text()[-2000:])
    assert not portal.failure and not portal.plan and not pending,(root,portal.failure,portal.plan,pending)
    assert len(portal.calls)==6 and not portal.closed,(root,portal.calls,portal.closed)
    for destination in [*(name+'.txt' for name in OUTPUTS),'failed-favorites.md']:
        assert (root/destination/'sentinel.txt').read_bytes()==b'unchanged directory destination\n','Failed export modified the destination'
    for name in [*OUTPUTS,'failed-favorites','interrupted-favorites']:assert (root/(name+'-started')).is_file(),'Write did not reach its admitted phase'
    for pattern in ['.tmp*','.goutputstream-*']:assert not list(root.glob(pattern)),'Failed export leaked its temporary file'
    saved=(root/'interrupted-favorites.md').read_text()
    assert 'Write lifecycle favorite 中文😀' in saved and 'Persisted lifecycle note' in saved and 'UNSAVED failure draft' not in saved
    with sqlite3.connect(root/'config/codex-thread-studio/studio.sqlite3') as db:
        rows=db.execute('SELECT title,note FROM favorites').fetchall()
    assert rows==[('Write lifecycle favorite 中文😀','Persisted lifecycle note')],rows
    assert not runtime.starts(root),'Write acceptance unexpectedly sent a model turn'
    for name in ['backend.pid','opencode.pid']:
        if (root/name).exists():assert not Path('/proc',(root/name).read_text().strip()).exists(),'Owned provider survived'
    text=(root/'app.log').read_text();assert not any(s in text for s in ['CRITICAL','panicked','Theme parser error','Gtk-WARNING']),(root,text[-2000:])
    print(json.dumps(dict(result,saveCalls=len(portal.calls),realDiskFailures=5,destinationSentinelsUnchanged=True,temporaryFilesCleaned=True,sourceMetadataUnchanged=True,memory=measured,artifacts=str(root))))

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('native');parser.add_argument('gateway');parser.add_argument('--private-root');args=parser.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE,(0,0));native,gateway=str(Path(args.native).resolve()),str(Path(args.gateway).resolve())
    if args.private_root:return run(native,gateway,Path(args.private_root))
    base=Path(__file__).resolve().parents[1]/'target/gnome-native-smoke';base.mkdir(exist_ok=True,parents=True);root=Path(tempfile.mkdtemp(prefix='write-failures-',dir=base));runtime.create_profile(root)
    for variant in ['a','b']:
        (root/f'snapshot-{variant}.txt').write_bytes(('# Snapshot '+variant+' 中文😀\r\n'+('Literal '+variant+' Unicode 中文😀 and trailing space \r\n'*2000)+'control\0tail\r\n').encode())
    for destination in [*(name+'.txt' for name in OUTPUTS),'failed-favorites.md']:
        (root/destination).mkdir();(root/destination/'sentinel.txt').write_bytes(b'unchanged directory destination\n')
    (root/'interrupted-favorites.md').write_text('original destination\n')
    (root/'config/codex-thread-studio/gnome.json').write_text(json.dumps(dict(selectedBackend='codex',selectedSession='target')))
    codex=runtime.CODEX.replace("    elif method=='thread/read':","    elif method=='model/list':answer(identity,{'data':[]})\n    elif method=='thread/items/list':answer(identity,{'data':[],'nextCursor':None})\n    elif method=='thread/read':")
    for name,source in [('fake-codex.py',codex),('fake-opencode.py',runtime.OPENCODE)]:path=root/name;path.write_text(source);path.chmod(0o700)
    env=dict(os.environ,CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway,CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),CODEX_HOME=str(root/'codex'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),READER_CACHE_DIR=str(root/'reader-cache'),GDK_BACKEND='wayland',GTK_USE_PORTAL='1',GIO_USE_VFS='local',GIO_USE_VOLUME_MONITOR='unix',STUDIO_GNOME_SMOKE_DIR=str(root),STUDIO_GNOME_SMOKE_WRITE_FAILURES='1',STUDIO_OWNED_MUTTER_INPUT='1')
    compositor=[shutil.which('mutter') or 'mutter','--headless','--wayland','--no-x11','--wayland-display','studio-test','--virtual-monitor','1600x1000','--']
    with isolated_environment(env,headless=True) as private:subprocess.run(['dbus-run-session','--',*compositor,sys.executable,str(Path(__file__).resolve()),native,gateway,'--private-root',str(root)],cwd=root,env=private,check=True,timeout=140)
if __name__=='__main__':main()
