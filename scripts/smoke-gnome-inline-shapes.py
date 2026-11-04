#!/usr/bin/env python3
"""Native Pango inline-math experiment with an independent Wayland clipboard."""
import argparse,json,os,resource,shutil,subprocess,sys,tempfile,time
from pathlib import Path
sys.dont_write_bytecode=True
sys.path.insert(0,str(Path(__file__).with_name('test-support')))
from private_wayland import isolated_environment,OwnedMutterInput

def run(binary,root):
    controlled=OwnedMutterInput();controlled.enable_clipboard_reader()
    try:
        with (root/'app.log').open('w') as log:
            child=subprocess.Popen([binary,str(root)],cwd=root,env=dict(os.environ),stdout=log,stderr=log)
            try:
                end=time.monotonic()+60
                while child.poll() is None:
                    controlled.respond(root)
                    if time.monotonic()>end:raise TimeoutError(('Native inline shape experiment timed out',root))
                    time.sleep(.01)
            except BaseException:child.terminate();child.wait(timeout=10);raise
    finally:controlled.close()
    result=json.loads((root/'result.json').read_text())
    assert child.returncode==0 and result['ok'],(root,child.returncode,result,(root/'app.log').read_text()[-2500:])
    cases={c['name']:c for c in result['multilineProbe']['cases']}
    assert result['multilineProbe']['diagnosticOnly'] and set(cases)=={'lf','crlf','outside'}
    assert cases['lf']['rawLabelCopyExact'] and cases['outside']['rawLabelCopyExact']
    assert cases['crlf']['production']=={'textView':True,'copyExact':True,'renderedMath':True,'singleLine':True}
    screenshots=['inline-light.png','inline-dark.png','multiline-crlf-production.png']+[f'multiline-{case}-{mode}.png' for case in cases for mode in ['normal','single']]
    assert all((root/name).stat().st_size>1000 for name in screenshots)
    text=(root/'app.log').read_text()
    assert not any(s in text for s in ['CRITICAL','panicked','Theme parser error','Gtk-WARNING']),(root,text[-2500:])
    print(json.dumps(dict(result,artifacts=str(root))))

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('binary');parser.add_argument('--private-root');args=parser.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE,(0,0));binary=str(Path(args.binary).resolve())
    if args.private_root:return run(binary,Path(args.private_root))
    base=Path(__file__).resolve().parents[1]/'target/gnome-native-smoke';base.mkdir(parents=True,exist_ok=True)
    root=Path(tempfile.mkdtemp(prefix='inline-shapes-',dir=base))
    env=dict(os.environ,XDG_CONFIG_HOME=str(root/'config'),XDG_CACHE_HOME=str(root/'cache'),XDG_DATA_HOME=str(root/'data'),GDK_BACKEND='wayland',STUDIO_OWNED_MUTTER_INPUT='1',STUDIO_GNOME_SMOKE_DIR=str(root))
    compositor=[shutil.which('mutter') or 'mutter','--headless','--wayland','--no-x11','--wayland-display','studio-test','--virtual-monitor','1100x700','--']
    with isolated_environment(env,headless=True) as private:
        subprocess.run(['dbus-run-session','--',*compositor,sys.executable,str(Path(__file__).resolve()),binary,'--private-root',str(root)],cwd=root,env=private,check=True,timeout=90)
if __name__=='__main__':main()
