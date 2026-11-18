"""Read-only MCP echo fixture. No credentials or production state."""
import sys, json, time, threading
from concurrent.futures import ThreadPoolExecutor

output_lock=threading.Lock()
log_lock=threading.Lock()

def handle(message):
    method=message.get('method')
    result={}
    if method=='initialize':
        result={'protocolVersion':'2024-11-05','capabilities':{'tools':{}},'serverInfo':{'name':'teams-probe-echo','version':'0.1'}}
    elif method=='tools/list':
        result={'tools':[{'name':'echo','description':'Read-only synthetic probe. Echo a fixture value.', 'inputSchema':{'type':'object','properties':{'value':{'type':'string'}},'required':['value'],'additionalProperties':False},'annotations':{'readOnlyHint':True,'destructiveHint':False}}]}
    elif method=='resources/list':result={'resources':[]}
    elif method=='resources/templates/list':result={'resourceTemplates':[]}
    elif method=='prompts/list':result={'prompts':[]}
    elif method=='tools/call':
        with log_lock:
            with open(sys.argv[1],'a') as f:f.write(json.dumps({'method':method,'params':message.get('params',{})},sort_keys=True)+'\n')
        time.sleep(0.7)
        result={'content':[{'type':'text','text':json.dumps({'value':message.get('params',{}).get('arguments'), 'observed_meta':message.get('params',{}).get('_meta')},sort_keys=True)}]}
    if 'id' in message:
        with output_lock:
            print(json.dumps({'jsonrpc':'2.0','id':message['id'],'result':result}),flush=True)

with ThreadPoolExecutor(max_workers=4) as pool:
    for line in sys.stdin:
        try:message=json.loads(line)
        except ValueError:continue
        pool.submit(handle,message)
