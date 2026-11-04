#!/usr/bin/env python3
"""Native backend-file editing: protected API, explicit save, conflicts and lazy restart."""
import argparse,importlib.util,json,os,resource,shutil,subprocess,sys,tempfile,time
from pathlib import Path
from urllib.request import Request
from urllib.error import HTTPError
sys.dont_write_bytecode=True
sys.path.insert(0,str(Path(__file__).with_name('test-support')))
from private_wayland import isolated_environment
from memory_sampler import ProcessMemory
spec=importlib.util.spec_from_file_location('backend_runtime',Path(__file__).with_name('smoke-gnome-router-runtime.py'))
runtime=importlib.util.module_from_spec(spec);spec.loader.exec_module(runtime)

def api_checks(gateway,root):
    runtime.fixture.BACKEND=runtime.CODEX;runtime.fixture.OPENCODE_BACKEND=runtime.OPENCODE
    api=runtime.fixture.Gateway(Path(gateway),root,'codex')
    path=root/'config/codex-thread-studio/backends.json';endpoint='/studio/native/backend-config'
    try:
        assert not path.exists()
        original=api.settings.read_bytes()
        registry=api.api('/studio/backends')[1]
        for method,body in [('GET',None),('POST',{'source':'{}'}),('PUT',{'expectedRevision':'x','source':'{}'})]:
            assert api.api(endpoint,body,method=method,authorized=False)[0]==401
        status,base=api.api(endpoint);assert status==200 and not base['exists'] and base['validationError'] is None
        assert not path.exists(),'Read created the configuration file'
        entry=dict(id='native-extra',name='Native 中文',tag='NC',adapter='codex-app-server',command=str(root/'never-start-extra'),args=['--fixture-extra','中文 arg'])
        source=json.dumps(dict(version=1,backends=[entry]),ensure_ascii=False,indent=2).replace('\n','\r\n')+'\r\n'
        status,result=api.api(endpoint,{'source':source},method='POST');assert status==200 and result==dict(validationError=None,backendCount=1)
        assert not path.exists(),'Validation wrote configuration'
        for invalid in ['{',json.dumps({'version':2}),json.dumps({'env':{}}),json.dumps({'backends':[{**entry,'id':'codex'}]}),json.dumps({'backends':[{**entry,'adapter':'other'}]})]:
            status,result=api.api(endpoint,{'source':invalid},method='POST');assert status==200 and result['validationError'] and result['backendCount'] is None
            assert api.api(endpoint,dict(expectedRevision=base['revision'],source=invalid),method='PUT')[0]==400
            assert not path.exists()
        assert api.api(endpoint,{'source':' '*(256*1024+1)},method='POST')[0]==400
        assert api.api(endpoint,{'source':source,'path':'/tmp/not-a-config-target'},method='POST')[0]==400
        headers={'Authorization':'Bearer '+api.token,'Origin':'https://example.invalid','Content-Type':'application/json'}
        try:api.client.open(Request(api.origin+endpoint,headers=headers,data=json.dumps({'source':source}).encode(),method='POST'),timeout=10);raise AssertionError('Foreign origin accepted')
        except HTTPError as error:assert error.code==403
        large=json.dumps(dict(version=1,backends=[{**entry,'id':f'large-{i}','args':['x'*4000]*10} for i in range(6)]))
        started=time.monotonic();status,result=api.api(endpoint,{'source':large},method='POST')
        assert status==200 and result==dict(validationError=None,backendCount=6)
        timing=round((time.monotonic()-started)*1000)
        status,saved=api.api(endpoint,dict(expectedRevision=base['revision'],source=source),method='PUT')
        assert status==200 and saved['exists'] and saved['source']==source
        assert path.read_bytes()==source.encode()
        assert api.api('/studio/backends')[1]==registry,'Save hot-reloaded the active registry'
        assert api.api('/studio/backend/native-extra')[0]==404
        assert api.api(endpoint,dict(expectedRevision=base['revision'],source=base['source']),method='PUT')[0]==409
        assert path.read_bytes()==source.encode()
        path.write_text('broken JSON\n')
        status,broken=api.api(endpoint);assert status==200 and broken['validationError'] and broken['source']=='broken JSON\n'
        assert api.api(endpoint,dict(expectedRevision=saved['revision'],source=source),method='PUT')[0]==409
        assert api.api(endpoint,dict(expectedRevision=broken['revision'],source=source),method='PUT')[0]==200
        path.write_text(' '*(256*1024+1));assert api.api(endpoint)[0]==400
        path.unlink()
        assert api.settings.read_bytes()==original,'Backend editing changed shared preferences'
        assert not runtime.starts(root) and not (root/'backend.pid').exists() and not (root/'extra.pid').exists()
        assert not list(path.parent.glob('.tmp*'))
        return dict(sourceBytes=len(large.encode()),validationRoundtripMs=timing,auth=True,conflicts=True,exactBytes=True,invalidRepair=True,noRuntimeReload=True,noBackendStart=True)
    finally:api.close()

