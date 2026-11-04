"""Fixture-only proxy: fake Ollama routes, authenticated pass-through to an owned gateway.

Never connects to port 11434. Production gateway routing is unchanged. The native
client still exercises its real WebSocket, HTTP, configuration and provider code.
"""
import http.client
import http.server
import html
import json
import os
from pathlib import Path
import select
import signal
import socket
import subprocess
import sys
import threading
from urllib.parse import urlsplit


def main(gateway, argv, *, faults=False):
    root=Path(os.environ.get('STUDIO_GNOME_PRIVATE_PROXY_DIR') or os.environ['STUDIO_GNOME_SMOKE_DIR'])
    child=subprocess.Popen([gateway,*argv],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=sys.stderr,text=True,bufsize=1)
    (root/('recovery-gateway.pid' if faults else 'translation-gateway.pid')).write_text(str(child.pid))
    (root/('recovery-proxy.pid' if faults else 'translation-proxy.pid')).write_text(str(os.getpid()))
    if faults:
        with (root/'recovery-launch-pids.jsonl').open('a') as output:
            output.write(json.dumps({'proxy':os.getpid(),'gateway':child.pid})+'\n')
    actual=None
    for _ in range(8):
        line=child.stdout.readline()
        if not line:raise RuntimeError('Owned gateway exited before its endpoint')
        if 'then open: ' in line:
            actual=urlsplit(line.split('then open: ',1)[1].strip());break
    if not actual:raise RuntimeError('Owned gateway provided no endpoint')
    if actual.hostname!='127.0.0.1' or not actual.fragment.startswith('token='):raise RuntimeError('Invalid owned gateway endpoint')
    token=actual.fragment[6:];origin=f'http://127.0.0.1:{actual.port}'
    lock=threading.Lock()
    class Proxy(http.server.BaseHTTPRequestHandler):
        protocol_version='HTTP/1.1'
        def log_message(self,*_):pass
        def reply(self,value,status=200):
            body=json.dumps(value,ensure_ascii=False).encode();self.send_response(status);self.send_header('Content-Type','application/json');self.send_header('Content-Length',str(len(body)));self.end_headers()
            try:self.wfile.write(body)
            except (BrokenPipeError,ConnectionResetError):pass  # A cancelled private HTTP reader.
        def handle_request(self):
            public=f'http://127.0.0.1:{self.server.server_port}'
            if self.command=='GET' and self.path=='/fixture-selection':
                text='Unsent draft: Translate **English** <b>literal</b> 中文😀\n.\nCANCEL all\n!.. raw text'
                body=('<!doctype html><meta charset="utf-8"><p id="quote">'+html.escape(text)+'</p>').encode()
                self.send_response(200);self.send_header('Content-Type','text/html; charset=utf-8');self.send_header('Content-Length',str(len(body)));self.end_headers();self.wfile.write(body);return
            if self.headers.get('Origin') not in [None,public]:return self.reply({'error':'untrusted fixture origin'},403)
            websocket=self.headers.get('Upgrade','').lower()=='websocket'
            if self.headers.get('Authorization')!='Bearer '+token and not (websocket and self.headers.get('Sec-WebSocket-Protocol')=='codex-thread-studio.auth.'+token):return self.reply({'error':'missing fixture credential'},401)
            length=int(self.headers.get('Content-Length','0'))
            if length>256*1024:return self.reply({'error':'fixture input budget'},413)
            body=self.rfile.read(length) if length else b''
            if faults and self.path == '/studio/backends':
                import time
                gate=root/'discovery-fault'
                status=int(gate.read_text()) if gate.exists() else 200
                with lock,(root/'discovery-requests.jsonl').open('a') as output:
                    output.write(json.dumps(dict(method=self.command,status=status,at=time.time_ns()))+'\n')
                if status != 200:return self.reply({'error':'private discovery diagnostic <b>literal</b> 中文😀'},status)
            if faults and self.path == '/studio/native/epub' and self.command == 'POST':
                value=json.loads(body) if body else {}
                if value.get('operation') == 'search':
                    with lock,(root/'epub-search-requests.jsonl').open('a') as output:
                        output.write(json.dumps(value,ensure_ascii=False)+'\n')
            if faults and self.path == '/studio/native/pdf' and self.command == 'POST':
                value=json.loads(body) if body else {}
                if value.get('operation') == 'open':
                    with lock,(root/'pdf-open-requests.jsonl').open('a') as output:
                        output.write(json.dumps(dict(id=value.get('id'),passwordProvided=bool(value.get('password'))))+'\n')
            if faults and self.path == '/studio/native/tables' and self.command == 'POST':
                value=json.loads(body) if body else {}
                if value.get('operation') == 'query':
                    with lock,(root/'table-query-requests.jsonl').open('a') as output:
                        output.write(json.dumps(dict(id=value.get('id'),sheet=value.get('sheet'),sql=value.get('sql')),ensure_ascii=False)+'\n')
            if self.path.startswith('/studio/ollama/'):
                value=json.loads(body) if body else None
                with lock,(root/'ollama-requests.jsonl').open('a') as output:output.write(json.dumps(dict(path=self.path,body=value),ensure_ascii=False)+'\n')
                if self.path=='/studio/ollama/models':return self.reply({'models':[dict(name='fixture-local',modifiedAt='fixture',size=1),dict(name='fixture-other',modifiedAt='fixture',size=1)]})
                if self.path=='/studio/ollama/translate':
                    assert value['model']=='fixture-local' and value['text']
                    return self.reply(dict(translation='本地译文 中文😀',sourcePronunciation='/ləʊkəl/',translationPronunciation='běn dì yì wén',model=value['model'],totalDurationMs=1,loadDurationMs=0))
                if self.path=='/studio/ollama/continue-draft':
                    assert value['model']=='fixture-local' and value['assistantResponse']
                    held=(root/'hold-local-draft').exists()
                    if held:
                        import time
                        (root/'local-draft-held').touch()
                        while not (root/'release-local-draft').exists():time.sleep(.02)
                    self.reply(dict(prompt='Local continuation 中文😀',model=value['model'],totalDurationMs=1,loadDurationMs=0))
                    if held:(root/'local-draft-released').touch()
                    return
                return self.reply({'error':'unexpected fixture route'},404)
            headers={name:value for name,value in self.headers.items() if name.lower() not in ['host','origin']}
            headers['Host']=f'127.0.0.1:{actual.port}'
            if self.headers.get('Origin') is not None:headers['Origin']=origin
            transport = ('codex' if self.path.startswith('/ws/codex/') or self.path == '/ws/codex' else 'opencode' if self.path == '/studio/native/opencode-events' else None) if faults else None
            def trace(kind):
                if transport:
                    import time
                    with lock,(root/'transport-events.jsonl').open('a') as output:
                        output.write(json.dumps(dict(kind=kind,transport=transport,at=time.time_ns()))+'\n')
            if websocket or transport:
                if transport and (root/('reject-'+transport)).exists():
                    trace('rejected')
                    return self.reply({'error':'private startup transport unavailable'},503)
                trace('opened')
                with socket.create_connection(('127.0.0.1',actual.port),timeout=10) as upstream:
                    lag_fixture=transport=='codex' and os.environ.get('STUDIO_GNOME_SMOKE_EVENT_GAP')=='1'
                    if lag_fixture:upstream.setsockopt(socket.SOL_SOCKET,socket.SO_RCVBUF,65536)
                    upstream.sendall((self.command+' '+self.path+' HTTP/1.1\r\n'+''.join(k+': '+v+'\r\n' for k,v in headers.items())+'\r\n').encode()+body)
                    self.close_connection=True
                    held=False;tail=b'';marker=b'studio/appServer/lagged'
                    while child.poll() is None:
                        if transport and (root/('drop-'+transport)).exists():
                            trace('dropped');self.close_connection=True;return
                        holding=lag_fixture and (root/'hold-codex').exists()
                        if holding!=held:
                            held=holding;trace('held' if held else 'resumed')
                            if held:(root/'hold-codex-active').write_text('1')
                            else:(root/'hold-codex-active').unlink(missing_ok=True)
                        readable,_,_=select.select([self.connection] if held else [self.connection,upstream],[],[],.05 if faults else 1)
                        for source in readable:
                            try:data=source.recv(65536)
                            except (ConnectionResetError,BrokenPipeError):return
                            if not data:return
                            if lag_fixture and source is upstream:
                                # Observe only this known marker across TCP chunk
                                # boundaries. Never retain payloads or credentials.
                                combined=tail+data
                                for _ in range(combined.count(marker)):trace('lagged')
                                tail=combined[-(len(marker)-1):]
                            try:(upstream if source is self.connection else self.connection).sendall(data)
                            except (ConnectionResetError,BrokenPipeError):return
                return
            connection=http.client.HTTPConnection('127.0.0.1',actual.port,timeout=180)
            try:
                connection.request(self.command,self.path,body or None,headers);response=connection.getresponse()
                stream=response.getheader('Content-Type','').startswith('text/event-stream')
                self.send_response(response.status)
                if stream:
                    self.send_header('Content-Type',response.getheader('Content-Type'));self.send_header('Connection','close');self.end_headers();self.close_connection=True
                    while child.poll() is None:
                        data=response.read1(65536)
                        if not data:break
                        self.wfile.write(data);self.wfile.flush()
                else:
                    data=response.read()
                    if faults and self.path == '/studio/native/epub' and self.command == 'POST':
                        request=json.loads(body) if body else {}
                        gate=root/'hold-epub-search'
                        try:held_start=int(gate.read_text())
                        except FileNotFoundError:held_start=None
                        if request.get('operation')=='search' and held_start is not None and request.get('start')==held_start:
                            import time
                            marker=root/'epub-search-held.tmp'
                            marker.write_text(json.dumps(dict(request=request,response=json.loads(data)),ensure_ascii=False))
                            marker.replace(root/'epub-search-held.json')
                            while gate.exists() and child.poll() is None:time.sleep(.02)
                            if child.poll() is not None:
                                self.close_connection=True
                                return
                    for name,value in response.getheaders():
                        if name.lower() not in ['content-length','transfer-encoding','connection','server','date']:self.send_header(name,value)
                    self.send_header('Content-Length',str(len(data)));self.end_headers();self.wfile.write(data)
            except (BrokenPipeError,ConnectionResetError,TimeoutError,http.client.IncompleteRead):self.close_connection=True
            finally:connection.close()
        do_GET=do_POST=do_PUT=do_PATCH=do_DELETE=handle_request
    server=http.server.ThreadingHTTPServer(('127.0.0.1',0),Proxy);server.daemon_threads=True
    threading.Thread(target=server.serve_forever,daemon=True).start()
    (root/'translation-page-url').write_text(f'http://127.0.0.1:{server.server_port}/fixture-selection')
    print(f'Fixture gateway ready, then open: http://127.0.0.1:{server.server_port}/#token={token}',flush=True)
    def stop(*_):
        try:child.stdin.close()
        except (BrokenPipeError,ValueError):pass
    def parent():
        # An owned gateway may exit before stdin EOF. Avoid holding Python's
        # buffered stdin lock in a daemon during interpreter finalization.
        while os.read(sys.stdin.fileno(),8192):pass
        stop()
    def fault_stop():
        import time
        while child.poll() is None:
            if (root/'stop-gateway').exists():
                stop();return
            time.sleep(.02)
    if faults:threading.Thread(target=fault_stop,daemon=True).start()
    threading.Thread(target=parent,daemon=True).start();signal.signal(signal.SIGTERM,stop)
    try:code=child.wait()
    finally:server.shutdown();server.server_close()
    return code
