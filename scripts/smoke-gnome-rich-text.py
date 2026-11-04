#!/usr/bin/env python3
"""Measure long rich paragraphs through real GTK chat/reader controls, private providers only."""
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
spec=importlib.util.spec_from_file_location('rich_outputs',Path(__file__).with_name('smoke-gnome-outputs.py'))
outputs=importlib.util.module_from_spec(spec);spec.loader.exec_module(outputs)

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('native');parser.add_argument('gateway');parser.add_argument('--headless',action='store_true');parser.add_argument('--private-root');parser.add_argument('--unbroken',action='store_true')
    args=parser.parse_args();resource.setrlimit(resource.RLIMIT_CORE,(0,0))
    native,gateway=map(lambda p:str(Path(p).resolve()),[args.native,args.gateway])
    if args.private_root:
        root=Path(args.private_root)
        with (root/'app.log').open('w') as log:
            child=subprocess.Popen([native],env=dict(os.environ),cwd=root,stdout=log,stderr=log)
            memory=outputs.history.ProcessMemory(child.pid,gateway)
            try:
                deadline=time.monotonic()+110
                while child.poll() is None:
                    if time.monotonic()>deadline:raise TimeoutError(f'Native rich-text fixture timed out: {root}')
                    if 'panicked' in (root/'app.log').read_text():raise AssertionError((root,(root/'app.log').read_text()[-2400:]))
                    time.sleep(.05)
                code=child.returncode
            except BaseException:child.terminate();child.wait(timeout=10);raise
            finally:measure=memory.close()
        result=json.loads((root/'rich-text.json').read_text())
        assert code==0 and result['ok'],(root,code,result)
        assert not outputs.runtime.starts(root),'Reading triggered a model turn'
        text=(root/'app.log').read_text()
        assert not any(s in text for s in ['Gtk-WARNING','CRITICAL','panicked','Theme parser error']),(root,text[-2000:])
        for name in ['backend.pid','opencode.pid']:
            if (root/name).exists():assert not Path('/proc',(root/name).read_text().strip()).exists(),'Owned provider survived'
        print(json.dumps(dict(result,memory=measure,sourceBytes=(root/'project/report.md').stat().st_size,artifacts=str(root))))
        return
    base=Path(__file__).resolve().parents[1]/'target/gnome-native-smoke';base.mkdir(parents=True,exist_ok=True)
    root=Path(tempfile.mkdtemp(prefix='rich-text-ui-',dir=base));outputs.runtime.create_profile(root)
    middle=('Long paragraph 中文😀 keeps its context and formatting. '*80+'字😀abcdef'*9000) if args.unbroken else 'Long paragraph 中文😀 keeps its context and formatting. '*1400
    source='rich-layout-begin **bold 中文😀** *italic* ~~strike~~ `code` [linked 中文😀](nested/note.md:3) <b>literal</b> repeated-source 中文😀 '+middle+' repeated-source 中文😀 rich-layout-tail **end**\n'
    (root/'project/report.md').write_text(source)
    (root/'project/nested').mkdir(exist_ok=True);(root/'project/nested/note.md').write_text('# Linked\n\nDestination line three\n')
    (root/'raw-items.json').write_text(json.dumps([{'id':'rich-assistant','type':'agentMessage','text':source}],ensure_ascii=False))
    (root/'config/codex-thread-studio/gnome.json').write_text(json.dumps({'selectedBackend':'codex','selectedSession':'target'}))
    for name,source in zip(['fake-codex.py','fake-opencode.py'],outputs.providers()):
        path=root/name;path.write_text(source);path.chmod(0o700)
    env=dict(os.environ,CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway,CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),CODEX_HOME=str(root/'codex'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),READER_CACHE_DIR=str(root/'reader-cache'),GDK_BACKEND='wayland',STUDIO_GNOME_SMOKE_DIR=str(root),STUDIO_GNOME_SMOKE_RICH_TEXT='1')
    compositor=[]
    if args.headless:
        mutter=shutil.which('mutter')
        if not mutter:raise RuntimeError('Mutter is needed for --headless testing')
        compositor=[mutter,'--headless','--wayland','--no-x11','--wayland-display','studio-test','--virtual-monitor','1600x1000','--']
    with outputs.history.isolated_environment(env,headless=args.headless) as private:
        subprocess.run(['dbus-run-session','--',*compositor,sys.executable,str(Path(__file__).resolve()),native,gateway,'--private-root',str(root)],env=private,cwd=root,check=True,timeout=130)

if __name__=='__main__':main()