def gui(native,gateway,root):
    results=[]
    for phase in ['edit','restart']:
        env=dict(os.environ,STUDIO_GNOME_SMOKE_BACKENDS=phase)
        with (root/f'{phase}.log').open('w') as log:
            app=subprocess.Popen([native],cwd=root,env=env,stdout=log,stderr=log);memory=ProcessMemory(app.pid,gateway)
            try:app.wait(timeout=100)
            except BaseException:app.terminate();app.wait(timeout=10);raise
            finally:measured=memory.close()
        result=json.loads((root/'backend-config-result.json').read_text());assert app.returncode==0 and result['ok'],(phase,root,result)
        assert not any(word in (root/f'{phase}.log').read_text() for word in ['CRITICAL','panicked','Gtk-WARNING'])
        results.append(dict(phase=phase,checks=result,memory=measured));(root/'backend-config-result.json').rename(root/f'{phase}.json')
        for name in ['backend.pid','opencode.pid','extra.pid']:
            if (root/name).exists():assert not Path('/proc',(root/name).read_text().strip()).exists(),'Owned provider survived'
    assert not runtime.starts(root),'Configuration editor started a model turn'
    argv=json.loads((root/'extra-argv.json').read_text());assert argv[:2]==['--fixture-extra','中文 arg'] and 'app-server' in argv
    assert (root/'config/codex-thread-studio/backends.json').read_bytes()==(root/'expected-backends.json').read_bytes()
    print(json.dumps(dict(ok=True,phases=results,exactSavedFile=True,lazyConfiguredBackend=True,ownedCleanup=True,artifacts=str(root))))

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('native');parser.add_argument('gateway');parser.add_argument('--api-only',action='store_true');parser.add_argument('--private-root');args=parser.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE,(0,0));native,gateway=str(Path(args.native).resolve()),str(Path(args.gateway).resolve())
    if args.private_root:return gui(native,gateway,Path(args.private_root))
    base=Path(__file__).resolve().parents[1]/'target/gnome-native-smoke';base.mkdir(parents=True,exist_ok=True);root=Path(tempfile.mkdtemp(prefix='backend-config-',dir=base));runtime.create_profile(root)
    report=api_checks(gateway,root)
    if args.api_only:print(json.dumps(dict(ok=True,api=report,artifacts=str(root))));return
    (root/'config/codex-thread-studio/backends.json').write_text('broken JSON\n')
    preferences=root/'config/codex-thread-studio/settings.json';raw=json.loads(preferences.read_text());raw['language']='zh-CN';preferences.write_text(json.dumps(raw))
    (root/'config/codex-thread-studio/gnome.json').write_text(json.dumps(dict(selectedBackend='codex',selectedSession='target')))
    code=runtime.CODEX.replace("    elif method=='thread/read':","    elif method=='model/list':answer(identity,{'data':[]})\n    elif method=='thread/items/list':answer(identity,{'data':[],'nextCursor':None})\n    elif method=='thread/read':")
    extra=code.replace("'backend.pid'","'extra.pid'").replace("lock=threading.Lock()","(root/'extra-argv.json').write_text(json.dumps(sys.argv[1:]))\nlock=threading.Lock()",1)
    for name,body in [('fake-codex.py',code),('fake-extra.py',extra),('fake-opencode.py',runtime.OPENCODE)]:path=root/name;path.write_text(body);path.chmod(0o700)
    env=dict(os.environ,CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway,CODEX_THREAD_STUDIO_BACKENDS_CONFIG=str(root/'config/codex-thread-studio/backends.json'),CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),CODEX_HOME=str(root/'codex'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),READER_CACHE_DIR=str(root/'reader-cache'),GDK_BACKEND='wayland',GIO_USE_VFS='local',GIO_USE_VOLUME_MONITOR='unix',STUDIO_GNOME_SMOKE_DIR=str(root))
    with isolated_environment(env,headless=True) as private:
        subprocess.run(['dbus-run-session','--',shutil.which('mutter') or 'mutter','--headless','--wayland','--no-x11','--wayland-display','studio-test','--virtual-monitor','1600x1000','--',sys.executable,str(Path(__file__).resolve()),native,gateway,'--private-root',str(root)],cwd=root,env=private,check=True,timeout=230)
if __name__=='__main__':main()
