#!/usr/bin/env python3
"""Scoped session rules and actual native list/Settings controls in private profiles."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import resource
import subprocess
import sys
import tempfile
import time
from urllib.request import Request
from urllib.error import HTTPError
sys.dont_write_bytecode=True
sys.path.insert(0,str(Path(__file__).with_name('test-support')))
from memory_sampler import ProcessMemory
spec=importlib.util.spec_from_file_location('visibility_fixture',Path(__file__).with_name('smoke-gnome-router-runtime.py'))
runtime=importlib.util.module_from_spec(spec);spec.loader.exec_module(runtime)

def api_checks(executable,root):
    runtime.fixture.BACKEND=runtime.CODEX;runtime.fixture.OPENCODE_BACKEND=runtime.OPENCODE
    service=runtime.fixture.Gateway(Path(executable),root,'codex');endpoint='/studio/native/session-visibility'
    try:
        assert service.api(endpoint,authorized=False)[0]==401
        assert service.api(endpoint,{},method='PATCH',authorized=False)[0]==401
        try:
            service.client.open(Request(service.origin+endpoint,headers={'Authorization':'Bearer '+service.token,'Origin':'https://foreign.example'}))
            raise AssertionError('Foreign origin could read visibility rules')
        except HTTPError as error:assert error.code==403
        code,base=service.api(endpoint);assert code==200,base
        initial=base['values'];raw=service.preferences();raw['futureVisibility']={'preserve':True};raw['contentWidth']='wide';service.settings.write_text(json.dumps(raw))
        def patch(revision,values,**extra):return service.api(endpoint,{'expectedRevision':revision,'values':values,**extra},method='PATCH')
        values={'hiddenSessionDirectories':[str(root/'hidden'),str(root/'project')],
                'sessionDirectoryIgnore':['# keep comment','**/build/','!'+str(root/'project/build/keep')+'/']}
        code,saved=patch(base['revision'],values);assert code==200 and saved['values']==values,saved
        assert service.preferences()['futureVisibility']['preserve'] and service.preferences()['contentWidth']=='wide'
        before=service.settings.read_bytes()
        assert patch(base['revision'],initial)[0]==409 and service.settings.read_bytes()==before
        for invalid in [dict(values,hiddenSessionDirectories=['']),dict(values,hiddenSessionDirectories=['/hidden']*257),
                        dict(values,sessionDirectoryIgnore=['bad\trule']),dict(values,sessionDirectoryIgnore=['x'*4097]),
                        dict(values,sessionDirectoryIgnore=['*']*513),dict(values,unknown=True),{'hiddenSessionDirectories':[]}]:
            assert patch(saved['revision'],invalid)[0]==400,invalid
            assert service.settings.read_bytes()==before
        assert patch(saved['revision'],values,unknown=True)[0]==400
        code,reset=patch(saved['revision'],initial);assert code==200 and reset['values']==initial,reset
        # Scoped recovery reads preserve bounded raw values and never rewrite on GET.
        corruptions=[dict(initial,hiddenSessionDirectories={'broken':True}),
                     dict(initial,sessionDirectoryIgnore=['x'*4097]),
                     dict(initial,sessionDirectoryIgnore=['bad\x00rule']),
                     dict(initial,sessionDirectoryIgnore=['scratch-*']*513),
                     dict(initial,sessionDirectoryIgnore=[f'{i:03}'+('a?'*2046) for i in range(500)])]
        for corrupt in corruptions:
            raw=service.preferences();raw.update(corrupt);service.settings.write_text(json.dumps(raw))
            before=service.settings.read_bytes();code,recovery=service.api(endpoint)
            assert code==200 and recovery['values']==corrupt and recovery.get('validationError'),recovery.get('validationError')
            assert service.settings.read_bytes()==before
            assert patch(recovery['revision'],corrupt)[0]==400 and service.settings.read_bytes()==before
            code,fixed=patch(recovery['revision'],initial)
            assert code==200 and fixed['values']==initial and not fixed.get('validationError'),fixed
            assert patch(recovery['revision'],initial)[0]==409
            assert service.preferences()['futureVisibility']['preserve']
        assert service.preferences()['futureSetting']['preserve']
        assert not runtime.starts(root),'Visibility settings triggered a model turn'
    finally:service.close()

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('native');parser.add_argument('gateway');parser.add_argument('--api-only',action='store_true');parser.add_argument('--large-catalog',action='store_true');parser.add_argument('--recovery',action='store_true');args=parser.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE,(0,0))
    native,gateway=map(lambda p:str(Path(p).resolve()),[args.native,args.gateway])
    base=Path(__file__).resolve().parents[1]/'target/gnome-native-smoke';base.mkdir(parents=True,exist_ok=True)
    root=Path(tempfile.mkdtemp(prefix='visibility-ui-',dir=base));runtime.create_profile(root)
    (root/'project/build/keep').mkdir(parents=True);(root/'project/build/drop').mkdir()
    api_checks(gateway,root)
    if args.api_only:print(json.dumps({'ok':True,'visibilityApi':True,'artifacts':str(root)}));return
    backend=runtime.CODEX.replace("assert params['includeTurns'] is False",'pass').replace("assert params['excludeTurns'] is True",'pass')
    backend=backend.replace("answer(identity,{'thread':summary(session)})", "answer(identity,{'thread':{**summary(session),'turns':[{'id':'visible-turn','status':'completed','items':[{'id':'visibility-history','type':'agentMessage','text':'Retained conversation 中文😀'}]}]}})")
    backend=backend.replace("    elif method=='thread/list':","    elif method=='model/list':answer(identity,{'data':[]})\n    elif method=='thread/list':",1)
    backend=backend.replace("[summary('router'),summary('target'),summary('hidden'),", "[{**summary('build-keep'),'cwd':str(root/'project/build/keep')},{**summary('build-drop'),'cwd':str(root/'project/build/drop')},summary('router'),summary('target'),summary('hidden'),",1)
    if args.large_catalog:
        line=next(line for line in backend.splitlines() if "elif method=='thread/list':" in line)
        backend=backend.replace(line,"""    elif method=='thread/list':
        special=[{**summary('build-keep'),'cwd':str(root/'project/build/keep')},{**summary('build-drop'),'cwd':str(root/'project/build/drop')},summary('router'),summary('target'),summary('hidden')]
        start=int(params.get('cursor') or 0);stop=min(start+100,10030)
        answer(identity,{'data':[special[i] if i<len(special) else summary('bulk-'+str(i)) for i in range(start,stop)],'nextCursor':str(stop) if stop<10030 else None})""")
    for name,source in [('fake-codex.py',backend),('fake-opencode.py',runtime.OPENCODE)]:
        path=root/name;path.write_text(source);path.chmod(0o700)
    environment=dict(os.environ,CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway,CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),CODEX_HOME=str(root/'codex'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),READER_CACHE_DIR=str(root/'reader-cache'),GDK_BACKEND='wayland',STUDIO_GNOME_SMOKE_DIR=str(root))
    environment.pop('DISPLAY',None);environment.pop('XAUTHORITY',None)
    if args.large_catalog:environment['STUDIO_GNOME_SMOKE_VISIBILITY_LARGE']='1'
    if args.recovery:
        backend_guard="""    if method in ['thread/read','thread/resume']:
        saved=json.loads((root/'config/codex-thread-studio/settings.json').read_text())
        assert isinstance(saved.get('hiddenSessionDirectories'),list), 'Read bypassed the unknown Router registry'
