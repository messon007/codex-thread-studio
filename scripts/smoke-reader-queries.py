#!/usr/bin/env python3
"""Run after cargo build -p marknoria --example reader-service."""
import json, pathlib, subprocess, tempfile, time, urllib.request, urllib.error
ROOT = pathlib.Path(__file__).resolve().parents[1]
BINARY = ROOT / 'target/debug/examples/reader-service'

def check(path):
    process = subprocess.Popen([str(BINARY), str(path)], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    try:
        address = json.loads(process.stdout.readline())
        base = 'http://' + address['address']
        headers = {'x-reader-token': address['token'], 'Content-Type': 'application/json'}
        def call(route, data=None, status=200):
            request = urllib.request.Request(base + route, headers=headers, data=None if data is None else json.dumps(data).encode())
            try:
                with urllib.request.urlopen(request, timeout=12) as response:
                    assert response.status == status
                    return json.load(response)
            except urllib.error.HTTPError as error:
                assert error.code == status, (route, error.code, error.read())
        for _ in range(200):
            document = call('/api/document')
            if document['state'] == 'ready': break
            assert document['state'] != 'error', document
            time.sleep(.05)
        else: raise AssertionError('index timeout')
        headers['x-reader-document'] = document['documentId']
        revision = document['index']['revision']
        if path.suffix == '.md':
            schema = call('/api/table-query/schema?table=0')
            assert schema['sourceRows'] == 120
            result = call('/api/table-query', {'table': 0, 'revision': revision, 'sql': 'SELECT count(*) AS n, sum(CAST("Value" AS INTEGER)) AS total FROM data'})
            assert result['rows'] == [['120', '7140']], result
            result = call('/api/table-query', {'table': 0, 'revision': revision, 'sql': 'SELECT _line, "Name" FROM data ORDER BY _row'})
            assert result['rows'][0] == ['3', 'row-0'], result
            assert result['totalRows'] == 120
            page = call('/api/table-query?id=' + result['id'] + '&row=' + str(result['rowEnd']))
            assert page['rowStart'] > 0
            for sql in ["ATTACH DATABASE '/tmp/unsafe.db' AS unsafe", 'DELETE FROM data', 'SELECT load_extension(\'x\')', 'PRAGMA database_list']:
                call('/api/table-query', {'table': 0, 'revision': revision, 'sql': sql}, 400)
            call('/api/table-query', {'table': 0, 'revision': 'old', 'sql': 'SELECT 1'}, 409)
        else:
            def jq(filter, **extra):
                return call('/api/jq-query', {'revision': revision, 'filter': filter, **extra})
            result = jq('select(.active) | .name')
            assert result['totalValues'] == 1 and '中文' in result['text'], result
            result = jq('map(.value) | add', slurp=True)
            assert result['text'].strip() == '5', result
            call('/api/jq-query', {'revision': revision, 'id': result['id'], 'close': True})
            call('/api/jq-query', {'revision': revision, 'filter': 'env'}, 400)
            call('/api/jq-query', {'revision': revision, 'filter': 'def loop: loop; loop'}, 400)
        print(path.suffix, 'queries, revision checks and confinement passed')
    finally:
        process.terminate()
        try: process.wait(timeout=5)
        except subprocess.TimeoutExpired: process.kill(); process.wait()

with tempfile.TemporaryDirectory(prefix='studio-query-smoke-') as directory:
    directory = pathlib.Path(directory)
    markdown = directory / 'table.md'
    markdown.write_text('| Name | Value |\n|---|---|\n' + ''.join(f'| row-{n} | {n} |\n' for n in range(120)))
    check(markdown)
    data = directory / 'data.jsonl'
    data.write_text('{"name":"中文","active":true,"value":2}\n{"name":"other","active":false,"value":3}\n')
    check(data)
