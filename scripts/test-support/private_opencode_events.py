"""Injected into owned fake OpenCode executables; never contacts a real service."""

SOURCE = r'''
sdk_events=[]
def emit_sdk(value):
    with lock:sdk_events.append(value)
def stream_sdk(handler):
    handler.send_response(200);handler.send_header('Content-Type','text/event-stream');handler.end_headers()
    def send(data):
        # Split UTF-8, CRLF and field boundaries before the larger body chunks.
        for offset in range(0,min(len(data),420),7):handler.wfile.write(data[offset:min(offset+7,len(data),420)]);handler.wfile.flush()
        for offset in range(420,len(data),16384):handler.wfile.write(data[offset:offset+16384]);handler.wfile.flush()
    cursor=0;stamp=None;extra=[]
    try:
        for byte in [b'\xef',b'\xbb',b'\xbf']:handler.wfile.write(byte);handler.wfile.flush();time.sleep(.002)
        while True:
            raw=root/'raw-events.bin'
            if raw.exists():send(raw.read_bytes());return
            path=root/'sdk-events.json'
            if path.exists() and path.stat().st_mtime_ns!=stamp:
                try:extra=json.loads(path.read_text());stamp=path.stat().st_mtime_ns
                except json.JSONDecodeError:time.sleep(.01);continue
            with lock:rows=[*sdk_events,*extra]
            for index,value in enumerate(rows[cursor:],cursor):
                payload=json.dumps(value,ensure_ascii=False,indent=2)
                frame='event: message\r\nid: fixture-'+str(index)+'\r\nretry: 1000\r\n'+''.join('data: '+line+'\r\n' for line in payload.splitlines())+'\r\n'
                send(frame.encode())
            cursor=len(rows)
            handler.wfile.write(b': heartbeat\n\n');handler.wfile.flush();time.sleep(.04)
    except (BrokenPipeError,ConnectionResetError):pass
'''