"""
        path=root/'fake-codex.py';source=path.read_text();source=source.replace("    if method=='initialize':",backend_guard+"    if method=='initialize':",1);path.write_text(source)
        settings=root/'config/codex-thread-studio/settings.json';raw=json.loads(settings.read_text())
        raw.update(hiddenSessionDirectories={'broken':True},sessionDirectoryIgnore=['x'*(1024*1024)])
        settings.write_text(json.dumps(raw))
    results=[]
    for phase in ['recovery' if args.recovery else 'edit','restore']:
        environment['STUDIO_GNOME_SMOKE_VISIBILITY']=phase
        with (root/f'app-{phase}.log').open('w') as log:
            child=subprocess.Popen([native],env=environment,cwd=root,stdout=log,stderr=log)
            observer=ProcessMemory(child.pid,gateway)
            try:child.wait(timeout=90)
            except subprocess.TimeoutExpired:child.terminate();child.wait(timeout=10);raise AssertionError(f'Visibility fixture timed out: {root}')
            finally:memory=observer.close()
        assert child.returncode==0,(root,phase,child.returncode)
        result=json.loads((root/f'visibility-{phase}.json').read_text());assert result['ok'],(root,result);result['memory']=memory;results.append(result)
        assert not runtime.starts(root),'Visibility UI triggered a model turn'
        for name in ['backend.pid','opencode.pid']:
            if not (root/name).exists():continue
            identity=(root/name).read_text().strip()
            for _ in range(100):
                if not Path('/proc',identity).exists():break
                time.sleep(.05)
            assert not Path('/proc',identity).exists(),f'Owned backend {identity} survived visibility fixture'
        log=(root/f'app-{phase}.log').read_text();assert not any(s in log for s in ['CRITICAL','panicked','Theme parser error']),(root,log[-2500:])
    raw=json.loads((root/'config/codex-thread-studio/settings.json').read_text())
    assert raw['futureSetting']['preserve'] and raw['futureVisibility']['preserve'] and raw['queueDepth']==3 and raw['router']['futureRouter']==42
    print(json.dumps({'ok':True,'visibilityApi':True,'launches':results,'unrelatedPreferences':True,'backendCleanup':True,'artifacts':str(root)}))

if __name__=='__main__':main()
