#!/usr/bin/python3
"""Offline shared-proxy fixture. No daemon, inference or shell execution exists."""
import base64, hashlib, json, os, struct, sys, time
if '--version' in sys.argv:
    print('codex-cli 0.167.2'); sys.exit(0)
if '--help' in sys.argv or sys.argv[1:] == ['app-server', 'daemon', 'start']:
    sys.exit(0)
if sys.argv[1:] != ['app-server', 'proxy']:
    sys.exit(2)
project = os.environ['STUDIO_SYNC_FIXTURE_PROJECT']
thread_id = '00000000-0000-4000-8000-000000000777'
count = 402
thread = dict(id=thread_id, name='Large history fixture', preview='Large history fixture',
              modelProvider='fixture', createdAt=int(time.time()), updatedAt=int(time.time()),
              cwd=project, turns=[], status={'type':'idle'}, source='cli', ephemeral=False, gitInfo=None)
router = {**thread, 'id':'00000000-0000-4000-8000-000000000888', 'name':'Router fixture'}
def turn(i):
    return {'id':f'turn-{i}', 'status':'completed', 'error':None, 'items':[
        {'id':f'user-{i}', 'type':'userMessage', 'content':[{'type':'text','text':f'Fixture request {i}'}]},
        {'id':f'command-{i}', 'type':'commandExecution','command':'fixture-only','cwd':project,
         'status':'completed','commandActions':[], 'aggregatedOutput':'x'*176000,'exitCode':0,'durationMs':1},
        {'id':f'answer-{i}', 'type':'agentMessage','text':f'Fixture answer {i}'}]}
def page(end):
    return {'data':[turn(i) for i in range(end,max(0,end-4),-1)], 'nextCursor':str(end-4) if end>4 else None, 'backwardsCursor':None}
def exact(n):
    data=sys.stdin.buffer.read(n)
    if len(data)!=n: raise EOFError
    return data
def frame(data, opcode=1):
    if isinstance(data,str):data=data.encode()
    n=len(data)
    header=bytes([128|opcode,n]) if n<126 else bytes([128|opcode,126])+struct.pack('!H',n) if n<65536 else bytes([128|opcode,127])+struct.pack('!Q',n)
    sys.stdout.buffer.write(header+data);sys.stdout.buffer.flush()
header=b''
while not header.endswith(b'\r\n\r\n'):header+=exact(1)
key=next(line.split(b':',1)[1].strip() for line in header.split(b'\r\n') if line.lower().startswith(b'sec-websocket-key:'))
accept=base64.b64encode(hashlib.sha1(key+b'258EAFA5-E914-47DA-95CA-C5AB0DC85B11').digest())
sys.stdout.buffer.write(b'HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: '+accept+b'\r\n\r\n');sys.stdout.buffer.flush()
try:
    while True:
        a,b=exact(2);n=b&127
        if n==126:n=struct.unpack('!H',exact(2))[0]
        if n==127:n=struct.unpack('!Q',exact(8))[0]
        if n>1024*1024:raise ValueError('fixture input budget')
        mask=exact(4) if b&128 else None;data=exact(n)
        if mask:data=bytes(v^mask[i%4] for i,v in enumerate(data))
        if a&15==8:break
        if a&15==9:frame(data,10);continue
        if a&15!=1:continue
        message=json.loads(data)
        if 'id' not in message:continue
        method=message.get('method');params=message.get('params') or {}
        selected=thread if params.get('threadId')==thread_id else router
        result={}
        if method=='initialize':result={'userAgent':'studio-large-history-fixture','platformFamily':'unix','platformOs':'linux'}
        elif method=='thread/list':result={'data':[thread],'nextCursor':None}
        elif method=='thread/loaded/list':result={'data':[thread_id]}
        elif method=='thread/start':
            router.update(cwd=params.get('cwd',project),name=params.get('name','Thread Router'));result={'thread':router}
        elif method=='thread/name/set' and params.get('threadId')==router['id']:
            router['name']=params.get('name','Thread Router')
        elif method in ('thread/read','thread/resume'):
            full=selected['id']==thread_id and (params.get('includeTurns') if method=='thread/read' else not params.get('excludeTurns'))
            result={'thread':{**selected,'turns':[turn(i) for i in range(1,count+1)] if full else []}}
            if method=='thread/resume' and params.get('initialTurnsPage'):
                result['initialTurnsPage']=page(count) if selected['id']==thread_id else {'data':[],'nextCursor':None}
        elif method=='thread/turns/list':result=page(int(params.get('cursor') or count)) if selected['id']==thread_id else {'data':[],'nextCursor':None}
        elif method=='model/list':result={'data':[{'id':'fixture','model':'fixture','displayName':'Fixture','isDefault':True,'defaultReasoningEffort':'medium','supportedReasoningEfforts':[{'reasoningEffort':'medium','description':'Fixture'}]}],'nextCursor':None}
        elif method=='account/read':result={'account':{'type':'apiKey'},'requiresOpenaiAuth':False}
        elif method=='account/rateLimits/read':result={'rateLimits':{},'rateLimitsByLimitId':{}}
        elif method=='config/read':result={'config':{'model':'fixture','model_provider':'fixture'},'origins':{},'layers':[]}
        elif method in ('skills/list','app/list','mcpServerStatus/list','collaborationMode/list'):result={'data':[],'nextCursor':None}
        elif method=='configRequirements/read':result={'requirements':None}
        response={'id':message['id'],'result':result}
        if method.startswith(('turn/','command/','process/')):response={'id':message['id'],'error':{'code':-32601,'message':'Fixture disables inference and commands'}}
        raw=json.dumps(response,separators=(',',':')).encode()
        with open(os.path.join(project,'rpc.jsonl'),'a') as log:log.write(json.dumps({'method':method,'params':params,'bytes':len(raw)})+'\n')
        frame(raw)
except (EOFError,BrokenPipeError):pass
