#!/usr/bin/env python3
"""Native browser settings transactions and live policy; owned profiles/pages only."""
import argparse
import http.server
import importlib.util
import json
import os
from pathlib import Path
import resource
import shutil
import subprocess
import sys
import tempfile
import threading
import time
from urllib.request import Request
from urllib.error import HTTPError
sys.dont_write_bytecode=True
spec=importlib.util.spec_from_file_location('browser_settings_runtime',Path(__file__).with_name('smoke-gnome-router-runtime.py'))
runtime=importlib.util.module_from_spec(spec);spec.loader.exec_module(runtime)
sys.path.insert(0,str(Path(__file__).with_name('test-support')))
from private_wayland import isolated_environment,OwnedMutterInput
from memory_sampler import ProcessMemory

def api_checks(gateway,root):
    runtime.fixture.BACKEND=runtime.CODEX;runtime.fixture.OPENCODE_BACKEND=runtime.OPENCODE
    service=runtime.fixture.Gateway(Path(gateway),root,'codex')
    def api(body=None,status=200,auth=True,origin=None):
        headers={'Content-Type':'application/json'}
        if auth:headers['Authorization']='Bearer '+service.token
        if origin:headers['Origin']=origin
        request=Request(service.origin+'/studio/native/browser-settings',data=None if body is None else json.dumps(body).encode(),headers=headers,method='GET' if body is None else 'PATCH')
        try:response=service.client.open(request,timeout=20)
        except HTTPError as error:response=error
        with response:
            data=response.read();assert response.status==status,(response.status,status,data[:500]);return json.loads(data) if data else None
    try:
        path=root/'config/codex-thread-studio/settings.json'
        original=path.read_bytes();api(auth=False,status=401);api(origin='https://not-studio.invalid',status=403)
        first=api();assert len(first['revision'])==64 and path.read_bytes()==original
        values=first['values'];values['allowPrivateNetwork']=True
        raw=json.loads(path.read_text());raw['futureBrowserFixture']={'keep':'中文😀'};raw['browser']={'agent':{'enabled':False,'profile':'isolated','futureAgent':42},'restoreTabs':True,'embeddedWidth':900,'futureBrowser':{'keep':True}};path.write_text(json.dumps(raw))
        current=api()
        saved=api({'expectedRevision':current['revision'],'values':values});assert saved['values']==values
        persisted=json.loads(path.read_text());assert persisted['browser']['agent']==raw['browser']['agent'] and persisted['browser']['futureBrowser']==raw['browser']['futureBrowser'] and persisted['futureBrowserFixture']==raw['futureBrowserFixture']
        unchanged=path.read_bytes();api({'expectedRevision':current['revision'],'values':values},status=409)
        api({'expectedRevision':saved['revision'],'values':dict(values,allowHttp='yes')},status=400)
        api({'expectedRevision':saved['revision'],'values':dict(values,unrecognized=True)},status=400)
        api({'expectedRevision':'bad','values':values},status=400);assert path.read_bytes()==unchanged
        values['allowPrivateNetwork']=False;current=api({'expectedRevision':saved['revision'],'values':values})
        def tabs(body=None,status=200,auth=True,origin=None):
            headers={'Content-Type':'application/json'}
            if auth:headers['Authorization']='Bearer '+service.token
            if origin:headers['Origin']=origin
            request=Request(service.origin+'/studio/native/browser-tabs',data=None if body is None else json.dumps(body).encode(),headers=headers,method='GET' if body is None else 'PUT')
            try:response=service.client.open(request,timeout=20)
            except HTTPError as error:response=error
            with response:
                data=response.read();assert response.status==status,(response.status,status,data[:500]);return json.loads(data) if data else None
        tabs(auth=False,status=401);tabs(origin='https://not-studio.invalid',status=403)
        first=tabs();assert first['tabs']==[] and first['preferences']['restoreTabs']
        selected=tabs({'expectedRevision':first['revision'],'tabs':['https://example.com/你好','http://127.0.0.1:3131/one'],'active':1})
        assert selected['active']==1 and '%E4' in selected['tabs'][0]
        tabs({'expectedRevision':first['revision'],'tabs':[],'active':0},status=409)
        for bad in [dict(tabs=['file:///etc/passwd'],active=0),dict(tabs=['https://user:secret@example.com'],active=0),dict(tabs=['https://example.com']*9,active=0),dict(tabs=['https://example.com'],active=1),dict(tabs=[],active=0,unknown=True)]:
            tabs(dict(expectedRevision=selected['revision'],**bad),status=400)
        # Both narrowed native settings and the legacy shared-settings writer
        # must rotate the generation, including empty off/on cycles.
        off=api(dict(expectedRevision=current['revision'],values=values,restoreTabs=False));assert not off['restoreTabs']
        empty=tabs();assert empty['tabs']==[]
        tabs(dict(expectedRevision=selected['revision'],tabs=['https://example.com'],active=0),status=409)
        on=api(dict(expectedRevision=off['revision'],values=values,restoreTabs=True))
        tabs(dict(expectedRevision=empty['revision'],tabs=['https://example.com'],active=0),status=409)
        assert tabs()['tabs']==[] and not 'example.com' in (path.parent/'gnome-browser-tabs.json').read_text()
        raw=json.loads(path.read_text())
        for restore in [False,True]:
            raw['browser']['restoreTabs']=restore
            request=Request(service.origin+'/studio/preferences',data=json.dumps(raw).encode(),headers={'Authorization':'Bearer '+service.token,'Content-Type':'application/json'},method='PUT')
            with service.client.open(request,timeout=20) as response:assert response.status==200
        tabs(dict(expectedRevision=empty['revision'],tabs=['https://example.com'],active=0),status=409)

    finally:service.close()

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('native');parser.add_argument('gateway',nargs='?');parser.add_argument('--api-only',action='store_true');parser.add_argument('--headless',action='store_true');parser.add_argument('--private-root')
    args=parser.parse_args();resource.setrlimit(resource.RLIMIT_CORE,(0,0))
    native,gateway=str(Path(args.native).resolve()),str(Path(args.gateway or args.native).resolve())
    if args.private_root:return run(native,gateway,Path(args.private_root),dict(os.environ))
    base=Path(__file__).resolve().parents[1]/'target/gnome-native-smoke';base.mkdir(parents=True,exist_ok=True)
    root=Path(tempfile.mkdtemp(prefix='browser-settings-ui-',dir=base));runtime.create_profile(root)
    api_checks(gateway,root)
    if args.api_only:print(json.dumps({'ok':True,'scopedApi':True,'authOrigin':True,'staleSave':True,'preservesUnknownSettings':True,'artifacts':str(root)}));return
    backend=runtime.CODEX.replace("    elif method=='thread/read':","    elif method=='model/list':answer(identity,{'data':[]})\n    elif method=='thread/items/list':answer(identity,{'data':[],'nextCursor':None})\n    elif method=='thread/read':")
    for name,source in [('fake-codex.py',backend),('fake-opencode.py',runtime.OPENCODE)]:
        path=root/name;path.write_text(source);path.chmod(0o700)
    (root/'config/codex-thread-studio/gnome.json').write_text(json.dumps({'selectedBackend':'codex','selectedSession':'target'}))
    env=dict(os.environ,CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway,CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),CODEX_HOME=str(root/'codex'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),READER_CACHE_DIR=str(root/'reader-cache'),GDK_BACKEND='wayland',STUDIO_GNOME_SMOKE_DIR=str(root),STUDIO_GNOME_SMOKE_BROWSER_SETTINGS='1')
    compositor=[]
    if args.headless:
        mutter=shutil.which('mutter')
        if not mutter:raise RuntimeError('Mutter is required for --headless')
        compositor=[mutter,'--headless','--wayland','--no-x11','--wayland-display','studio-test','--virtual-monitor','1600x1000','--'];env['STUDIO_OWNED_MUTTER_INPUT']='1'
    with isolated_environment(env,headless=args.headless) as private:
        subprocess.run(['dbus-run-session','--',*compositor,sys.executable,str(Path(__file__).resolve()),native,gateway,'--private-root',str(root)],env=private,cwd=root,check=True,timeout=300)

