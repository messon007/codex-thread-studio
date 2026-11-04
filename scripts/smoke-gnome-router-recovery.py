#!/usr/bin/env python3
"""Recover a genuinely uncertain private controller creation without replay."""
import importlib.util
import json
from pathlib import Path
import sqlite3
import sys
import tempfile
import time
from urllib.error import URLError

sys.dont_write_bytecode=True
sys.path.insert(0,str(Path(__file__).with_name('test-support')))
from native_accessibility import ProcessPeer
spec=importlib.util.spec_from_file_location('setup_fixture',Path(__file__).with_name('smoke-gnome-router.py'))
fixture=importlib.util.module_from_spec(spec);spec.loader.exec_module(fixture)
INSPECT='/studio/native/router/controller/recovery'
ADOPT='/studio/native/router/controller/adopt'

def close(gateway,root):
    peers=[]
    try:
        for name,script in [('backend.pid','backend.py'),('opencode.pid','opencode.py')]:
            path=root/name
            if not path.exists():continue
            identity=int(path.read_text())
            proc=Path('/proc',str(identity))
            try:
                fields=(proc/'stat').read_text().rsplit(')',1)[1].split()
            except FileNotFoundError:
                continue
            if fields[0]=='Z':continue
            assert int(fields[1])==gateway.process.pid, f'Unexpected recovery backend parent: {name}'
            assert str(root/script).encode() in (proc/'cmdline').read_bytes().split(b'\0'), f'Unexpected recovery backend command: {name}'
            peers.append((name,ProcessPeer(identity,(proc/'exe').resolve(strict=True))))
    finally:
        gateway.close()
    # An unreaping container PID 1 can retain dead children in /proc. Observe
    # the original process identity, as the installed desktop fixtures do.
    for name,peer in peers:
        for _ in range(100):
            if peer.poll() is not None:break
            time.sleep(.05)
        assert peer.poll() is not None,f'Owned recovery backend survived: {name}'

