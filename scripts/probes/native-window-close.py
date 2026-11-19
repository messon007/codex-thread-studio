"""Native titlebar-close diagnostic. Run only inside an isolated Bubblewrap fixture.
The fixture must contain .studio-close-fixture; real Codex config must be hidden.
Usage: python3 native-window-close.py FIXTURE_DIRECTORY DEBUG_BINARY
"""
import os,sys,time,json,subprocess,threading,http.server,socketserver
from pathlib import Path
root=Path(sys.argv[1]).resolve();binary=sys.argv[2]
if not (root/'.studio-close-fixture').is_file():
 raise RuntimeError('Dedicated isolated fixture marker required')
wayland=os.environ['WAYLAND_DISPLAY']
if not wayland.startswith('/'):
 wayland=str(Path(os.environ['XDG_RUNTIME_DIR'])/wayland)
class Server(socketserver.ThreadingMixIn,http.server.HTTPServer):daemon_threads=True
class Handler(http.server.BaseHTTPRequestHandler):
 def log_message(self,*a):pass
 def do_GET(self):
  if self.path=='/slow':time.sleep(40)
  self.send_response(200);self.send_header('Content-Type','text/html');self.end_headers()
  try:self.wfile.write(b'<title>Close fixture</title><p>Pending resource test</p><img src="/slow">')
  except (BrokenPipeError,ConnectionResetError):pass
server=Server(('127.0.0.1',0),Handler);threading.Thread(target=server.serve_forever,daemon=True).start()
results=[]
for idx,mode in enumerate(['shell','pending-browser','pending-browser']):
 case=root/str(idx);case.mkdir();runtime=Path('/tmp')/f'studio-close-{idx}';runtime.mkdir(mode=0o700)
 env={**os.environ,'XDG_CONFIG_HOME':str(case/'config'),'XDG_CACHE_HOME':str(case/'cache'),'XDG_DATA_HOME':str(case/'data'),'XDG_RUNTIME_DIR':str(runtime),'WAYLAND_DISPLAY':wayland,'CODEX_THREAD_STUDIO_CODEX_CONNECTION':'embedded','CODEX_THREAD_STUDIO_CODEX_BIN':'/usr/bin/false'}
 with (case/'stderr.log').open('w') as stderr,(case/'stdout.log').open('w') as stdout:
  proc=subprocess.Popen([binary],env=env,stdout=stdout,stderr=stderr)
  try:
   deadline=time.monotonic()+20
   while not (runtime/'codex-thread-studio-dev-capture.sock').exists():
    if proc.poll() is not None:raise RuntimeError('app exited before ready')
    if time.monotonic()>deadline:raise TimeoutError('not ready')
    time.sleep(.1)
   time.sleep(3)
   if mode=='pending-browser':subprocess.run([binary,'--dev-open-browser',f'http://127.0.0.1:{server.server_port}/'],env=env,capture_output=True,timeout=12,check=True);time.sleep(2)
   start=time.monotonic();command=subprocess.run([binary,'--dev-close-window'],env=env,capture_output=True,timeout=12)
   code=proc.wait(timeout=15);elapsed=time.monotonic()-start
   time.sleep(.7)
   log=(case/'stderr.log').read_text()
   results.append({'case':idx,'mode':mode,'exit':code,'close_seconds':round(elapsed,3),'command_exit':command.returncode,'webkit_internal_errors':log.count('WebKit encountered an internal error'),'thread_local_panic':'cannot access a Thread Local Storage value' in log})
   print(json.dumps(results[-1]),flush=True)
  finally:
   if proc.poll() is None:proc.terminate();proc.wait(timeout=10)
(root/'report.json').write_text(json.dumps(results,indent=2)+'\n')
