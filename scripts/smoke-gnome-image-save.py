#!/usr/bin/env python3
"""Original PNG/SVG/GIF bytes and native Open/Save lifecycle on owned Wayland."""
import argparse,base64,hashlib,importlib.util,json,os,resource,shutil,subprocess,sys,tempfile,time
from pathlib import Path
sys.dont_write_bytecode=True
sys.path.insert(0,str(Path(__file__).with_name('test-support')))
from private_wayland import isolated_environment,OwnedMutterInput
from private_file_portal import FilePortal
from memory_sampler import ProcessMemory
spec=importlib.util.spec_from_file_location('image_save_fixture',Path(__file__).with_name('smoke-gnome-images.py'))
fixture=importlib.util.module_from_spec(spec);spec.loader.exec_module(fixture)

def plan(root):
    save=dict(interface='FileChooser',method='SaveFile',title='Save original image',options=dict(multiple=False,directory=False,current_name='sample.png'))
    svg=dict(save,options=dict(save['options'],current_name='vector.svg'))
    gif=dict(save,options=dict(save['options'],current_name='image.gif'))
    opened=dict(interface='FileChooser',method='OpenFile',title='Open image',options=dict(multiple=False,directory=False))
    uri=lambda name:(root/name).as_uri()
    return [dict(save,status=1),dict(save,uris=['https://example.invalid/image.png']),dict(save,uris=[]),dict(save,uris=[uri('saved original 中文 #.png')]),
        dict(save,hold=True,lateUris=[uri('cancelled.png')]),dict(save,hold=True,lateUris=[uri('replaced.png')]),dict(svg,uris=[uri('original-vector.svg')]),
        dict(save,uris=[uri('admitted-original.png')]),dict(svg,hold=True,lateUris=[uri('parent-cancelled.svg')]),dict(gif,uris=[uri('inline-original.gif')]),
        dict(save,uris=[uri('changed-source.png')]),dict(save,uris=[uri('failed-original.png')]),dict(opened,status=1),
        dict(opened,hold=True,lateUris=[uri('project/vector.svg')]),dict(opened,hold=True,lateUris=[uri('project/sample.png')]),dict(opened,uris=[uri('project/sample.png')])]

