#!/usr/bin/env python3
"""Gateway-owned utility isolation with private providers; no real prompts/files."""
import argparse, ast, concurrent.futures, http.client, importlib.util, json, resource, tempfile, time, sys
from pathlib import Path
from urllib.request import Request
from urllib.error import HTTPError
sys.dont_write_bytecode=True
spec=importlib.util.spec_from_file_location('utility_translation_fixture',Path(__file__).with_name('smoke-gnome-translation.py'))
translation=importlib.util.module_from_spec(spec);spec.loader.exec_module(translation)
runtime=translation.runtime

def opencode_source():
    text=translation.opencode_source().replace('utilities={}','utilities={}\nghosts={}').replace('str(len(utilities)+1)','str(len(utilities)+len(ghosts)+1)')
    text=text.replace("    if identity in utilities:return utilities[identity]", """    if identity in utilities:
        result=utilities[identity]
        if (root/'adopt-utility').exists():result={**result,'title':'User adopted session'}
        if (root/'wrong-utility-read').exists():result={**result,'id':'oc-target'}
        return result
    if identity in ghosts:return ghosts[identity]""")
    text=text.replace("if path=='/experimental/session':return self.response([session('oc-router'),session('oc-target'),{**session('oc-similar'),'title':'Studio translation user notes'},*[session(identity) for identity in list(utilities)]])", """if path=='/experimental/session':
            ordinary=[session('oc-router'),session('oc-target'),{**session('oc-similar'),'title':'Studio translation user notes'}]
            if (root/'large-catalog').exists():ordinary=[{**value,'summary':{'private':'x'*(2*1024*1024)}} for value in ordinary]
            owned=[session(identity) for identity in list(utilities)]
            if (root/'catalog-full').exists() and owned:
                if query.get('cursor'):return self.response([{**session('oc-after'),'title':'After filtered tail'}])
                rows=[{**session('oc-page-'+str(i)),'time':{'updated':1000-i}} for i in range(99)]
                return self.response([*rows,{**owned[0],'time':{'updated':901}}])
            return self.response([*ordinary,*owned,*ghosts.values()])""")
    text=text.replace("            return self.response(utilities[identity])", """            if (root/'inherited-permission').exists():utilities[identity]['permission']=[{'permission':'bash','pattern':'*','action':'allow'},*utilities[identity]['permission']]
            if (root/'unsafe-permission').exists():utilities[identity]['permission'].append({'permission':'bash','pattern':'*','action':'allow'})
            result=utilities[identity]
            if (root/'wrong-utility-create').exists():result={**result,'id':'oc-target'}
            if (root/'hold-utility-create').exists():
                (root/'utility-created').write_text(identity)
                while not (root/'release-utility-create').exists():time.sleep(.01)
            return self.response(result)""")
    text=text.replace("            utilities.pop(identity);return self.response(True)", """            ghosts[identity]=utilities.pop(identity)
            if (root/'lose-delete-ack').exists():self.close_connection=True;return
            return self.response(True)""")
    text=text.replace("            ghosts[identity]=utilities.pop(identity)","            if (root/'false-delete-ack').exists():return self.response(False)\n            ghosts[identity]=utilities.pop(identity)")
    ast.parse(text);return text

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('gateway');args=parser.parse_args();resource.setrlimit(resource.RLIMIT_CORE,(0,0))
    base=Path(__file__).resolve().parents[1]/'target/gnome-native-smoke';base.mkdir(exist_ok=True,parents=True);root=Path(tempfile.mkdtemp(prefix='utility-sessions-',dir=base));runtime.create_profile(root)
    runtime.fixture.BACKEND=translation.codex_source();runtime.fixture.OPENCODE_BACKEND=opencode_source()
    service=runtime.fixture.Gateway(Path(args.gateway).resolve(),root,'opencode')
    memory=translation.ProcessMemory(service.process.pid,args.gateway)
    def request(value,status=200,auth=True,origin=None):
        headers={'Content-Type':'application/json'}
        if auth:headers['Authorization']='Bearer '+service.token
        if origin:headers['Origin']=origin
        req=Request(service.origin+'/studio/native/utility-session',data=json.dumps(value).encode(),headers=headers)
        try:response=service.client.open(req,timeout=45)
        except HTTPError as error:response=error
        with response:
            body=response.read();assert response.status==status,(root,response.status,status,body[:600]);return json.loads(body) if body else None
    def create():return request(dict(operation='create',backend='opencode',sourceThreadId='oc-target',cwd=str(root/'project-oc')))
    def catalog(cursor=None):return request(dict(operation='catalog',backend='opencode',cursor=cursor))
    def calls():
        path=root/'opencode-requests.jsonl';return [json.loads(line) for line in path.read_text().splitlines()] if path.exists() else []
    def closes(identity):return [r for r in calls() if r['method']=='DELETE' and r['path']=='/session/'+identity]
    def targets():
        status,value=service.api('/studio/native/router/targets?controller=codex%3Arouter');assert status==200,(status,value);return {row['sessionKey'] for row in value['targets']}
    def event_response(path='/studio/native/opencode-events',status=200,auth=True,origin=None):
        headers={'Accept':'text/event-stream'}
        if auth:headers['Authorization']='Bearer '+service.token
        if origin:headers['Origin']=origin
        try:response=service.client.open(Request(service.origin+path,headers=headers),timeout=8)
        except HTTPError as error:response=error
        if status is not None:assert response.status==status,(root,response.status,status)
        return response
    def events(values,raw=False):
        (root/'sdk-events.json').write_text(json.dumps(values,ensure_ascii=False))
        result=[];fields={};data=[]
        with event_response('/opencode/global/event' if raw else '/studio/native/opencode-events') as response:
            while True:
                line=response.readline()
                assert line,'Event stream ended before barrier'
                line=line.decode().rstrip('\r\n')
                if not line:
                    if data:
                        value=json.loads('\n'.join(data));result.append(value)
                        assert fields.get('event')=='message' and fields.get('retry')=='1000' and fields.get('id','').startswith('fixture-'),fields
                        if value.get('type')=='fixture.barrier':return result
                    fields={};data=[]
                elif line.startswith('data:'):data.append(line[5:].lstrip(' '))
                elif ':' in line:
                    key,value=line.lstrip('\ufeff').split(':',1);fields[key]=value.strip()
    def rejects_stream():
        with event_response() as response:
            try:assert not response.read(),'Invalid event bytes reached the client'
            except (http.client.IncompleteRead,http.client.RemoteDisconnected) as error:
                assert not getattr(error,'partial',b''),'Invalid event body was partially forwarded'
    def status_event(identity,status='busy'):
        return {'type':'session.status','properties':{'sessionID':identity,'status':{'type':status}}}
    barrier={'type':'fixture.barrier','properties':{'text':'Preserved Unicode 中文😀'}}
    try:
        empty=dict(operation='catalog',backend='opencode')
        request(empty,status=401,auth=False);request(empty,status=403,origin='https://foreign.invalid')
        for value in [dict(operation='unknown'),dict(operation='create',backend='opencode',sourceThreadId='oc-target',cwd='relative'),dict(operation='close',lease='../source'),dict(empty,unexpected=True)]:request(value,status=400)
        request(dict(empty,backend='missing'),status=404)
        request(dict(empty,cursor='x'*4097),status=400)
        with event_response(status=401,auth=False):pass
        with event_response(status=403,origin='https://foreign.invalid'):pass
        (root/'large-catalog').touch();page=catalog();assert len(json.dumps(page).encode())<16*1024 and 'private' not in json.dumps(page)
        (root/'large-catalog').unlink()
        # While the SDK has created a row but withholds its identity ACK, the
        # exact reserved nonce/project filters it without blocking catalog I/O.
        (root/'hold-utility-create').touch()
        with concurrent.futures.ThreadPoolExecutor(max_workers=1) as executor:
            pending=executor.submit(create);runtime.until(lambda:(root/'utility-created').exists())
            identity=(root/'utility-created').read_text();started=time.monotonic();page=catalog()
            assert time.monotonic()-started<3,'Catalog blocked behind creation'
            ids={r['id'] for r in page['data']};assert identity not in ids and {'oc-target','oc-similar'}.issubset(ids),page
            assert 'opencode:'+identity not in targets() and 'opencode:oc-similar' in targets()
            # Creation observation is filtering evidence, never a cleanup lease.
            nonce=next(r['body']['title'] for r in calls() if r['method']=='POST' and r['path']=='/session')
            metadata={'type':'session.created','properties':{'info':{'id':identity,'title':nonce,'directory':str(root/'project-oc')}}}
            ordinary=status_event('oc-similar')
            assert events([metadata,status_event(identity),ordinary,barrier])==[ordinary,barrier]
            request(dict(operation='close',lease=nonce.removeprefix('Studio translation ')),status=409)
            assert not closes(identity),'An event observation authorized cleanup before creation acknowledgment'
            (root/'release-utility-create').touch();receipt=pending.result(timeout=10)
        (root/'hold-utility-create').unlink();(root/'release-utility-create').unlink()
        lease=receipt['lease'];identity=receipt['session']['id'];assert len(lease)==32 and receipt['session']['directory']==str(root/'project-oc')
        rejected=runtime.finished(service,runtime.submit(service,runtime.body(service,'Must never reach a utility',{'sessionKey':'opencode:'+identity,'cwd':str(root/'project-oc')})),'failed')
        assert 'utility sessions are not Router targets' in rejected['error'],rejected
        ordinary=status_event('oc-similar');source=status_event('oc-target','idle')
        private=[status_event(identity),{'payload':{'type':'message.updated','properties':{'info':{'sessionID':identity,'id':'private'}}}},{'type':'message.part.updated','properties':{'part':{'sessionID':identity,'text':'Private utility text 中文😀'}}}]
        assert events([*private,ordinary,source,barrier])==[ordinary,source,barrier]
        assert events([*private,barrier],raw=True)==[*private,barrier],'Legacy raw proxy was filtered'
        # The hidden tail still determines the upstream cursor. A following real
        # session is reachable even though only 99 rows remain in the first page.
        (root/'catalog-full').touch();page=catalog();assert len(page['data'])==99 and page['nextCursor']=='901',page
        following=catalog(page['nextCursor']);assert [r['id'] for r in following['data']]==['oc-after'] and following['nextCursor'] is None
        (root/'catalog-full').unlink()
        request(dict(operation='close',lease=lease));request(dict(operation='close',lease=lease));assert len(closes(identity))==1
        assert identity not in {r['id'] for r in catalog()['data']},'Late deleted row leaked'
        assert events([status_event(identity,'idle'),barrier])==[barrier]
        # A later allow invalidates denial; a previous allow before the final
        # catch-all is permitted, matching the existing Router confirmation.
        (root/'inherited-permission').touch();receipt=create();request(dict(operation='close',lease=receipt['lease']));(root/'inherited-permission').unlink()
        before=len([r for r in calls() if r['method']=='DELETE']);(root/'unsafe-permission').touch();create_value=dict(operation='create',backend='opencode',sourceThreadId='oc-target',cwd=str(root/'project-oc'));request(create_value,status=502);(root/'unsafe-permission').unlink()
        assert len([r for r in calls() if r['method']=='DELETE'])==before+1
        # Changed metadata relinquishes visibility but cannot authorize deletion.
        receipt=create();identity=receipt['session']['id'];before=len(closes(identity));(root/'adopt-utility').touch()
        request(dict(operation='close',lease=receipt['lease']),status=502);request(dict(operation='close',lease=receipt['lease']),status=409)
        assert len(closes(identity))==before and identity in {r['id'] for r in catalog()['data']}
        assert events([status_event(identity),barrier])==[status_event(identity),barrier],'Adopted user events were hidden'
        (root/'adopt-utility').unlink()
        # A wrong source identity in the creation ACK never authorizes prompting
        # or deletion. The unknown side effect is not retried.
        (root/'wrong-utility-create').touch();before=len([r for r in calls() if r['method']=='POST' and r['path']=='/session']);request(create_value,status=502);(root/'wrong-utility-create').unlink()
        assert len([r for r in calls() if r['method']=='POST' and r['path']=='/session'])==before+1
        # An accepted deletion with a missing ACK is still a single attempt.
        receipt=create();identity=receipt['session']['id'];before=len(closes(identity));(root/'lose-delete-ack').touch()
        request(dict(operation='close',lease=receipt['lease']),status=502);request(dict(operation='close',lease=receipt['lease']),status=409)
        assert len(closes(identity))==before+1
        (root/'lose-delete-ack').unlink()
        receipt=create();identity=receipt['session']['id'];before=len(closes(identity));(root/'false-delete-ack').touch()
        request(dict(operation='close',lease=receipt['lease']),status=502);request(dict(operation='close',lease=receipt['lease']),status=409)
        assert len(closes(identity))==before+1
        (root/'false-delete-ack').unlink()
        active=[create() for _ in range(4)]
        before=len([r for r in calls() if r['method']=='POST' and r['path']=='/session'])
        request(create_value,status=429)
        assert len([r for r in calls() if r['method']=='POST' and r['path']=='/session'])==before
        for receipt in active:request(dict(operation='close',lease=receipt['lease']))
        connections=[]
        try:
            for _ in range(4):connections.append(event_response())
            with event_response(status=429):pass
            connections.pop().close()
            deadline=time.monotonic()+3
            while True:
                response=event_response(status=None)
                if response.status==200:connections.append(response);break
                response.close();assert time.monotonic()<deadline,'Closed stream retained its admission slot';time.sleep(.02)
        finally:
            for response in connections:response.close()
        large={'type':'fixture.large','properties':{'text':'x'*(13*1024*1024)}}
        started=time.monotonic();assert events([large,barrier])==[large,barrier];large_ms=round((time.monotonic()-started)*1000)
        del large
        (root/'sdk-events.json').write_text(json.dumps([{'type':'fixture.large','properties':{'text':'x'*(16*1024*1024)}}]))
        started=time.monotonic()
        rejects_stream()
        rejection_ms=round((time.monotonic()-started)*1000)
        (root/'sdk-events.json').write_text('[]')
        (root/'raw-events.bin').write_bytes(b'data: {invalid}\r\n\r\n')
        rejects_stream();(root/'raw-events.bin').unlink()
        (root/'raw-events.bin').write_bytes(b'data: \xff\r\n\r\n')
        rejects_stream();(root/'raw-events.bin').unlink()
        (root/'raw-events.bin').write_bytes(b'id: metadata-only\rretry: 22\r\rdata: '+json.dumps(barrier).encode()+b'\r\r')
        with event_response() as response:wire=response.read().decode()
        assert 'id:metadata-only' in wire.replace(' ','') and 'retry:22' in wire.replace(' ',''),wire
        assert json.loads(next(line[5:].strip() for line in wire.splitlines() if line.startswith('data:')))==barrier
        (root/'raw-events.bin').unlink()
        assert events([barrier])==[barrier],'An invalid stream damaged later clients'
        assert not [r for r in calls() if r['path'].endswith('/prompt_async')]
        assert not [r for r in calls() if r['method']!='GET' and r['path'].startswith('/session/oc-target')],'Source session mutated'
        assert not runtime.starts(root),'API verification sent a model prompt'
        measured=memory.close();assert measured['gatewayPeakRssKiB']>0
        print(json.dumps(dict(ok=True,authOrigin=True,pendingCreationHidden=True,userNamesPreserved=True,routerTargetsFiltered=True,explicitUtilityTargetRejected=True,originalCursor=True,lateCatalogFiltered=True,orderedPermissions=True,adoptionPreserved=True,wrongIdentityRejected=True,deleteNotReplayed=True,sourceUnchanged=True,utilityEventsFiltered=True,ordinaryEventsPreserved=True,adoptedEventsPreserved=True,legacyProxyUnchanged=True,fragmentedUnicodeBom=True,eventFieldsPreserved=True,streamAdmissionBounded=True,streamSlotReleased=True,validLargeEventBytes=13*1024*1024,validLargeEventMs=large_ms,oversizedEventRejected=True,oversizedEventRejectionMs=rejection_ms,malformedEventRejected=True,gatewayPeakRssKiB=measured['gatewayPeakRssKiB'],artifacts=str(root))))
    finally:
        memory.close()
        service.close()
        for name in ['sdk-events.json','raw-events.bin']:(root/name).unlink(missing_ok=True)
if __name__=='__main__':main()
