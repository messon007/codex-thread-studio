#!/usr/bin/env python3
"""Probe the installed Codex paging/resume contract against a local fake model.

All SDK state, workspaces and inference traffic stay inside an owned fixture.
Does not read production credentials or send an inference request to OpenAI.
"""
import argparse
import json
import os
from pathlib import Path
import queue
import resource
import subprocess
import tempfile
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


class Client:
    def __init__(self, executable, root, port):
        home=root/'codex';home.mkdir();project=root/'project';project.mkdir()
        (home/'config.toml').write_text(f'''model = "fixture-model"
model_provider = "fixture"
approval_policy = "never"
sandbox_mode = "read-only"
[model_providers.fixture]
name = "Local hydration fixture"
base_url = "http://127.0.0.1:{port}/v1"
wire_api = "responses"
env_key = "STUDIO_FAKE_API_KEY"
requires_openai_auth = false
supports_websockets = false
''')
        self.stderr=(root/'sdk.log').open('w')
        env=dict(os.environ,CODEX_HOME=str(home),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),STUDIO_FAKE_API_KEY='local-fixture-only',OPENAI_API_KEY='')
        self.child=subprocess.Popen([executable,'app-server','--listen','stdio://'],cwd=project,env=env,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=self.stderr,text=True,bufsize=1)
        self.incoming=queue.Queue();self.messages=[];self.sequence=0
        def read():
            for line in self.child.stdout:
                try:self.incoming.put(json.loads(line))
                except ValueError:self.incoming.put({'fixtureError':'Non-JSON SDK output'})
        self.reader=threading.Thread(target=read,daemon=True);self.reader.start()
    def send(self,value):
        self.child.stdin.write(json.dumps(value)+'\n');self.child.stdin.flush()
    def wait(self,test,timeout=15):
        end=time.monotonic()+timeout
        while time.monotonic()<end:
            for value in self.messages:
                if test(value):return value
            try:value=self.incoming.get(timeout=min(.2,max(.001,end-time.monotonic())))
            except queue.Empty:
                if self.child.poll() is not None:raise AssertionError('Codex exited: '+str(self.child.returncode))
                continue
            self.messages.append(value)
        raise AssertionError('SDK contract deadline exceeded; observed methods: '+repr([v.get('method') for v in self.messages][-15:]))
    def rpc(self,method,params):
        self.sequence+=1;identity=self.sequence
        self.send({'id':identity,'method':method,'params':params})
        value=self.wait(lambda v:v.get('id')==identity and 'method' not in v)
        if 'error' in value:raise AssertionError((method,value['error']))
        return value['result']
    def close(self):
        if self.child.poll() is None:
            self.child.stdin.close()
            try:self.child.wait(timeout=5)
            except subprocess.TimeoutExpired:self.child.terminate();self.child.wait(timeout=5)
        self.stderr.close()


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('codex');args=parser.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE,(0,0))
    base=Path(__file__).resolve().parents[2]/'target/gnome-native-smoke';base.mkdir(parents=True,exist_ok=True)
    root=Path(tempfile.mkdtemp(prefix='sdk-hydration-',dir=base));release=threading.Event();requests=[]
    class Handler(BaseHTTPRequestHandler):
        def log_message(self,*_):pass
        def do_POST(self):
            size=int(self.headers.get('Content-Length',0));assert size<16*1024*1024
            self.rfile.read(size);requests.append(self.path)
            assert self.path=='/v1/responses'
            self.send_response(200);self.send_header('Content-Type','text/event-stream');self.send_header('Cache-Control','no-cache');self.end_headers()
            sequence=0
            def emit(kind,**fields):
                nonlocal sequence
                value={'type':kind,'sequence_number':sequence,**fields};sequence+=1
                self.wfile.write(('event: '+kind+'\ndata: '+json.dumps(value)+'\n\n').encode());self.wfile.flush()
            response={'id':'resp-fixture','object':'response','created_at':1,'status':'in_progress','model':'fixture-model','output':[]}
            item={'id':'msg-fixture','type':'message','role':'assistant','status':'in_progress','phase':'final_answer','content':[]}
            emit('response.created',response=response)
            emit('response.output_item.added',output_index=0,item=item)
            emit('response.output_text.delta',item_id='msg-fixture',output_index=0,content_index=0,delta='First 中文 ')
            assert release.wait(timeout=30)
            emit('response.output_text.delta',item_id='msg-fixture',output_index=0,content_index=0,delta='second 😀')
            item={**item,'status':'completed','content':[{'type':'output_text','text':'First 中文 second 😀','annotations':[]}]}
            emit('response.output_item.done',output_index=0,item=item)
            emit('response.completed',response={**response,'status':'completed','output':[item],'usage':{'input_tokens':1,'output_tokens':2,'total_tokens':3}})
    server=ThreadingHTTPServer(('127.0.0.1',0),Handler)
    serving=threading.Thread(target=server.serve_forever,daemon=True);serving.start()
    client=None
    try:
        client=Client(str(Path(args.codex).resolve()),root,server.server_port)
        client.rpc('initialize',{'clientInfo':{'name':'studio_hydration_fixture','title':'Native hydration fixture','version':'0.1'},'capabilities':{'experimentalApi':True}})
        client.send({'method':'initialized','params':{}})
        thread=client.rpc('thread/start',{'cwd':str(root/'project'),'model':'fixture-model','modelProvider':'fixture','approvalPolicy':'never','sandbox':'read-only'})['thread']['id']
        client.rpc('turn/start',{'threadId':thread,'input':[{'type':'text','text':'Local protocol fixture; return the supplied fake text.'}]})
        client.wait(lambda v:v.get('method')=='item/agentMessage/delta')
        resumed=client.rpc('thread/resume',{'threadId':thread,'excludeTurns':True})
        params={'threadId':thread,'limit':120,'sortDirection':'desc'}
        if resumed.get('itemsBackwardsCursor'):params['cursor']=resumed['itemsBackwardsCursor']
        items=client.rpc('thread/items/list',params)
        states=client.rpc('thread/turns/list',{'threadId':thread,'limit':4,'sortDirection':'desc','itemsView':'summary'})
        full_turns=client.rpc('thread/turns/list',{'threadId':thread,'limit':4,'sortDirection':'desc','itemsView':'full'})
        full_read=client.rpc('thread/read',{'threadId':thread,'includeTurns':True})
        release.set();client.wait(lambda v:v.get('method')=='turn/completed')
        completed=client.rpc('thread/items/list',{'threadId':thread,'limit':120,'sortDirection':'desc'})
        assert resumed['thread']['id']==thread and not resumed['thread'].get('turns')
        assert isinstance(resumed.get('itemsBackwardsCursor'),str) and resumed['itemsBackwardsCursor']
        assert isinstance(resumed.get('turnsBackwardsCursor'),str) and resumed['turnsBackwardsCursor']
        assert any(turn['status']=='inProgress' for turn in states['data'])
        assert not any(entry['item'].get('type')=='agentMessage' for entry in items['data']),'Probe contract changed: active text is now persisted'
        assert not any(item.get('type')=='agentMessage' for turn in full_read['thread']['turns'] for item in turn['items']),'Probe contract changed: full reads now return active text'
        assert any(entry['item'].get('text')=='First 中文 second 😀' for entry in completed['data'])
        assert requests==['/v1/responses'],requests
        result={'ok':True,'requests':requests,'resume':resumed,'activeItems':items,'turnStates':states,'fullTurnPage':full_turns,'fullRead':full_read,'completedItems':completed,'events':[v for v in client.messages if v.get('method') in ['item/started','item/agentMessage/delta','item/completed','turn/started','turn/completed']]}
        (root/'contract.json').write_text(json.dumps(result,ensure_ascii=False,indent=2))
        print(json.dumps({'ok':True,'artifacts':str(root),'resumeTurns':len(resumed['thread'].get('turns',[])),'itemCursor':bool(resumed.get('itemsBackwardsCursor')),'activeItemCount':len(items['data']),'turnStateCount':len(states['data'])}))
    finally:
        release.set()
        if client:client.close()
        server.shutdown();server.server_close()


if __name__=='__main__':main()
