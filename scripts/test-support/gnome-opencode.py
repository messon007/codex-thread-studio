#!/usr/bin/env python3
"""Private deterministic OpenCode HTTP/SSE fixture; never uses real sessions."""
import base64
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import queue
import sys
import threading
import time
from urllib.parse import urlparse, parse_qs, unquote

DIRECTORY = Path(__file__).resolve().parent
PROJECT = str(DIRECTORY / 'project')
SESSION = {'id': 'native-oc-thread', 'title': 'Native OpenCode acceptance', 'directory': PROJECT}
SESSIONS = {SESSION['id']: SESSION}
MESSAGES = [{'info': {'id': 'oc-history-user', 'role': 'user', 'sessionID': SESSION['id']},
             'parts': [{'id': 'oc-history-part', 'type': 'text', 'text': 'Native OpenCode history'}]}]
SUBSCRIBERS = []
PENDING = {'permission':{},'question':{}}
LOCK = threading.Lock()
BUSY = False
AUTH = 'Basic ' + base64.b64encode(('opencode:' + os.environ['OPENCODE_SERVER_PASSWORD']).encode()).decode()
OLDER = [
    {'info': {'id': 'oc-old-user', 'role': 'user', 'sessionID': SESSION['id']},
     'parts': [{'id': 'oc-old-question-part', 'type': 'text', 'text': 'An earlier OpenCode question'}]},
    {'info': {'id': 'oc-old-answer', 'role': 'assistant', 'parentID': 'oc-old-user', 'sessionID': SESSION['id']},
     'parts': [{'id': 'oc-old-answer-part', 'type': 'text', 'text': '# Earlier OpenCode answer\n\nnative-oc-old-marker 中文😀 is available from older pages.\n\n[Project document](nested/note.md:1)'}]},
]
FILLER = [
    {'info': {'id': f'oc-later-message-{index:04}', 'role': 'assistant', 'parentID': f'oc-later-user-{index:04}', 'sessionID': SESSION['id']},
     'parts': [{'id': f'oc-later-part-{index:04}', 'type': 'text', 'text': f'Later history entry {index}'}]}
    for index in range(201)
]


def event(kind, **properties):
    value = {'directory': PROJECT, 'payload': {'type': kind, 'properties': {'sessionID': SESSION['id'], **properties}}}
    with LOCK:
        category=kind.split('.')[0]
        if category in PENDING:
            if kind.endswith('.asked'):PENDING[category][properties['id']]=value['payload']['properties']
            elif properties.get('requestID'):PENDING[category].pop(properties['requestID'],None)
        for subscriber in SUBSCRIBERS:
            subscriber.put(value)


