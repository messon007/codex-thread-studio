#!/usr/bin/env python3
"""Verify native reader API bounds/anchors/edit security using private file copies."""
import json
import os
from pathlib import Path
import re
import select
import shutil
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request


def main():
    if len(sys.argv) not in (2, 3):
        raise SystemExit('Usage: smoke-gnome-documents.py GATEWAY_BINARY [MARKDOWN_FILE]')
    binary = str(Path(sys.argv[1]).resolve())
    root = Path(__file__).resolve().parent.parent
    output = root / 'target' / 'gnome-native-smoke'
    output.mkdir(parents=True, exist_ok=True)
    directory = Path(tempfile.mkdtemp(prefix='documents-', dir=output))
    project = directory / 'project'
    project.mkdir()
    source = project / 'large.md'
    if len(sys.argv) == 3:
        shutil.copyfile(sys.argv[2], source)
    else:
        with source.open('w') as stream:
            stream.write('# Native reader\n\n| Name | Requirement | Context |\n|---|---|---|\n')
            for row in range(20_000):
                stream.write(f'| item {row} | REQ-{row:06} | 中文 context {"details " * 12} |\n')
    original = source.read_bytes()
    environment = dict(os.environ)
    environment.update({
        'XDG_CONFIG_HOME': str(directory / 'config'),
        'XDG_DATA_HOME': str(directory / 'data'),
        'XDG_CACHE_HOME': str(directory / 'cache'),
        'READER_CACHE_DIR': str(directory / 'reader-cache'),
        'CODEX_HOME': str(directory / 'codex'),
        'CODEX_THREAD_STUDIO_CODEX_BIN': '/nonexistent/native-reader-codex',
        'CODEX_THREAD_STUDIO_OPENCODE_BIN': '/nonexistent/native-reader-opencode',
    })
    log = (directory / 'gateway.log').open('w')
    child = subprocess.Popen([binary, '--serve', '--listen', '127.0.0.1:0', '--parent-pipe'],
                             env=environment, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                             stderr=log, text=True, bufsize=1)
    try:
        origin = token = None
        startup = ''
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline:
            readable, _, _ = select.select([child.stdout], [], [], 0.5)
            if not readable:
                continue
            data = os.read(child.stdout.fileno(), 8192)
            startup += data.decode()
            match = re.search(r'(http://127\.0\.0\.1:\d+)/#token=([0-9a-f]{32})', startup)
            if match:
                origin, token = match.groups()
                break
            if not data and child.poll() is not None:
                raise RuntimeError('Gateway exited during startup')
        assert origin and token, 'Gateway did not become ready'

        def request(path, body=None, headers=None, method=None, expected=200):
            credential = {'Authorization': 'Bearer ' + token, **(headers or {})}
            if body is not None:
                credential['Content-Type'] = 'application/json'
            req = urllib.request.Request(origin + path, data=json.dumps(body).encode() if body is not None else None,
                                         headers=credential, method=method or ('POST' if body is not None else 'GET'))
            try:
                response = urllib.request.urlopen(req, timeout=40)
            except urllib.error.HTTPError as error:
                response = error
            assert response.status == expected, f'{path}: HTTP {response.status}, expected {expected}'
            data = response.read(32 * 1024 * 1024 + 1)
            assert len(data) <= 32 * 1024 * 1024
            return json.loads(data) if data else None

        def opened(path):
            value = request('/studio/native/markdown/open', {'root': str(project), 'path': str(path)})
            assert 'largeMarkdown' not in value and 'url' not in value
            return value['readerId']

        def indexed(reader):
            deadline = time.monotonic() + 90
            while time.monotonic() < deadline:
                value = request(f'/studio/native/markdown/{reader}/document')
                if value['state'] == 'ready':
                    assert 'initial' not in value, 'Native index must not include HTML fragments'
                    return value
                assert value['state'] == 'indexing', value.get('error', 'Index failed')
                time.sleep(0.05)
            raise RuntimeError('Document indexing timed out')

        started = time.monotonic()
        assert request('/studio/native/info')['appName'] == 'Codex Thread Studio'
        native_read = {'backend': 'missing-native-test-backend', 'threadId': 'thread', 'operation': 'read'}
        request('/studio/native/conversation/read', native_read, expected=400)
        request('/studio/native/conversation/read', native_read, headers={'Authorization': ''}, expected=401)
        request('/studio/native/conversation/read', native_read, headers={'Origin': 'https://example.com'}, expected=403)
        request('/studio/native/conversation/read', {**native_read, 'operation': 'turn/start'}, expected=400)
        request('/studio/native/conversation/read', {**native_read, 'operation': 'search', 'searchTerm': ''}, expected=400)
        preferences = request('/studio/native/preferences')
        assert preferences['theme'] == 'system'
        shared_preferences = request('/studio/preferences')
        native_saved = request('/studio/native/preferences', {'theme': 'dark', 'selectedBackend': 'codex', 'selectedSession': 'test-session'}, method='PATCH')
        assert native_saved['windowWidth'] == 1280 and native_saved['selectedSession'] == 'test-session'
        resized = request('/studio/native/preferences', {'windowWidth': 1440}, method='PATCH')
        assert resized['theme'] == 'dark' and resized['selectedSession'] == 'test-session'
        request('/studio/native/preferences', {'windowWidth': 20}, method='PATCH', expected=400)
        request('/studio/native/preferences', {'draft': 'must not persist'}, method='PATCH', expected=400)
        request('/studio/native/preferences', {'theme': 'light'}, method='PATCH', headers={'Authorization': ''}, expected=401)
        assert request('/studio/preferences') == shared_preferences
        assert request('/studio/native/preferences')['windowWidth'] == 1440
        assert request('/studio/session-state/pin')['pinnedSessions'] == []
        queue_path='/studio/session-state/message-queue?sessionKey=codex%3Anative-queue-test'
        assert request(queue_path)==[]
        queued={'id':'queue-one','text':'later 中文😀','input':[{'type':'skill','name':'test','path':'/tmp/private/SKILL.md'}],'createdAt':123}
        request('/studio/session-state/message-queue',{'sessionKey':'codex:native-queue-test','messages':[queued]},method='PUT',expected=204)
        assert request(queue_path)==[queued]
        assert request('/studio/session-state/message-queue?sessionKey=opencode%3Anative-queue-test')==[]
        request(queue_path,headers={'Authorization':''},expected=401)
        request(queue_path,headers={'Origin':'https://example.com'},expected=403)
        request('/studio/session-state/message-queue?sessionKey=invalid',expected=400)
        request('/studio/session-state/message-queue',{'sessionKey':'codex:native-queue-test','messages':[]},method='PUT',expected=204)
        options_path = '/studio/session-state/turn-options?sessionKey=codex%3Aprivate-native-test'
        assert request(options_path) == {'model': '', 'effort': '', 'serviceTier': ''}
        request('/studio/session-state/turn-options', {'sessionKey': 'codex:private-native-test', 'model': 'native-model', 'effort': 'high', 'serviceTier': 'fast'}, method='PUT', expected=204)
        assert request(options_path) == {'model': 'native-model', 'effort': 'high', 'serviceTier': 'fast'}
        request(options_path, headers={'Authorization': ''}, expected=401)
        request('/studio/session-state/pin', {'sessionKey': 'codex:private-native-test', 'pinned': True}, method='PUT')
        assert request('/studio/session-state/pin')['pinnedSessions'] == ['codex:private-native-test']
        assert request('/studio/session-state')['pinnedSessions'] == ['codex:private-native-test']
        request('/studio/session-state/pin', headers={'Authorization': ''}, expected=401)
        request('/studio/session-state/pin', {'sessionKey': 'codex:private-native-test', 'pinned': False}, method='PUT')
        assert request('/studio/session-state/pin')['pinnedSessions'] == []
        reader = opened(source)
        document = indexed(reader)
        open_ms = (time.monotonic() - started) * 1000
        pages = document['index']['pages']
        candidates = {0, len(pages) // 2, len(pages) - 1}
        candidates.update(range(min(12, len(pages))))
        continuation = next((i for i, page in enumerate(pages) if page.get('table') is not None and page['rowStart'] > 0), None)
        if continuation is not None:
            candidates.add(continuation)
        maximum = 0
        for page in sorted(candidates):
            result = request(f'/studio/native/markdown/{reader}/fragment?target={page}')
            window = result['source']
            content = window['text'].encode()
            assert content == original[window['start']:window['end']], 'Source byte mapping changed'
            assert window['line'] == original[:window['start']].count(b'\n') + 1, 'Source line mapping changed'
            assert len(content) <= 512 * 1024, 'Unbounded native fragment'
            maximum = max(maximum, len(content))
            if page == continuation:
                header = result['header']
                assert header and header['text'].encode() == original[header['start']:header['end']]
                assert len(header['text'].splitlines()) == 2, 'Continuation table lost its header/separator'
        request(f'/studio/native/markdown/{reader}/fragment?target={len(pages)}', expected=400)
        table_checks = 0
        for table_id, table in enumerate(document['index']['tables'][:3]):
            selected = [0, len(table['columns']) - 1] if len(table['columns']) > 1 else [0]
            columns = ','.join(map(str, selected))
            expected_start = 0
            for _ in range(3):
                value = request(f'/studio/native/markdown/{reader}/table-source?id={table_id}&row={expected_start}&columns={columns}')
                assert value['columns'] == selected and value['rowStart'] == expected_start
                assert value['rowEnd'] <= table['rows'] and (value['rowEnd'] - value['rowStart']) * len(selected) <= 512
                ranges = [(fragment['metadata']['rowStart'], fragment['metadata']['rowStart'] + fragment['metadata']['rows'])
                          for fragment in value['fragments']]
                for row in range(value['rowStart'], value['rowEnd']):
                    assert sum(start <= row < end for start, end in ranges) == 1, 'Table page lost/duplicated source rows'
                for fragment in value['fragments']:
                    window = fragment['source']
                    assert window['text'].encode() == original[window['start']:window['end']]
                table_checks += 1
                if value['rowEnd'] == table['rows']:
                    break
                expected_start = value['rowEnd']
            request(f'/studio/native/markdown/{reader}/table-source?id={table_id}&columns=0,0', expected=400)
            request(f'/studio/native/markdown/{reader}/table-source?id={table_id}&row={table["rows"] + 1}', expected=400)
        request(f'/studio/native/markdown/{reader}/document', headers={'Authorization': ''}, expected=401)
        request(f'/studio/native/markdown/{reader}/document', headers={'Origin': 'https://example.com'}, expected=403)
        request(f'/studio/native/markdown/{reader}/fragment?target=0', body={}, expected=405)
        request(f'/studio/native/markdown/{reader}/heartbeat')

        # Exercise the same atomic-save/conflict boundary through the native API.
        edit_file = project / 'editable.md'
        edit_file.write_bytes('# Native edit\r\n\r\n中文😀 before\r\n'.encode())
        edit_reader = opened(edit_file)
        edit_document = indexed(edit_reader)
        headers = {'x-reader-document': edit_document['documentId']}
        edit_path = f'/studio/native/markdown/{edit_reader}/edit'
        page = request(edit_path, headers=headers)
        window = page['window']
        # Snapshot ranges are UTF-8 file bytes; patch positions are UTF-16 units
        # in the LF-normalized editor buffer, matching the existing editor API.
        prefix = window['text'].split('before', 1)[0]
        offset = len(prefix.encode('utf-16-le')) // 2
        save = {'revision': page['revision'], 'start': window['start'], 'end': window['end'],
                'changes': [{'from': offset, 'to': offset + 6, 'insert': 'after'}]}
        saved = request(edit_path, save, headers=headers)
        assert edit_file.read_bytes() == '# Native edit\r\n\r\n中文😀 after\r\n'.encode()
        assert saved['documentId'] != edit_document['documentId']
        request(edit_path, save, headers=headers, expected=409)
        outside = directory / 'outside.md'
        outside.write_text('Outside project; must stay unchanged.\n')
        request('/studio/native/markdown/open', {'root': str(project), 'path': str(outside)}, expected=403)
        edit_file.rename(project / 'saved-backup.md')
        edit_file.symlink_to(outside)
        request(edit_path, save, headers={'x-reader-document': saved['documentId']}, expected=403)
        assert outside.read_text() == 'Outside project; must stay unchanged.\n'
        assert source.read_bytes() == original, 'Read-only test changed the private large document'
        # Comparison uses immutable indexed snapshots, not re-opened user paths.
        left_path = project / 'compare-left.md'
        right_dir = project / 'other-directory'
        right_dir.mkdir()
        right_path = right_dir / 'compare-right.md'
        left_path.write_bytes('# Comparison\r\n\r\nunchanged\r\nbefore 中文😀\r\n'.encode())
        right_path.write_bytes('# Comparison\r\n\r\nunchanged\r\nafter 中文😀\r\nadded\r\n'.encode())
        def readonly(path):
            value = request('/studio/native/markdown/open-readonly', {'root': str(project), 'path': str(path)})
            assert value['readOnly'] is True and 'url' not in value
            return value['readerId']
        left = readonly(left_path)
        right = readonly(right_path)
        left_index, right_index = indexed(left), indexed(right)
        left_path.write_text('Changed on disk after indexing\n')
        comparison = request('/studio/native/compare', {'left': left, 'right': right})
        assert comparison['changes'] == 1 and comparison['added'] == 2 and comparison['removed'] == 1
        assert comparison['leftHash'] == left_index['index']['revision']
        assert comparison['rightHash'] == right_index['index']['revision']
        change = request(f'/studio/native/compare/{comparison["id"]}/0')
        assert change['leftLine'] == 4 and change['rightLine'] == 4
        assert change['leftChanged'] == [4] and change['rightChanged'] == [4, 5]
        request(f'/studio/native/compare/{comparison["id"]}/1', expected=400)
        request('/studio/native/compare', {'left': left, 'right': 'not-an-open-document'}, expected=409)
        request('/studio/native/compare', {'left': left, 'right': right}, headers={'Authorization': ''}, expected=401)
        request('/studio/native/compare', {'left': left, 'right': right}, headers={'Origin': 'https://example.com'}, expected=403)
        request(f'/studio/native/markdown/{left}/edit', expected=403)
        request('/studio/markdown/close', {'id': left})
        request(f'/studio/native/compare/{comparison["id"]}/0', expected=410)
        request('/studio/markdown/close', {'id': right})
        # Full indexed-table selection is independent of the displayed row page.
        selection_file = project / 'table-selection.md'
        prefix = '# Native selection\r\n\r\nBefore **中文😀**.\r\n\r\n'
        heading = '| **A** | B |\r\n|---|---|\r\n'
        records = ['| [one][dest] | $x^2$ |\r\n'] + [f'| row {n} 😀 | value {n} |\r\n' for n in range(1, 40)]
        grid = heading + ''.join(records)
        tail = '\r\nAfter tail.\r\n\r\n[dest]: ./next.md\r\n'
        selection_file.write_bytes((prefix + grid + tail).encode())
        selected_reader = readonly(selection_file)
        selected_doc = indexed(selected_reader)
        selected_revision = selected_doc['index']['revision']
        selection_url = f'/studio/native/markdown/{selected_reader}/table-text?ids=0&limit=524288&revision={selected_revision}'
        selected = request(selection_url, headers={'x-reader-document': selected_doc['documentId']})
        expected = 'A\tB\none\t$x^2$' + ''.join(f'\nrow {n} 😀\tvalue {n}' for n in range(1, 40))
        assert selected['revision'] == selected_revision and selected['documentId'] == selected_doc['documentId']
        assert selected['tables'][0]['text'] == expected and selected['tables'][0]['id'] == 0
        prefix_url=selection_url.split('/table-text?')[0]+'/table-prefix?id=0&characters={}&revision='+selected_revision
        for count in [0,1,7,31,16000,len(expected),len(expected)+1]:
            prefix_result=request(prefix_url.format(count),headers={'x-reader-document':selected_doc['documentId']})
            assert prefix_result['documentId']==selected_doc['documentId'] and prefix_result['revision']==selected_revision
            assert prefix_result['prefix']==dict(id=0,text=expected[:count],complete=len(expected)<=count)
        for headers,status in [({'Authorization':''},401),({'Origin':'https://example.com'},403),({'x-reader-document':'wrong-document'},409)]:
            request(prefix_url.format(7),headers=headers,expected=status)
        request(prefix_url.format(7).replace(selected_revision,'0'*64),expected=409)
        request(prefix_url.format(131073),expected=400)
        request(prefix_url.format(7).replace('id=0','id=999999'),expected=400)
        request(prefix_url.format(7),body={},expected=405)
        span = selected['tables'][0]['source']
        assert span == {'start': len(prefix.encode()), 'end': len((prefix + grid).encode()), 'line': 5, 'endLine': 46}, span
        assert selected_doc['index']['tables'][0]['endPage'] - selected_doc['index']['tables'][0]['firstPage'] > 1
        request(selection_url, headers={'Authorization': ''}, expected=401)
        request(selection_url, headers={'Origin': 'https://example.com'}, expected=403)
        request(selection_url, headers={'x-reader-document': 'wrong-document'}, expected=409)
        request(selection_url.replace(selected_revision, '0' * 64), expected=409)
        request(selection_url.replace('ids=0', 'ids=0,0'), expected=400)
        request(selection_url.replace('ids=0', 'ids=999999'), expected=400)
        request(selection_url.replace('limit=524288', 'limit=524289'), expected=400)
        request(selection_url.replace('limit=524288', f'limit={len(expected.encode()) - 1}'), expected=400)
        request(selection_url, body={}, expected=405)
        quote = 'Before 中文😀.\n\n' + expected + '\n\nAfter tail.'
        def locate(selection_reader, revision, text, anchor_hash=None):
            return request(f'/studio/native/markdown/{selection_reader}/locate-quote', {
                'revision': revision, 'query': {'quote': text, 'anchor': {
                    'baseHash': anchor_hash or revision, 'textKind': 'flow'}}})['located']
        found = locate(selected_reader, selected_revision, quote)
        assert found['status'] == 'unique' and found['textKind'] == 'flow', found
        assert found['position']['line'] == 3 and found['position']['endLine'] == 48, found
        request('/studio/markdown/close', {'id': selected_reader})
        request(selection_url, expected=410)
        request(prefix_url.format(7),expected=410)
        # Measure production parser work on a table far beyond the copy budget.
        huge_prefix=project/'large-table-prefix.md'
        huge_row='| 汉😀 | bounded prefix evidence '+'context '*18+'|\n'
        huge_header='| Header | Value |\n|---|---|\n'
        total=(32*1024*1024-len(huge_header.encode()))//len(huge_row.encode())+1
        with huge_prefix.open('wb') as file:
            file.write(huge_header.encode())
            chunk=huge_row.encode()*1024
            for _ in range(total//1024):file.write(chunk)
            file.write(huge_row.encode()*(total%1024))
        huge_reader=readonly(huge_prefix);huge_document=indexed(huge_reader)
        before_prefix=time.monotonic()
        limited=request(f'/studio/native/markdown/{huge_reader}/table-prefix?id=0&characters=16000&revision='+huge_document['index']['revision'],headers={'x-reader-document':huge_document['documentId']})
        prefix_ms=round((time.monotonic()-before_prefix)*1000,2)
        expected_large='Header\tValue'+('\n汉😀\t'+('bounded prefix evidence '+'context '*18).rstrip())*200
        assert limited['prefix']==dict(id=0,text=expected_large[:16000],complete=False)
        assert huge_document['index']['sourceBytes']>=32*1024*1024
        assert limited['work']['sourceBytes']<128*1024 and limited['work']['sourcePages']<32,limited['work']
        request('/studio/markdown/close',{'id':huge_reader})

        duplicate_file = project / 'duplicate-table-selection.md'
        pair = 'Before **中文😀**.\r\n\r\n' + grid + '\r\nAfter tail.\r\n'
        duplicate_file.write_bytes(('# Duplicates\r\n\r\n' + pair + '\r\n' + pair + '\r\n[dest]: ./next.md\r\n').encode())
        duplicate_reader = readonly(duplicate_file)
        duplicate_doc = indexed(duplicate_reader)
        duplicate_revision = duplicate_doc['index']['revision']
        batch = request(f'/studio/native/markdown/{duplicate_reader}/table-text?ids=0,1&limit=524288&revision={duplicate_revision}')['tables']
        assert [t['text'] for t in batch] == [expected, expected] and [t['id'] for t in batch] == [0, 1]
        assert batch[0]['source']['end'] < batch[1]['source']['start']
        assert locate(duplicate_reader, duplicate_revision, quote, 'f' * 64)['status'] == 'ambiguous'
        request('/studio/markdown/close', {'id': duplicate_reader})
        wide = project / 'column-cursor.md'
        wide.write_text('|' + '|'.join(f'Column {n}' for n in range(12)) + '|\n|'
                        + '---|' * 12 + '\n' + ''.join(
                            '|' + '|'.join(f'{row}:{column}' for column in range(12)) + '|\n'
                            for row in range(100)))
        wide_reader = readonly(wide)
        indexed(wide_reader)
        for count, size in [(12, 42), (11, 46), (4, 50)]:
            columns = ','.join(map(str, range(count)))
            page = request(f'/studio/native/markdown/{wide_reader}/table-source?id=0&row=42&columns={columns}')
            assert page['rowStart'] == 42 and page['pageSize'] == size
            assert page['rowEnd'] == 42 + size and size * count <= 512
            ranges = [(fragment['metadata']['rowStart'], fragment['metadata']['rowStart'] + fragment['metadata']['rows'])
                      for fragment in page['fragments']]
            for row in range(page['rowStart'], page['rowEnd']):
                assert sum(start <= row < end for start, end in ranges) == 1
        request(f'/studio/native/markdown/{wide_reader}/table-source?id=0&row=100&columns=0', expected=400)
        request('/studio/markdown/close', {'id': wide_reader})
        large_atom = project / 'large-atomic-table.md'
        large_atom.write_text('# Large atom\n\nBefore sentinel.\n\n| H |\n|---|\n| ' + 'x' * 18000 + ' |\n\nAfter sentinel.\n')
        atom_reader = readonly(large_atom)
        atom_doc = indexed(atom_reader)
        atom_revision = atom_doc['index']['revision']
        assert locate(atom_reader, atom_revision, 'After sentinel.')['status'] == 'unique'
        assert locate(atom_reader, atom_revision, 'Before sentinel.\n\nAfter sentinel.')['status'] == 'missing'
        request('/studio/markdown/close', {'id': atom_reader})
        request('/studio/markdown/close', {'id': reader})
        request('/studio/markdown/close', {'id': edit_reader})
        request(f'/studio/native/markdown/{reader}/document', expected=410)
        result = {'ok': True, 'sourceBytes': len(original), 'pages': len(pages),
                  'openMs': round(open_ms, 1), 'indexMs': document['index']['indexMs'],
                  'sourceRangeChecks': len(candidates), 'tableContinuation': continuation is not None,
                  'tablePageChecks': table_checks,
                  'columnChangesKeepRowCursor': True,
                  'maxFragmentBytes': maximum, 'edit': True, 'conflict': True,
                  'authentication': True, 'origin': True, 'confined': True, 'unicodeCrlf': True,
                  'closed': True, 'wholeTableSelection': True, 'boundedTablePrefix': True, 'largePrefixWork': dict(documentBytes=huge_document['index']['sourceBytes'],milliseconds=prefix_ms,**limited['work']), 'wholeTableQuoteRelocation': True, 'atomicTableBoundaries': True, 'comparison': True, 'immutableComparison': True, 'nativePreferences': True, 'artifacts': str(directory)}
        (directory / 'result.json').write_text(json.dumps(result, indent=2))
        print(json.dumps(result, indent=2))
    finally:
        child.stdin.close()
        try:
            child.wait(timeout=8)
        except subprocess.TimeoutExpired:
            child.kill()
            child.wait()
        child.stdout.close()
        log.close()


if __name__ == '__main__':
    main()
