#!/usr/bin/env python3
"""Single-attempt OpenCode Router commands with private, deterministic providers."""
import base64
import importlib.util
import json
from pathlib import Path
import sqlite3
import sys
import tempfile
import time

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location('router_runtime', Path(__file__).with_name('smoke-gnome-router-runtime.py'))
runtime = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runtime)
fixture = runtime.fixture


def oc_requests(root, suffix=None):
    path = root/'opencode-requests.jsonl'
    rows = [json.loads(line) for line in path.read_text().splitlines()] if path.exists() else []
    return [row for row in rows if suffix is None or row['path'].endswith(suffix)]


def main():
    executable = Path(sys.argv[1]).resolve()
    fixture.BACKEND, fixture.OPENCODE_BACKEND = runtime.CODEX, runtime.OPENCODE
    artifacts = Path(__file__).resolve().parents[1]/'target/gnome-native-smoke'
    artifacts.mkdir(parents=True, exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix='router-command-', dir=artifacts))
    runtime.create_profile(root)
    gateway = fixture.Gateway(executable, root, 'opencode')
    opened = time.monotonic()
    skill = {'type':'skill','name':'oc-build','path':'oc-build'}
    target = {'sessionKey':'opencode:oc-target','cwd':str(root/'project-oc')}

    def request(prompt='  Run skill 中文😀\n', references=None):
        return {**runtime.body(gateway, prompt, target), 'references':references if references is not None else [skill]}

    def send(value, expected='completed'):
        return runtime.finished(gateway, runtime.submit(gateway, value), expected)

    def commands():
        return [row for row in oc_requests(root, '/command') if row['method']=='POST']

    def close_owned():
        pids = [int(path.read_text()) for name in ['backend.pid','opencode.pid'] if (path:=root/name).exists()]
        gateway.close()
        runtime.until(lambda:all(not Path(f'/proc/{pid}').exists() for pid in pids))

    try:
        assert gateway.api('/studio/native/router/stop', {'id':'missing'}, authorized=False)[0] == 401
        reference = {'type':'file','root':str(root/'project-oc'),'path':'oc-reference.md'}
        image = 'data:image/gif;base64,'+base64.b64encode(b'GIF89atest').decode()
        (root/'large-oc-inventory').touch()
        value = {**request(references=[skill, reference]), 'images':[image]}
        result = send(value)
        (root/'large-oc-inventory').unlink()
        assert result['response'] == 'Native OpenCode skill response 中文😀', result
        assert result['target']['title'] == 'Native oc-target', result
        assert result['targetTurnId'].startswith('oc-target-command-user-')
        assert result['responseItems'][0]['id'] == result['targetTurnId']+'-answer'
        command = commands()[-1]
        assert command['body']['arguments'] == value['prompt']
        assert command['body']['model'] == 'provider/oc-target-model'
        assert command['body']['parts'] == [
            {'type':'file','mime':'image/gif','url':image},
            {'type':'file','mime':'text/plain','filename':str(root/'project-oc/oc-reference.md'),'url':(root/'project-oc/oc-reference.md').as_uri()}]
        assert command['query'] == {'directory':[str(root/'project-oc')]}
        assert len(commands()) == 1 and gateway.api('/studio/native/router/jobs', value)[0] == 200
        assert len(commands()) == 1, 'Command replayed on the same request ID'
        assert not any(row['path'].endswith('/prompt_async') for row in oc_requests(root))
        assert not runtime.starts(root), 'Explicit target unexpectedly used the controller model'
        for flag, refs in [('disable-oc-skill',[skill]), ('collision-oc-skill',[skill]),
                           ('', [skill,skill]), ('', [skill,{'type':'mention','name':'plugin','path':'plugin://id'}]),
                           ('', [{'type':'skill','name':'oc-build','path':'/codex/SKILL.md'}])]:
            count = len(commands())
            if flag:(root/flag).touch()
            failed = send(request(references=refs), 'failed')
            if flag:(root/flag).unlink()
            assert not failed['targetTurnId'] and len(commands()) == count, failed
        # Genuine selected-controller planning uses descriptions, never the typed skill.
        planned = runtime.body(gateway, 'Choose oc-target skill')
        planned['references'] = [skill]
        result = send(planned)
        assert commands()[-1]['body']['arguments'] == 'Forwarded 中文: Choose oc-target skill'
        assert all(p['type']=='text' for p in runtime.starts(root,'router')[-1]['params']['input'])
        # The synchronous command endpoint cannot acknowledge a user identity early.
        # Stop must explicitly reject rather than aborting the latest unrelated message.
        (root/'hold-oc-command').touch()
        (root/'oc-command-accepted').unlink(missing_ok=True)
        held = runtime.submit(gateway, request('Held command'))
        runtime.until(lambda:(root/'oc-command-accepted').exists())
        held_view = runtime.get(gateway, held)
        assert held_view['status'] == 'dispatching' and 'will not guess' in held_view['dispatchNotice']
        status, rejected = gateway.api('/studio/native/router/stop', {'id':held})
        assert status == 409 and 'target conversation' in json.dumps(rejected), rejected
        assert not oc_requests(root, '/abort')
        assert gateway.api('/studio/native/router/jobs?id='+held+'&controller=codex:router&statusOnly=true')[0] == 200
        (root/'release-oc-command').touch()
        runtime.finished(gateway, held)
        (root/'hold-oc-command').unlink();(root/'release-oc-command').unlink()
        # Do not infer idle from a command's completed assistant. Preserve upstream status.
        # Start while idle, then hold server status after the actual command accepts.
        (root/'hold-oc-command').touch();(root/'oc-command-accepted').unlink()
        active = runtime.submit(gateway, request('Command with busy server status'))
        runtime.until(lambda:(root/'oc-command-accepted').exists())
        (root/'oc-active').touch();(root/'release-oc-command').touch()
        runtime.until(lambda:runtime.get(gateway,active)['status']=='running')
        time.sleep(.85)
        assert runtime.get(gateway,active)['status'] == 'running'
        assert gateway.api('/studio/native/router/stop', {'id':active})[0] == 202
        runtime.finished(gateway,active,'stopped')
        for flag in ['oc-active','release-oc-target','hold-oc-command','release-oc-command']:(root/flag).unlink(missing_ok=True)
        for flag in ['wrong-oc-command-session','old-oc-command-identity','missing-oc-command-identity','fail-oc-command','lose-oc-command-ack']:
            (root/flag).touch();count=len(commands())
            failed = send(request('Do not replay '+flag), 'failed')
            (root/flag).unlink()
            assert len(commands()) == count+1, 'Uncertain command was automatically retried'
            if flag != 'fail-oc-command':assert not failed['targetTurnId'], failed
        count = len(commands())
        # Interrupt a command after server acceptance, before its response arrives.
        (root/'hold-oc-command').touch();(root/'oc-command-accepted').unlink()
        uncertain = runtime.submit(gateway, request('Interrupted command'))
        runtime.until(lambda:(root/'oc-command-accepted').exists())
        close_owned()
        (root/'hold-oc-command').unlink()
        gateway = fixture.Gateway(executable, root, 'opencode')
        runtime.finished(gateway,uncertain,'failed')
        assert len(commands()) == count+1
        # Reopening completed and failed records must not resend commands either.
        assert runtime.get(gateway,result['id'])['status'] == 'completed'
        with sqlite3.connect(root/'config/codex-thread-studio/studio.sqlite3') as db:
            saved = '\n'.join(row[0] for row in db.execute('SELECT json FROM native_router_jobs'))
        assert 'PRIVATE SKILL TEMPLATE NEVER PERSISTED' not in saved and 'data:image' not in saved
        assert not any('messageID' in row['body'] for row in commands())
        report = {'ok':True,'artifacts':str(root),'commands':len(commands()),
                  'elapsedSeconds':round(time.monotonic()-opened,2),'targetInventory':True,'attachments':True,
                  'serverIdentity':True,'uncertainNoReplay':True,'pendingStopGuard':True,'busyStatusPreserved':True,'ownedBackendCleanup':True}
        (root/'command-result.json').write_text(json.dumps(report,indent=2))
    finally:
        close_owned()
    print(json.dumps(report))


if __name__ == '__main__':
    main()