def run(native,gateway,root,env):
    requests=[]
    class Pages(http.server.BaseHTTPRequestHandler):
        def do_GET(self):
            requests.append(self.path)
            if self.path not in ['/one','/two','/favicon.ico']:self.send_error(404);return
            body=('<!doctype html><html><head><meta charset="utf-8"><title>Private browser policy '+self.path+'</title></head><body><h1>Browser policy 中文😀</h1><p>Read-only private fixture.</p><script>document.documentElement.dataset.scriptRan="yes";document.title="Page script ran";</script></body></html>').encode()
            self.send_response(200);self.send_header('Content-Type','text/html; charset=utf-8');self.send_header('Content-Length',str(len(body)));self.end_headers();self.wfile.write(body)
        def log_message(self,*args):pass
    server=http.server.ThreadingHTTPServer(('127.0.0.1',0),Pages);threading.Thread(target=server.serve_forever,daemon=True).start()
    env['STUDIO_GNOME_SMOKE_BROWSER_SETTINGS_URL']=f'http://127.0.0.1:{server.server_port}/one'
    results=[]
    try:
        for phase in ['initial','restart']:
            env['STUDIO_GNOME_SMOKE_BROWSER_RESTORE_PHASE']=phase
            log_path=root/('app.log' if phase=='initial' else 'app-restart.log')
            with log_path.open('w') as log:
                child=subprocess.Popen([native],env=env,cwd=root,stdout=log,stderr=log);memory=ProcessMemory(child.pid,gateway,include_web_content=True)
                controlled=None
                try:
                    if env.get('STUDIO_OWNED_MUTTER_INPUT')=='1':controlled=OwnedMutterInput()
                    deadline=time.monotonic()+150
                    while child.poll() is None:
                        if time.monotonic()>deadline:raise TimeoutError(f'Browser settings fixture timed out: {root} {phase}')
                        key=root/'browser-settings-key'
                        if key.exists():
                            key.unlink()
                            if controlled:
                                controlled.focus_only_window();time.sleep(.35)
                                for keysym,state in [(0xffe3,True),(0x2c,True),(0x2c,False),(0xffe3,False)]:controlled.call('NotifyKeyboardKeysym',controlled.GLib.Variant('(ub)',(keysym,state)))
                        if 'panicked' in log_path.read_text():raise AssertionError((root,log_path.read_text()[-3000:]))
                        time.sleep(.02)
                except BaseException:child.terminate();child.wait(timeout=10);raise
                finally:
                    if controlled:controlled.close()
                    measure=memory.close()
            result=json.loads((root/'browser-settings.json').read_text());assert child.returncode==0 and result['ok'],(root,phase,result,log_path.read_text()[-2400:])
            (root/f'browser-settings-{phase}.json').write_text(json.dumps(result))
            results.append(dict(result,observedMemory=measure))
            assert not runtime.starts(root),'Settings test started a model turn'
            for name in ['backend.pid','opencode.pid']:
                if (root/name).exists():
                    pid=(root/name).read_text().strip();assert not Path('/proc',pid).exists(),f'Owned provider survived: {pid}'
            text=log_path.read_text();assert not any(term in text for term in ['CRITICAL','panicked','Theme parser error']),(root,text[-2400:])
            saved=json.loads((root/'config/codex-thread-studio/gnome-browser-tabs.json').read_text())
            base=env['STUDIO_GNOME_SMOKE_BROWSER_SETTINGS_URL']
            assert saved['tabs']==[base,base[:-4]+'/two'] and saved['active']==1,(root,saved)
    finally:server.shutdown();server.server_close()
    print(json.dumps(dict(ok=True,launches=results,scopedApi=True,backendCleanup=True,requests=requests,artifacts=str(root))))

if __name__=='__main__':main()
