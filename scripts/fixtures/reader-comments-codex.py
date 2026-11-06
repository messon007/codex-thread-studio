#!/usr/bin/python3
"""Offline UI fixture: no model, shell, network, or inference operation exists."""
import json, os, sys, time
if '--version' in sys.argv:
    print('codex-cli 0.166.0'); sys.exit(0)
if 'app-server' not in sys.argv:
    print('unsupported fixture command', file=sys.stderr); sys.exit(2)
project = os.environ['STUDIO_SYNC_FIXTURE_PROJECT']
thread_id = '00000000-0000-4000-8000-000000000777'
thread = dict(id=thread_id, preview='Document integration fixture', modelProvider='fixture',
              createdAt=int(time.time()), updatedAt=int(time.time()), cwd=project,
              turns=[], status={'type':'idle'}, source='cli', ephemeral=False, gitInfo=None)
router={**thread,'id':'00000000-0000-4000-8000-000000000888','preview':'Router fixture'}
router_file=os.path.join(project,'fake-router.json')
try:
    with open(router_file) as saved: router=json.load(saved)
except FileNotFoundError: pass
for line in sys.stdin:
    try:
        message=json.loads(line)
        if 'id' not in message: continue
        method=message.get('method','')
        with open(os.path.join(project,'fake-rpc-methods.log'),'a') as trace: trace.write(method+'\n')
        if method=='thread/start':
            router.update(cwd=message.get('params',{}).get('cwd',project),name=message.get('params',{}).get('name','Thread Router'))
            with open(router_file,'w') as saved: json.dump(router,saved)
        if method=='thread/name/set' and message.get('params',{}).get('threadId')==router['id']:
            router['name']=message.get('params',{}).get('name','Thread Router')
            with open(router_file,'w') as saved: json.dump(router,saved)
        if method.startswith(('turn/','command/','process/')):
            response={'error':{'code':-32601,'message':'Inference and command execution disabled in fixture'}}
        else:
            values={
                'initialize':{'userAgent':'studio-offline-fixture','platformFamily':'unix','platformOs':'linux'},
                'thread/list':{'data':[thread],'nextCursor':None},
                'thread/loaded/list':{'data':[thread_id]},
                'thread/start':{'thread':router},
                'thread/read':{'thread':router if message.get('params',{}).get('threadId')==router['id'] else thread},
                'thread/resume':{'thread':router if message.get('params',{}).get('threadId')==router['id'] else thread},
                'model/list':{'data':[{'id':'fixture','model':'fixture','displayName':'Fixture',
                    'isDefault':True,'defaultReasoningEffort':'medium',
                    'supportedReasoningEfforts':[{'reasoningEffort':'medium','description':'Fixture'}]}],'nextCursor':None},
                'account/read':{'account':{'type':'apiKey'},'requiresOpenaiAuth':False},
                'account/rateLimits/read':{'rateLimits':{},'rateLimitsByLimitId':{}},
                'config/read':{'config':{'model':'fixture','model_provider':'fixture'},'origins':{},'layers':[]},
                'skills/list':{'data':[]}, 'app/list':{'data':[],'nextCursor':None},
                'mcpServerStatus/list':{'data':[],'nextCursor':None},
                'collaborationMode/list':{'data':[]},'configRequirements/read':{'requirements':None},
            }
            response={'result':values.get(method,{})}
        print(json.dumps({'id':message['id'],**response}),flush=True)
    except (ValueError,KeyError):
        sys.exit(1)