class Handler(BaseHTTPRequestHandler):
    protocol_version = 'HTTP/1.1'

    def log_message(self, *args):
        pass

    def authorized(self):
        if self.headers.get('Authorization') != AUTH:
            self.respond({'error': 'Authentication required'}, 401)
            return False
        return True

    def record(self, body=None):
        parsed=urlparse(self.path);path=unquote(parsed.path)+('?' + parsed.query if parsed.query else '')
        with LOCK, (DIRECTORY / 'opencode-requests.jsonl').open('a') as output:
            output.write(json.dumps({'method': self.command, 'path': path, 'rawPath':self.path,'body': body}) + '\n')

    def respond(self, value=None, status=200, headers=None):
        body = json.dumps(value).encode() if status != 204 else b''
        self.send_response(status)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(body)))
        for name, value in (headers or {}).items():
            self.send_header(name, value)
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        if not self.authorized():
            return
        self.record()
        path = unquote(urlparse(self.path).path)
        if path in ('/permission','/question'):
            assert parse_qs(urlparse(self.path).query)=={'directory':[PROJECT]}
            with LOCK:pending=list(PENDING[path[1:]].values())
            return self.respond(pending)
        if path == '/global/health':
            return self.respond({'healthy': True, 'version': 'native-test'})
        if path == '/global/event':
            self.send_response(200)
            self.send_header('Content-Type', 'text/event-stream')
            self.send_header('Cache-Control', 'no-cache')
            self.end_headers()
            subscriber = queue.Queue()
            with LOCK:
                SUBSCRIBERS.append(subscriber)
            try:
                self.wfile.write(b': private fixture\r\n\r\n')
                self.wfile.flush()
                while True:
                    try:
                        value = subscriber.get(timeout=0.2)
                        encoded = ('data: ' + json.dumps(value, ensure_ascii=False) + '\r\n\r\n').encode()
                        # Exercise actual transport boundaries inside UTF-8.
                        split = min(len(encoded), 224)
                        for offset in range(0, split, 7):
                            self.wfile.write(encoded[offset:min(offset + 7, split)])
                        for offset in range(split, len(encoded), 65536):
                            self.wfile.write(encoded[offset:offset + 65536])
                        self.wfile.flush()
                    except queue.Empty:
                        self.wfile.write(b': heartbeat\r\n\r\n')
                        self.wfile.flush()
            except (BrokenPipeError, ConnectionResetError):
                pass
            finally:
                with LOCK:
                    SUBSCRIBERS.remove(subscriber)
            return
        if path == '/find/file':
            return self.respond(['nested/说明 note.md', 'nested/note.md'])
        if path == '/command':
            return self.respond([{'name':'native-build','description':'Native build skill 中文😀','source':'skill'}])
        if path == '/experimental/session':
            return self.respond(list(SESSIONS.values()))
        if path == '/config/providers':
            return self.respond({'providers': [{'id': 'native-provider', 'models': {'native-model': {'id': 'native-model'}}}]})
        if path == '/session/status':
            return self.respond({SESSION['id']: {'type': 'busy' if BUSY else 'idle'}})
        if path == '/session/native-managed-opencode-id':
            workspace=str(Path(os.environ['XDG_DATA_HOME']) / 'codex-thread-studio/router')
            assert parse_qs(urlparse(self.path).query)=={'directory':[workspace]}
            return self.respond({'id':'native-managed-opencode-id','title':'Thread Router','directory':workspace,
                                 'permission':[{'permission':'*','pattern':'*','action':'deny'}]})
        if path.startswith('/session/') and path.split('/')[-1] in SESSIONS:
            return self.respond(SESSIONS[path.split('/')[-1]])
        if path == '/session/native-oc-other-thread':
            return self.respond({**SESSION, 'id': 'native-oc-other-thread', 'title': 'Other OpenCode source'})
        if path.startswith('/session/') and path.endswith('/message') and path.split('/')[2] in {*SESSIONS, 'native-oc-other-thread'}:
            with LOCK:
                messages = OLDER + FILLER + list(MESSAGES)
            query = parse_qs(urlparse(self.path).query)
            limit = min(200, max(1, int(query.get('limit', ['200'])[0])))
            cursor = query.get('before', [None])[0]
            repeated = (DIRECTORY / 'native-oc-repeat-cursor.flag').exists()
            end = len(messages) if cursor in (None, 'repeat-native-cursor') else int(cursor.removeprefix('native-before-'))
            start = max(0, end - limit)
            headers = {}
            if start and not (DIRECTORY / 'native-oc-no-cursor.flag').exists():
                headers['x-next-cursor'] = 'repeat-native-cursor' if repeated else f'native-before-{start}'
            return self.respond(messages[start:end], headers=headers)
        return self.respond({'error': 'Unknown private fixture endpoint'}, 404)

    def do_POST(self):
        global BUSY
        if not self.authorized():
            return
        body = json.loads(self.rfile.read(int(self.headers.get('Content-Length', '0'))) or b'null')
        self.record(body)
        path = unquote(urlparse(self.path).path)
        if path.startswith('/session/') and path.endswith('/fork'):
            fork = {**SESSION, 'id': 'native-oc-fork', 'title': 'Native OpenCode fork'}
            SESSIONS[fork['id']] = fork
            return self.respond(fork)
        if path.endswith('/shell'):
            assert 'messageID' not in body
            assert body['model']=={'providerID':'native-provider','modelID':'native-model'}, body
            identity='oc-shell-user-'+str(len(MESSAGES))
            user={'info':{'id':identity,'role':'user','sessionID':SESSION['id']},'parts':[{'id':identity+'-part','type':'text','text':'!'+body['command']}]}
            answer={'info':{'id':identity+'-assistant','role':'assistant','parentID':identity,'sessionID':SESSION['id']},'parts':[{'id':identity+'-output','type':'tool','tool':'bash','state':{'output':'Native OpenCode shell output 中文😀'}}]}
            with LOCK: MESSAGES.extend([user,answer])
            return self.respond(answer)
        if path.endswith('/command'):
            assert body['command'] in ('native-build','compact','review'), body
            assert 'messageID' not in body
            identity='oc-command-user-'+str(len(MESSAGES))
            user={'info':{'id':identity,'role':'user','sessionID':SESSION['id']},'parts':[{'id':identity+'-part','type':'text','text':body['arguments']}]}
            answer={'info':{'id':identity+'-assistant','role':'assistant','parentID':identity,'sessionID':SESSION['id']},'parts':[{'id':identity+'-answer','type':'text','text':'# Native skill command result\n\nReturned without SSE duplication. '+body['command']}]}
            with LOCK:
                MESSAGES.extend([user,answer])
            return self.respond(answer)
        if path.endswith('/prompt_async'):
            assert 'messageID' not in body, 'Client must not create OpenCode message identity'
            message_text = next((part.get('text','') for part in body['parts'] if part['type']=='text'),'')
            identity = 'oc-user-' + str(len(MESSAGES))
            user = {'info': {'id': identity, 'role': 'user', 'sessionID': SESSION['id']},
                    'parts': [{**part, 'id': identity + '-part-' + str(index)} for index, part in enumerate(body['parts'])]}
            with LOCK:
                MESSAGES.append(user)
            BUSY = True
            event('session.status', status={'type': 'busy'})
            event('message.updated', info=user['info'])
            for part in user['parts']:
                event('message.part.updated', part={**part, 'sessionID': SESSION['id'], 'messageID': identity})
            assistant = identity + '-assistant'
            event('message.updated', info={'id': assistant, 'role': 'assistant', 'sessionID': SESSION['id']})
            event('message.part.updated', part={'id': assistant, 'type': 'text', 'text': '', 'sessionID': SESSION['id'], 'messageID': assistant})
            event('message.part.delta', messageID=assistant, partID=assistant, field='text', delta='Native OpenCode 中文😀')
            event('message.updated', info={'id': assistant, 'role': 'assistant', 'sessionID': SESSION['id'], 'time': {'completed': 123}})
            if message_text=='native-oc-fast':
                structured={'nativeStructured':True,'message':'Which 中文 session? 😀'}
                info={'id':assistant,'role':'assistant','parentID':identity,'sessionID':SESSION['id'],'structured':structured,'time':{'completed':123}}
                with LOCK:MESSAGES.append({'info':info,'parts':[{'id':assistant,'type':'text','text':'Native OpenCode 中文😀'}]})
                event('message.updated',info=info)
            if message_text == 'native-oc-fast' or any(part['type']=='file' for part in body['parts']):
                BUSY = False
                event('session.idle')
                time.sleep(0.2)  # Idle intentionally precedes the HTTP acknowledgement.
            elif message_text == 'native-oc-ref-hold':
                def finish_reference_hold():
                    global BUSY
                    flag = DIRECTORY / 'release-native-oc-ref.flag'
                    while not flag.exists():
                        time.sleep(0.02)
                    BUSY = False
                    event('session.idle')
                threading.Thread(target=finish_reference_hold, daemon=True).start()
            elif message_text == 'native-oc-queue-hold':
                def finish_queue_hold():
                    global BUSY
                    flag = DIRECTORY / 'release-native-oc-queue.flag'
                    while not flag.exists():
                        time.sleep(0.02)
                    BUSY = False
                    event('session.idle')
                threading.Thread(target=finish_queue_hold, daemon=True).start()
            elif message_text.startswith('native-oc-queue-'):
                BUSY = False
                event('session.idle')
                time.sleep(0.2)
            elif message_text == 'native-oc-background':
                def complete_background():
                    global BUSY
                    # The background/Attention check must finish offscreen,
                    # independently of transport/rendering speed. Early idle
                    # before HTTP acknowledgement has its own fast-turn case.
                    flag = DIRECTORY / 'release-native-oc-background.flag'
                    while not flag.exists():
                        time.sleep(0.02)
                    BUSY = False
                    event('session.idle')
                threading.Thread(target=complete_background, daemon=True).start()
            else:
                event('permission.asked', id='native-permission', permission='bash', patterns=['echo native'])
            return self.respond(status=204)
        if path == '/permission/native-permission/reply':
            assert parse_qs(urlparse(self.path).query)=={'directory':[PROJECT]}
            assert body == {'reply': 'once'}, body
            event('permission.replied', requestID='native-permission')
            event('question.asked', id='native-question', questions=[{'question': 'Choose native mode', 'options': [{'label': 'GTK4', 'description': 'Native widgets'}], 'custom': False}])
            return self.respond(True)
        if path == '/question/native-question/reply':
            assert parse_qs(urlparse(self.path).query)=={'directory':[PROJECT]}
            assert body == {'answers': [['GTK4']]}, body
            event('question.replied', requestID='native-question')
            return self.respond(True)
        if path.endswith('/abort'):
            BUSY = False
            event('session.status', status={'type': 'idle'})
            return self.respond(True)
        return self.respond({'error': 'Unknown private fixture endpoint'}, 404)

    def do_PATCH(self):
        if not self.authorized():
            return
        body = json.loads(self.rfile.read(int(self.headers.get('Content-Length', '0'))))
        self.record(body)
        identity = unquote(urlparse(self.path).path.split('/')[-1])
        if identity in SESSIONS and isinstance(body.get('title'), str):
            SESSIONS[identity]['title'] = body['title']
            return self.respond(SESSIONS[identity])
        return self.respond({'error': 'Unknown private session'}, 404)

    def do_DELETE(self):
        if not self.authorized():
            return
        self.record()
        identity = unquote(urlparse(self.path).path.split('/')[-1])
        if identity in SESSIONS and identity != SESSION['id']:
            del SESSIONS[identity]
            return self.respond(True)
        return self.respond({'error': 'Only private forks may be deleted'}, 400)


(DIRECTORY / 'opencode.pid').write_text(str(os.getpid()))
port = int(sys.argv[sys.argv.index('--port') + 1])
server = ThreadingHTTPServer(('127.0.0.1', port), Handler)
server.daemon_threads = True
server.serve_forever()
