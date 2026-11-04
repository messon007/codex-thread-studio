#!/usr/bin/env python3
"""Read-only routed-source ownership after gateway restart, private provider only."""
import importlib.util
import json
from pathlib import Path
import sqlite3
import sys
import tempfile

sys.dont_write_bytecode=True
spec=importlib.util.spec_from_file_location('scope_fixture',Path(__file__).with_name('smoke-gnome-supervision.py'))
fixture=importlib.util.module_from_spec(spec);spec.loader.exec_module(fixture)
runtime=fixture.runtime
SCOPE='/studio/native/router/supervision-scope'

def main():
    binary=Path(sys.argv[1]).resolve();base=Path(__file__).resolve().parents[1]/'target/gnome-native-smoke';base.mkdir(parents=True,exist_ok=True)
    root=Path(tempfile.mkdtemp(prefix='supervision-scope-',dir=base));runtime.create_profile(root)
    runtime.fixture.BACKEND=fixture.source();runtime.fixture.OPENCODE_BACKEND=runtime.OPENCODE
    gateway=runtime.fixture.Gateway(binary,root,'opencode')
    try:
        identity=runtime.submit(gateway,runtime.body(gateway,'Restored routed task'))
        runtime.until(lambda:runtime.get(gateway,identity)['status']=='running')
        job=runtime.get(gateway,identity)
    finally:gateway.close()
    gateway=runtime.fixture.Gateway(binary,root,'opencode')
    try:
        before=runtime.requests(root)
        with sqlite3.connect(root/'config/codex-thread-studio/studio.sqlite3') as db:stored=db.execute('SELECT owner,phase,json FROM native_router_jobs WHERE id=?',(identity,)).fetchone()
        query={'backend':'codex','threadId':'target','turnId':job['targetTurnId'],'cwd':str(root/'project')}
        assert gateway.api(SCOPE,query,authorized=False)[0]==401
        for bad in [{},{**query,'extra':True},{**query,'backend':'opencode'},{**query,'turnId':'bad\0id'},{**query,'cwd':''}]:assert gateway.api(SCOPE,bad)[0]==400
        status,value=gateway.api(SCOPE,query)
        assert status==200 and value=={**query,'route':{'id':identity,'controller':'codex:router'}},(status,value)
        assert len(json.dumps(value))<1024
        assert gateway.api(SCOPE,{**query,'cwd':str(root/'other-project')})[0]==409
        assert gateway.api(SCOPE,{**query,'turnId':'unconfirmed'})[0]==409
        status,unrouted=gateway.api(SCOPE,{**query,'threadId':'unrouted'})
        assert status==200 and unrouted['route'] is None
        assert runtime.requests(root)==before,'Metadata lookup initialized, recovered or called a provider'
        with sqlite3.connect(root/'config/codex-thread-studio/studio.sqlite3') as db:assert db.execute('SELECT owner,phase,json FROM native_router_jobs WHERE id=?',(identity,)).fetchone()==stored
    finally:gateway.close()
    runtime.until(lambda:all(not Path('/proc',p.read_text().strip()).exists() for p in [root/'backend.pid',root/'opencode.pid'] if p.exists()),seconds=8)
    print(json.dumps({'ok':True,'artifacts':str(root),'restartLookup':True,'noProviderRpcOrRecovery':True,'recordUnchanged':True,'sourceIdentityGuards':True,'authenticated':True,'smallMetadataOnly':True,'ownedProvidersExited':True}))

if __name__=='__main__':main()