def run(native,gateway,root):
    controlled=OwnedMutterInput();portal=FilePortal(root,plan(root));pending=[]
    try:
        with (root/'app.log').open('w') as log:
            child=subprocess.Popen([native],cwd=root,env=dict(os.environ),stdout=log,stderr=log);memory=ProcessMemory(child.pid,gateway)
            try:
                deadline=time.monotonic()+125
                while child.poll() is None:
                    portal.iterate();controlled.respond(root)
                    if portal.failure:raise AssertionError((root,portal.failure))
                    if time.monotonic()>deadline:raise TimeoutError(('Image Save fixture timed out',root))
                    if 'panicked' in (root/'app.log').read_text():raise AssertionError((root,(root/'app.log').read_text()[-2000:]))
                    time.sleep(.015)
                portal.iterate();pending=list(portal.objects)
            except BaseException:child.terminate();child.wait(timeout=10);raise
            finally:measured=memory.close()
    finally:portal.close();controlled.close()
    result=json.loads((root/'image-save.json').read_text())
    assert child.returncode==0 and result['ok'],(root,child.returncode,result,(root/'app.log').read_text()[-2000:])
    assert not portal.failure and not portal.plan and not pending,(root,portal.failure,portal.plan,pending)
    assert len(portal.calls)==16 and len(portal.closed)==5,(root,len(portal.calls),portal.closed)
    assert sum(v['event']=='late-response' for v in portal.trace)==5
    expected=(root/'expected.png').read_bytes()
    assert (root/'project/sample.png').read_bytes()==expected
    for name in ['saved original 中文 #.png','admitted-original.png']:assert (root/name).read_bytes()==expected,'Original PNG was resampled/reencoded'
    assert (root/'original-vector.svg').read_bytes()==(root/'project/vector.svg').read_bytes()
    assert (root/'inline-original.gif').read_bytes()==(root/'inline.gif').read_bytes()
    for name in ['cancelled.png','replaced.png','parent-cancelled.svg','changed-source.png']:assert (root/name).read_bytes()==b'unchanged destination\n'
    assert (root/'failed-original.png/sentinel').read_bytes()==b'unchanged directory\n'
    assert not list(root.glob('.tmp*')),'Original save left temporary files'
    assert not fixture.runtime.starts(root),'Image Save started a model turn'
    for name in ['backend.pid','opencode.pid']:
        if (root/name).exists():assert not Path('/proc',(root/name).read_text().strip()).exists(),'Owned provider survived'
    text=(root/'app.log').read_text();assert not any(s in text for s in ['CRITICAL','panicked','Theme parser error','Gtk-WARNING']),(root,text[-2000:])
    print(json.dumps(dict(result,saveCalls=12,openCalls=4,cancellations=5,lateReplies=5,exactOriginalBytes=True,unchangedDestinations=True,temporaryCleanup=True,memory=measured,artifacts=str(root))))

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('native');parser.add_argument('gateway');parser.add_argument('--private-root');args=parser.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE,(0,0));native,gateway=str(Path(args.native).resolve()),str(Path(args.gateway).resolve())
    if args.private_root:return run(native,gateway,Path(args.private_root))
    base=Path(__file__).resolve().parents[1]/'target/gnome-native-smoke';base.mkdir(exist_ok=True,parents=True);root=Path(tempfile.mkdtemp(prefix='image-save-',dir=base));fixture.runtime.create_profile(root)
    original=fixture.png(3000,1000);(root/'expected.png').write_bytes(original);(root/'project/sample.png').write_bytes(original);(root/'changed.png').write_bytes(fixture.png(4,3))
    (root/'project/vector.svg').write_text('<svg xmlns="http://www.w3.org/2000/svg" width="400" height="240"><rect width="400" height="240" fill="#008866"/><text x="30" y="100" font-size="34" fill="white">Original SVG 中文</text></svg>')
    (root/'inline.gif').write_bytes(base64.b64decode('R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAICRAEAOw=='))
    for name in ['cancelled.png','replaced.png','parent-cancelled.svg','changed-source.png']:(root/name).write_bytes(b'unchanged destination\n')
    (root/'failed-original.png').mkdir();(root/'failed-original.png/sentinel').write_bytes(b'unchanged directory\n')
    (root/'config/codex-thread-studio/gnome.json').write_text(json.dumps(dict(selectedBackend='codex',selectedSession='target')))
    code=fixture.runtime.CODEX.replace("    elif method=='thread/read':","    elif method=='model/list':answer(identity,{'data':[]})\n    elif method=='thread/items/list':answer(identity,{'data':[],'nextCursor':None})\n    elif method=='thread/read':")
    for name,source in [('fake-codex.py',code),('fake-opencode.py',fixture.runtime.OPENCODE)]:path=root/name;path.write_text(source);path.chmod(0o700)
    env=dict(os.environ,CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway,CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),CODEX_HOME=str(root/'codex'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),READER_CACHE_DIR=str(root/'reader-cache'),GDK_BACKEND='wayland',GTK_USE_PORTAL='1',GIO_USE_VFS='local',GIO_USE_VOLUME_MONITOR='unix',STUDIO_GNOME_SMOKE_DIR=str(root),STUDIO_GNOME_SMOKE_IMAGE_SAVE='1',STUDIO_OWNED_MUTTER_INPUT='1')
    compositor=[shutil.which('mutter') or 'mutter','--headless','--wayland','--no-x11','--wayland-display','studio-test','--virtual-monitor','1600x1000','--']
    with isolated_environment(env,headless=True) as private:subprocess.run(['dbus-run-session','--',*compositor,sys.executable,str(Path(__file__).resolve()),native,gateway,'--private-root',str(root)],cwd=root,env=private,check=True,timeout=155)
if __name__=='__main__':main()
