#!/usr/bin/env python3
"""Exercise native rich streaming through private providers, workers and actual GTK widgets."""
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
    parser.add_argument('native');parser.add_argument('gateway');parser.add_argument('--headless',action='store_true');parser.add_argument('--private-root')
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
                    if time.monotonic()>deadline:raise TimeoutError(f'Native streaming fixture timed out: {root}')
                    if 'panicked' in (root/'app.log').read_text():raise AssertionError((root,(root/'app.log').read_text()[-2400:]))
                    time.sleep(.05)
                code=child.returncode
            except BaseException:child.terminate();child.wait(timeout=10);raise
            finally:measure=memory.close()
        result=json.loads((root/'streaming.json').read_text())
        assert code==0 and result['ok'],(root,code,result)
        assert not outputs.runtime.starts(root),'Reading triggered a model turn'
        text=(root/'app.log').read_text()
        assert not any(s in text for s in ['Gtk-WARNING','CRITICAL','panicked','Theme parser error']),(root,text[-2000:])
        for name in ['backend.pid','opencode.pid']:
            if (root/name).exists():outputs.runtime.until(lambda:not Path('/proc',(root/name).read_text().strip()).exists(),seconds=10)
        print(json.dumps(dict(result,memory=measure,sourceBytes=(root/'project/report.md').stat().st_size,artifacts=str(root))))
        return
    base=Path(os.environ.get('STUDIO_NATIVE_QA_DIR') or Path(__file__).resolve().parents[1]/'target/gnome-native-smoke');base.mkdir(parents=True,exist_ok=True)
    root=Path(tempfile.mkdtemp(prefix='streaming-ui-',dir=base));outputs.runtime.create_profile(root)
    seed='# Streaming preview\n\n'+('Stream paragraph 中文😀 has stable formatting and context. '*350)+'\n\n| Name | Value |\n|---|---|\n'+''.join('| Row '+str(i)+' | value |\n' for i in range(80))+'\n```mermaid\nflowchart LR\n  A --> B\n```\n\n$$\nx=\\frac{1}{2}\n$$\n\nStreaming tail'
    (root/'stream-seed.md').write_text(seed)
    (root/'project/report.md').write_text(seed)
    (root/'raw-items.json').write_text('[]')
    (root/'config/codex-thread-studio/gnome.json').write_text(json.dumps({'selectedBackend':'codex','selectedSession':'target'}))
    codex,opencode=outputs.providers()
    codex=codex.replace("summary('target'),summary('hidden')","summary('target'),summary('other'),summary('hidden')",1)
    publisher=r"""
def publish_stream():
    while True:
        path=root/'stream-events.json'
        if path.exists():
            values=json.loads(path.read_text());path.unlink()
            for value in values:
                with lock:print(json.dumps(value),flush=True)
                if value.get('method')=='item/agentMessage/delta':time.sleep(.025)
        time.sleep(.005)
threading.Thread(target=publish_stream,daemon=True).start()
"""
    assert 'for raw in sys.stdin:' in codex
    codex=codex.replace('for raw in sys.stdin:',publisher+'\nfor raw in sys.stdin:',1)
    for name,source in [('fake-codex.py',codex),('fake-opencode.py',opencode)]:
        path=root/name;path.write_text(source);path.chmod(0o700)
    env=dict(os.environ,CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway,CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),CODEX_HOME=str(root/'codex'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),READER_CACHE_DIR=str(root/'reader-cache'),GDK_BACKEND='wayland',STUDIO_GNOME_SMOKE_DIR=str(root),STUDIO_GNOME_SMOKE_STREAMING='1')
    compositor=[]
    if args.headless:
        mutter=shutil.which('mutter')
        if not mutter:raise RuntimeError('Mutter is needed for --headless testing')
        compositor=[mutter,'--headless','--wayland','--no-x11','--wayland-display','studio-test','--virtual-monitor','1600x1000','--']
    with outputs.history.isolated_environment(env,headless=args.headless) as private:
        subprocess.run(['dbus-run-session','--',*compositor,sys.executable,str(Path(__file__).resolve()),native,gateway,'--private-root',str(root)],env=private,cwd=root,check=True,timeout=130)

if __name__=='__main__':main()
