"""Actual child-exit fault for private deterministic Codex providers only."""

SOURCE = r'''
with (root/'codex-process-starts.jsonl').open('a') as output:output.write(json.dumps(dict(pid=os.getpid(),at=time.time_ns()))+'\n')
def watch_exit():
    while True:
        if (root/'exit-codex-process').exists():
            (root/'exit-codex-process').unlink()
            with (root/'codex-process-exits.jsonl').open('a') as output:output.write(json.dumps(dict(pid=os.getpid(),code=23,at=time.time_ns()))+'\n')
            os._exit(23)
        time.sleep(.02)
threading.Thread(target=watch_exit,daemon=True).start()
'''

def instrument(source):
    assert source.count("for raw in sys.stdin:") == 1
    return source.replace("for raw in sys.stdin:", SOURCE + "\nfor raw in sys.stdin:", 1)
