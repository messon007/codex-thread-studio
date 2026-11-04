#!/usr/bin/env python3
"""GTK text-flow architecture probe with real diagrams, corpus and owned input."""
import argparse,json,os,resource,shutil,subprocess,sys,tempfile,time
from pathlib import Path
sys.dont_write_bytecode=True
sys.path.insert(0,str(Path(__file__).with_name('test-support')))
from private_wayland import isolated_environment,OwnedMutterInput
from memory_sampler import ProcessMemory

def run(binary,root):
    owned=OwnedMutterInput()
    try:
        owned.enable_clipboard_reader()
        with (root/'app.log').open('w') as log:
            child=subprocess.Popen([binary,str(root)],cwd=root,env=os.environ,stdout=log,stderr=log)
            memory=ProcessMemory(child.pid,root/'no-gateway')
            try:
                deadline=time.monotonic()+120
                while child.poll() is None:
                    owned.respond(root)
                    if time.monotonic()>deadline:raise TimeoutError(('Text-flow probe timed out',root))
                    time.sleep(.01)
            except BaseException:
                child.terminate()
                try:child.wait(timeout=10)
                except subprocess.TimeoutExpired:child.kill();child.wait(timeout=5)
                raise
            finally:measured=memory.close()
    finally:owned.close()
    result=json.loads((root/'result.json').read_text())
    assert child.returncode==0 and result['ok'],(root,result,(root/'app.log').read_text()[-2500:])
    assert not any(s in (root/'app.log').read_text() for s in ['CRITICAL','panicked','Gtk-WARNING'])
    screenshot='primary-flow.png' if os.environ.get('STUDIO_FLOW_PRIMARY_ONLY') else 'columns-flow.png' if os.environ.get('STUDIO_FLOW_COLUMNS_ONLY') else 'images-flow.png' if os.environ.get('STUDIO_FLOW_IMAGES_ONLY') else 'mixed-flow.png'
    assert (root/screenshot).stat().st_size>1000
    measurement=dict(result,memory=measured,artifacts=str(root))
    (root/'measurement.json').write_text(json.dumps(measurement,indent=2)+'\n')
    print(json.dumps(measurement))

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('binary');parser.add_argument('--reference');parser.add_argument('--private-root');focus=parser.add_mutually_exclusive_group();focus.add_argument('--images-only',action='store_true');focus.add_argument('--columns-only',action='store_true');focus.add_argument('--primary-only',action='store_true');args=parser.parse_args()
    binary=str(Path(args.binary).resolve());resource.setrlimit(resource.RLIMIT_CORE,(0,0))
    if args.private_root:return run(binary,Path(args.private_root))
    base=Path(__file__).resolve().parents[1]/'target/gnome-native-smoke';base.mkdir(parents=True,exist_ok=True)
    root=Path(tempfile.mkdtemp(prefix='text-flow-',dir=base))
    env=dict(os.environ,GDK_BACKEND='wayland',XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),STUDIO_OWNED_MUTTER_INPUT='1',STUDIO_GNOME_SMOKE_DIR=str(root))
    for flag in ['STUDIO_FLOW_IMAGES_ONLY','STUDIO_FLOW_COLUMNS_ONLY','STUDIO_FLOW_PRIMARY_ONLY']:env.pop(flag,None)
    if args.primary_only:env['STUDIO_FLOW_PRIMARY_ONLY']='1'
    if args.images_only:env['STUDIO_FLOW_IMAGES_ONLY']='1'
    if args.columns_only:env['STUDIO_FLOW_COLUMNS_ONLY']='1'
    if args.reference:env['STUDIO_FLOW_REFERENCE']=str(Path(args.reference).resolve())
    with isolated_environment(env,headless=True) as private:
        subprocess.run(['dbus-run-session','--',shutil.which('mutter') or 'mutter','--headless','--wayland','--no-x11','--wayland-display','studio-test','--virtual-monitor','1600x1000','--',sys.executable,str(Path(__file__).resolve()),binary,'--private-root',str(root)],cwd=root,env=private,check=True,timeout=150)

if __name__=='__main__':main()
