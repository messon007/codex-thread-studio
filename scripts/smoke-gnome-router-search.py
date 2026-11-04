#!/usr/bin/env python3
"""Persisted Router search API, with private history, no model turns and real SQLite."""
import argparse
import concurrent.futures
import importlib.util
import json
from pathlib import Path
import sqlite3
import sys
import tempfile
import time
from urllib.parse import urlencode
import uuid

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location('router_runtime', Path(__file__).with_name('smoke-gnome-router-runtime.py'))
runtime = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runtime)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('gateway')
    args = parser.parse_args()
    artifacts = Path(__file__).resolve().parents[1] / 'target/gnome-native-smoke'
    artifacts.mkdir(parents=True,exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix='router-search-api-', dir=artifacts))
    runtime.create_profile(root)
    runtime.fixture.BACKEND = runtime.CODEX
    runtime.fixture.OPENCODE_BACKEND = runtime.OPENCODE
    gateway = runtime.fixture.Gateway(Path(args.gateway).resolve(), root, 'codex')
    endpoint = '/studio/native/router/jobs'
    measurements = []
    try:
        def search(query, **options):
            route = endpoint + '?' + urlencode(dict(controller='codex:router', query=query, **options))
            at = time.monotonic()
            status, value = gateway.api(route)
            measurements.append({'queryBytes': len(query.encode()), 'milliseconds': round((time.monotonic()-at)*1000, 2),
                                 'responseBytes': len(json.dumps(value).encode()), 'status': status})
            return status, value

        assert gateway.api(endpoint+'?controller=codex:router&query=text', authorized=False)[0] == 401
        assert gateway.api(endpoint+'?controller=codex:router')[0] == 200
        def memory():
            values = {}
            for line in Path('/proc', str(gateway.process.pid), 'status').read_text().splitlines():
                if line.startswith(('VmRSS:', 'VmHWM:')):
                    name, amount, unit = line.split()
                    values[name.rstrip(':')+'KiB'] = int(amount)
            return values
        memory_before = memory()
        database = root / 'config/codex-thread-studio/studio.sqlite3'
        identities = []
        with sqlite3.connect(database) as db:
            for index in range(5000):
                identity = str(uuid.uuid4())
                identities.append(identity)
                prompt = f'Persisted request {index} ' + 'context 中文 ' * 30
                if index == 0:
                    prompt += 'ÉTÉ []%_.* rare-old-key中文 '
                job = dict(id=identity, controller='codex:router', createdAt=1, prompt=prompt, status='clarify',
                           controllerTurnId='', target=None, targetTurnId='', decision=None,
                           response=f'Persisted reply {index}', responseItems=[], error='', unread=index in [0, 4900])
                if index == 0:
                    job.update(status='completed', targetTurnId='stored-turn',
                               target=dict(sessionKey='codex:target', backend='codex', id='target', title='Stored target',
                                           cwd=str(root/'project'), responsibility='', fallbackCondition='', openingMessage=''),
                               contextExcerpt='x'*3900+'deep-tail-only😀')
                    job['responseItems'] = [dict(id='source-response', text='x'*24000+'stored-source-only😀', excerpt=True)]
                db.execute('INSERT INTO native_router_jobs(id,controller,created,owner,phase,json,receipt,unread) VALUES(?,?,?,?,?,?,?,?)',
                           (identity, 'codex:router', 1, 'private-search-fixture', job['status'], json.dumps(job), uuid.uuid4().hex, job['unread']))
            other = {**job, 'id': str(uuid.uuid4()), 'controller': 'opencode:oc-router', 'prompt': 'rare-old-key中文 '}
            db.execute('INSERT INTO native_router_jobs(id,controller,created,owner,phase,json,receipt,unread) VALUES(?,?,?,?,?,?,?,?)',
                       (other['id'], other['controller'], 1, 'private-search-fixture', 'clarify', json.dumps(other), uuid.uuid4().hex, 0))
            before = db.execute('SELECT id,json,receipt,unread FROM native_router_jobs ORDER BY rowid').fetchall()
        status, rare = search('rare-old-key中文 ')
        assert status == 200 and [row['id'] for row in rare['data']] == [identities[0]], rare
        assert rare['unreadCount'] == 2 and not rare['activeIds']
        assert search('rare-old-key中文  ')[1]['data'] == []
        assert search('été')[1]['data'][0]['id'] == identities[0]
        assert search('été', caseSensitive='true')[1]['data'] == []
        assert search('[]%_.*')[1]['data'][0]['id'] == identities[0]
        assert search("%' OR 1=1 --")[1]['data'] == []
        assert search('deep-tail-only😀')[1]['data'][0]['id'] == identities[0]
        assert search('stored-source-only😀')[1]['data'][0]['id'] == identities[0]
        status, first = search('Persisted request')
        assert status == 200 and len(first['data']) == 20 and first['nextCursor'] and first['newerCursor'] is None
        second = search('Persisted request', before=first['nextCursor'])[1]
        assert len(second['data']) == 20 and not ({r['id'] for r in first['data']} & {r['id'] for r in second['data']})
        assert search('Persisted request', after=second['newerCursor'])[1]['data'] == first['data']
        assert all('prompt' not in row and 'responseItems' not in row for row in first['data'])
        for query in ['x'*513, 'nul\0text']:
            assert search(query)[0] == 400
        assert search('text', id=identities[0])[0] == 400
        assert search('text', before=1, after=2)[0] == 400
        # Issue a history scan and a separate WAL write concurrently.
        with concurrent.futures.ThreadPoolExecutor(max_workers=1) as pool:
            pending = pool.submit(search, 'missing-deep-history-needle')
            write_at = time.monotonic()
            with sqlite3.connect(database, timeout=1) as db:
                db.execute('UPDATE native_router_jobs SET created=2 WHERE id=?', (identities[-1],))
            writer_ms = round((time.monotonic()-write_at)*1000, 2)
            assert pending.result()[0] == 200
        with sqlite3.connect(database) as db:
            after = db.execute('SELECT id,json,receipt,unread FROM native_router_jobs ORDER BY rowid').fetchall()
        assert before == after, 'Search changed a job or read receipt'
        assert not runtime.starts(root), 'Searching invoked a model'
        assert max(m['responseBytes'] for m in measurements) < 32*1024
        print(json.dumps({'ok': True, 'artifacts': str(root), 'jobs': 5001, 'samples': measurements,
                          'writerCommitMs': writer_ms, 'controllerIsolation': True, 'noMutations': True,
                          'literalUnicode': True, 'storedReplyPartsAndTail': True, 'paging': True, 'noModelTurns': True,
                          'gatewayMemoryBefore': memory_before, 'gatewayMemoryAfter': memory()}))
    finally:
        gateway.close()


if __name__ == '__main__':
    main()
