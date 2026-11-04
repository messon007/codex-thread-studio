"""First-launch faults on the private fixture's owned gateway executable only."""
import json,os,sys,time
from pathlib import Path

def fault():
    root=Path(os.environ['STUDIO_GNOME_SMOKE_DIR'])
    path=root/'launch-fault'
    kind=path.read_text() if path.exists() else 'ready'
    with (root/'launch-attempts.jsonl').open('a') as out:
        out.write(json.dumps(dict(pid=os.getpid(),at=time.time_ns(),kind=kind))+'\n')
    if kind=='ready':return
    (root/'first-launch.pid').write_text(str(os.getpid()))
    if kind=='exit':
        print('Private gateway fixture exited before its endpoint.',file=sys.stderr)
        raise SystemExit(23)
    assert kind=='hold'
    sys.stdin.buffer.read()
    (root/'launch-eof').write_text(str(time.time_ns()))
    raise SystemExit(0)
