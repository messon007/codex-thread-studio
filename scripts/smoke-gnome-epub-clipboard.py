#!/usr/bin/env python3
"""Actual Wayland EPUB selection and independent clipboard transfer on private data."""
import argparse,http.server,importlib.util,json,os,resource,shutil,subprocess,sys,tempfile,threading,time,zipfile
from pathlib import Path
sys.dont_write_bytecode=True
spec=importlib.util.spec_from_file_location('epub_copy_fixture',Path(__file__).with_name('smoke-gnome-epub.py'))
fixture=importlib.util.module_from_spec(spec);spec.loader.exec_module(fixture);runtime=fixture.runtime
sys.path.insert(0,str(Path(__file__).with_name('test-support')))
from private_wayland import isolated_environment,OwnedMutterInput
from memory_sampler import ProcessMemory

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('native');parser.add_argument('gateway');parser.add_argument('--private-root');args=parser.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE,(0,0));native,gateway=str(Path(args.native).resolve()),str(Path(args.gateway).resolve())
    if args.private_root:return run(native,gateway,Path(args.private_root),dict(os.environ))
    base=Path(__file__).resolve().parents[1]/'target/gnome-native-smoke';base.mkdir(parents=True,exist_ok=True);root=Path(tempfile.mkdtemp(prefix='epub-clipboard-ui-',dir=base));runtime.create_profile(root);requests=[]
    class Canary(http.server.BaseHTTPRequestHandler):
        def do_GET(self):requests.append(self.path);self.send_response(200);self.end_headers()
        def log_message(self,*args):pass
    server=http.server.ThreadingHTTPServer(('127.0.0.1',0),Canary);threading.Thread(target=server.serve_forever,daemon=True).start()
    try:
        source=root/'project/book.epub';fixture.book(source,f'http://127.0.0.1:{server.server_port}')
        with zipfile.ZipFile(source) as old,zipfile.ZipFile(root/'project/sample.epub','w',compression=zipfile.ZIP_DEFLATED) as new:
            for item in old.infolist():
                content=old.read(item)
                if item.filename.startswith('OPS/Text/'):
                    content=content.replace(b'<p id="copy">Needle and native text selection.</p>', '<pre id="copy">Needle <em>中文😀</em> trailing </pre>'.encode())
                new.writestr(item,content)
        backend=runtime.CODEX.replace("    elif method=='thread/read':","    elif method=='model/list':answer(identity,{'data':[]})\n    elif method=='thread/items/list':answer(identity,{'data':[],'nextCursor':None})\n    elif method=='thread/read':")
        for name,code in [('fake-codex.py',backend),('fake-opencode.py',runtime.OPENCODE)]:file=root/name;file.write_text(code);file.chmod(0o700)
        env=dict(os.environ,CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway,CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),CODEX_HOME=str(root/'codex'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),READER_CACHE_DIR=str(root/'reader-cache'),GDK_BACKEND='wayland',STUDIO_GNOME_SMOKE_DIR=str(root),STUDIO_GNOME_SMOKE_EPUB_CLIPBOARD='1',STUDIO_OWNED_MUTTER_INPUT='1')
        command=[shutil.which('mutter') or 'mutter','--headless','--wayland','--no-x11','--wayland-display','studio-test','--virtual-monitor','1600x1000','--']
        with isolated_environment(env,headless=True) as private:subprocess.run(['dbus-run-session','--',*command,sys.executable,str(Path(__file__).resolve()),native,gateway,'--private-root',str(root)],cwd=root,env=private,check=True,timeout=150)
        assert not requests,('EPUB fetched remote resources',requests)
    finally:server.shutdown();server.server_close()

def run(native,gateway,root,env):
    controlled=OwnedMutterInput();controlled.enable_clipboard_reader()
    try:
        with (root/'app.log').open('w') as log:
            child=subprocess.Popen([native],cwd=root,env=env,stdout=log,stderr=log);observer=ProcessMemory(child.pid,gateway,include_web_content=True)
            try:
                time.sleep(1);controlled.focus_only_window();deadline=time.monotonic()+110
                while child.poll() is None:
                    controlled.respond(root)
                    if time.monotonic()>deadline:raise TimeoutError(('EPUB clipboard fixture timed out',root))
                    if 'panicked' in (root/'app.log').read_text():raise AssertionError((root,(root/'app.log').read_text()[-3000:]))
                    time.sleep(.02)
            except BaseException:child.terminate();child.wait(timeout=10);raise
            finally:measure=observer.close()
    finally:controlled.close()
    result=json.loads((root/'epub-clipboard.json').read_text());assert child.returncode==0 and result['ok'],(root,result,(root/'app.log').read_text()[-2500:]);assert not runtime.starts(root)
    for _ in range(100):
        if not observer.web_content_alive():break
        time.sleep(.05)
    assert not observer.web_content_alive(),('Owned EPUB renderer survived',root)
    for name in ['backend.pid','opencode.pid']:
        if (root/name).exists():assert not Path('/proc',(root/name).read_text().strip()).exists(),'Owned backend survived'
    text=(root/'app.log').read_text();assert not any(s in text for s in ['CRITICAL','panicked','Theme parser error']),(root,text[-3000:])
    print(json.dumps(dict(result,memory=measure,artifacts=str(root))))

if __name__=='__main__':main()