def exercise(binary,root,kind):
    fixture.initial(root)
    (root/'hold-create').touch()
    gateway=fixture.Gateway(binary,root,kind)
    try:
        revision=gateway.api()[1]['revision']
        assert gateway.api(INSPECT,{'backend':kind,'expectedRevision':revision},authorized=False)[0]==401
        assert gateway.api(ADOPT,{'backend':kind,'expectedRevision':revision,'journalRevision':'0'*16,'threadId':'unknown'},authorized=False)[0]==401
        try:gateway.setup(timeout=.25);raise AssertionError('Creation should remain pending')
        except (TimeoutError,URLError):pass
        fixture.until(lambda:gateway.count()==1)
        assert gateway.api(INSPECT,{'backend':kind,'expectedRevision':revision})[0]==409
        assert gateway.count()==1
    finally:close(gateway,root)
    gateway=fixture.Gateway(binary,root,kind)
    try:
        revision=gateway.api()[1]['revision']
        request={'backend':kind,'expectedRevision':revision}
        status,pending=gateway.api(INSPECT,request)
        assert status==200 and pending['state']=='uncertain',pending
        assert gateway.setup()[0]==409 and gateway.count()==1
        source=root/('controller-oc.json' if kind=='opencode' else 'controller.json')
        record=json.loads(source.read_text());identity=record['id']
        if kind=='opencode':record['permission']=[{'permission':'*','pattern':'*','action':'allow'}];source.write_text(json.dumps(record))
        original_settings=gateway.settings.read_bytes();original_source=source.read_bytes()
        reviewed_request={**request,'threadId':identity}
        for flag in ['wrong-directory','active-controller','archived-controller','child-controller']+(['ephemeral-controller','unknown-status'] if kind=='codex' else []):
            (root/flag).touch()
            assert gateway.api(INSPECT,reviewed_request)[0]==409,flag
            (root/flag).unlink()
        status,reviewed=gateway.api(INSPECT,reviewed_request)
        assert status==200 and reviewed['candidate']['threadId']==identity and reviewed['candidate']['workspace']==pending['workspace'],reviewed
        assert reviewed['candidate']['status']=='idle'
        assert gateway.settings.read_bytes()==original_settings and source.read_bytes()==original_source
        adoption={**request,'threadId':identity,'journalRevision':pending['journalRevision']}
        for bad in [{**adoption,'journalRevision':'0'*16},{**adoption,'expectedRevision':'0'*16}]:
            assert gateway.api(ADOPT,bad)[0]==409
        assert gateway.api(INSPECT,{**reviewed_request,'expectedRevision':'0'*16})[0]==409
        for bad in [{**adoption,'threadId':'x'*257},{**adoption,'extra':True},{**adoption,'threadId':'bad\0id'}]:
            assert gateway.api(ADOPT,bad)[0]==400
        assert gateway.settings.read_bytes()==original_settings and source.read_bytes()==original_source
        database=root/'config/codex-thread-studio/studio.sqlite3'
        with sqlite3.connect(database) as db:
            assert db.execute('SELECT thread_id FROM native_router_setup WHERE backend=?',(kind,)).fetchone()==(None,)
            db.execute('UPDATE native_router_setup SET workspace=? WHERE backend=?',(str(root/'other-project'),kind))
        assert gateway.api(ADOPT,adoption)[0]==409
        with sqlite3.connect(database) as db:db.execute('UPDATE native_router_setup SET workspace=? WHERE backend=?',(pending['workspace'],kind))
        if kind=='opencode':
            (root/'ignore-restriction').touch()
            status,refused=gateway.api(ADOPT,adoption)
            assert status==502 and 'restriction' in str(refused),refused
            assert gateway.settings.read_bytes()==original_settings
            status,known=gateway.api(INSPECT,request)
            assert status==200 and known['state']=='known' and known['threadId']==identity,known
            assert known['journalRevision']!=pending['journalRevision']
            assert gateway.api(ADOPT,adoption)[0]==409
            (root/'ignore-restriction').unlink()
            adoption['journalRevision']=known['journalRevision']
        status,adopted=gateway.api(ADOPT,adoption)
        assert status==200 and adopted['adopted'] and not adopted['created'] and adopted['threadId']==identity,adopted
        assert gateway.count()==1
        preferences=gateway.preferences()
        assert preferences['router']['controllers'][kind]==identity
        assert preferences['router']['controllerBackend']=='codex'
        assert preferences['router']['futureRouter']==42 and preferences['futureSetting']['preserve']
        assert preferences['router']['fallbacks']==json.loads(original_settings)['router']['fallbacks']
        if kind=='opencode':assert json.loads(source.read_text())['permission'][-1]=={'permission':'*','pattern':'*','action':'deny'}
        status,current=gateway.api(INSPECT,{'backend':kind,'expectedRevision':gateway.api()[1]['revision']})
        assert status==200 and current['state']=='configured' and current['threadId']==identity
        assert gateway.setup()[0]==200 and gateway.count()==1
        (root/'archived-controller').touch()
        assert gateway.setup()[0]==409 and gateway.count()==1
        (root/'archived-controller').unlink()
        requests=[json.loads(line) for line in (root/'requests.jsonl').read_text().splitlines()] if (root/'requests.jsonl').exists() else []
        assert not any(row.get('method')=='turn/start' for row in requests)
        if kind=='opencode':
            requests=[json.loads(line) for line in (root/'opencode-requests.jsonl').read_text().splitlines()]
            assert not any('/message' in row['path'] or '/prompt' in row['path'] for row in requests)
    finally:close(gateway,root)
    gateway=fixture.Gateway(binary,root,kind)
    try:
        assert gateway.setup()[0]==200 and gateway.count()==1
    finally:close(gateway,root)
    return {'uncertainCreate':True,'activeSetupGuard':True,'reviewReadOnly':True,'metadataGuards':True,'staleStateGuards':True,'explicitAdoption':True,'oneCreation':True,'restartVerified':True,'permissionFailureRetainsIdentity':kind=='opencode'}

def main():
    binary=Path(sys.argv[1]).resolve()
    base=Path(__file__).resolve().parents[1]/'target/gnome-native-smoke';base.mkdir(parents=True,exist_ok=True)
    root=Path(tempfile.mkdtemp(prefix='router-recovery-api-',dir=base))
    results={kind:exercise(binary,root/kind,kind) for kind in ['codex','opencode']}
    print(json.dumps({'ok':True,'artifacts':str(root),'backends':results,'privateProvidersOnly':True,'backendCleanup':True}))

if __name__=='__main__':main()
