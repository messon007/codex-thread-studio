"""Silent SSIP fixture on an owned Unix socket; never uses a real audio service."""
import json
import socket
import threading
import time


class PrivateSpeech:
    def __init__(self, root, path):
        self.root, self.path = root, path
        self.socket = socket.socket(socket.AF_UNIX)
        self.socket.bind(str(path)); self.socket.listen(); self.socket.settimeout(.1)
        self.closing = threading.Event(); self.lock = threading.Lock()
        self.clients = []; self.workers = []; self.records = []; self.counter = 0
        self.foreign_live = True
        self.listener = threading.Thread(target=self.accept, daemon=True); self.listener.start()
        # A second live connection represents another application. Global cancel
        # is a hard failure; cancelling Studio's connection must leave this live.
        self.foreign = socket.socket(socket.AF_UNIX); self.foreign.connect(str(path))

    def record(self, **value):
        with self.lock:
            self.records.append(value)
            with (self.root/'speech-requests.jsonl').open('a') as output:
                output.write(json.dumps(value, ensure_ascii=False)+'\n')

    def accept(self):
        while not self.closing.is_set():
            try: client, _ = self.socket.accept()
            except socket.timeout: continue
            except OSError: return
            with self.lock:
                self.clients.append(client); identity = len(self.clients)
            worker = threading.Thread(target=self.serve, args=(client, identity), daemon=True)
            self.workers.append(worker); worker.start()

    def serve(self, client, identity):
        data = None; mode = ''; serial = 0; language = ''
        def reply(text):
            # Fragment every response to exercise a real blocking client parser.
            raw = text.encode()
            for position in range(0, len(raw), 7):
                client.sendall(raw[position:position+7]); time.sleep(.001)
        try:
            with client.makefile('rb') as reader:
                while not self.closing.is_set():
                    raw = reader.readline(96*1024)
                    if not raw: break
                    line = raw.decode().rstrip('\r\n')
                    if data is not None:
                        if line != '.':
                            data.append(line[1:] if line.startswith('..') else line); continue
                        with self.lock: self.counter += 1; serial = self.counter
                        text = '\n'.join(data); data = None
                        mode = (self.root/'speech-mode').read_text().strip()
                        self.record(event='data', client=identity, serial=serial, language=language, text=text)
                        if mode == 'early-end': reply(f'702-1\r\n702-{identity}\r\n702 END\r\n')
                        # Foreign-client END must not finish a held utterance.
                        if mode.startswith('hold'): reply('702-1\r\n702-99999\r\n702 END\r\n')
                        reply('225-1\r\n225 OK MESSAGE QUEUED\r\n')
                        if mode.startswith('hold'): (self.root/f'speech-held-{serial}').write_text('held')
                        elif mode != 'early-end': reply(f'702-1\r\n702-{identity}\r\n702 END\r\n')
                        continue
                    self.record(event='command', client=identity, command=line)
                    command = line.lower()
                    if command.startswith('set self client_name '): reply('208 OK CLIENT NAME SET\r\n')
                    elif command == 'history get client_id': reply(f'245-{identity}\r\n245 OK CLIENT ID\r\n')
                    elif command == 'set self ssml_mode off': reply('219 OK SSML MODE SET\r\n')
                    elif command.startswith('set self language '): language=line.split()[-1]; reply('201 OK LANGUAGE SET\r\n')
                    elif command == 'set self priority text': reply('202 OK PRIORITY SET\r\n')
                    elif command in ['set self notification end on','set self notification cancel on']: reply('220 OK NOTIFICATION SET\r\n')
                    elif command == 'speak': data=[]; reply('230 OK RECEIVING DATA\r\n')
                    elif command == 'cancel self':
                        self.record(event='cancel', client=identity, serial=serial)
                        if mode != 'hold-unconfirmed': reply('213 OK CANCELED\r\n')
                    else:
                        if command.startswith(('cancel ', 'stop ')): self.foreign_live=False
                        self.record(event='unexpected', client=identity, command=line)
                        reply('500 ERR UNEXPECTED FIXTURE COMMAND\r\n')
        except (BrokenPipeError, ConnectionResetError, OSError): pass
        finally: client.close(); self.record(event='closed', client=identity)

    def verify(self, source):
        records = list(self.records); data=[r for r in records if r['event']=='data']
        assert len(data)==5, data
        assert [r['text'] for r in data] == [source, 'yì wén', source, source, '译文 **Markdown** <b>literal</b> 中文😀'], data
        assert [r['language'] for r in data]==['en-US','cmn-LATN-PINYIN','en-US','en-US','cmn']
        assert self.foreign_live and not [r for r in records if r['event']=='unexpected'], records
        cancels=[r for r in records if r['event']=='cancel']
        assert len(cancels)==3 and all(r['client']!=1 for r in cancels), cancels
        closed={r['client'] for r in records if r['event']=='closed'}
        assert {r['client'] for r in data}.issubset(closed), records
        return dict(silentSpeechServer=True, scopedStop=True, exactSpeechData=True, speechEarlyEnd=True, speechConnections=len(data))

    def close(self):
        self.closing.set(); self.foreign.close(); self.socket.close()
        for client in list(self.clients):
            try: client.shutdown(socket.SHUT_RDWR)
            except OSError: pass
            client.close()
        self.listener.join(timeout=2)
        for worker in self.workers: worker.join(timeout=2)
        self.path.unlink(missing_ok=True)

    def __enter__(self): return self
    def __exit__(self, *_): self.close()
