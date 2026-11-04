#!/usr/bin/env python3
"""Exercise the native Mermaid process boundary; no GTK or live backend required."""
import argparse
import hashlib
import json
import os
import socket
from pathlib import Path
import subprocess
import time
from xml.etree import ElementTree as ET

FONTS = ['/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf',
         '/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf',
         '/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf',
         '/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc']


def run(command, request, timeout=10):
    return subprocess.run(command, input=json.dumps(request), text=True,
                          capture_output=True, timeout=timeout)


def children(pid):
    found = []
    try:
        direct = Path(f'/proc/{pid}/task/{pid}/children').read_text().split()
    except (FileNotFoundError, ProcessLookupError):
        return found
    for child in direct:
        child = int(child)
        try:
            stat = Path(f'/proc/{child}/stat').read_text().rsplit(')', 1)[1].split()
            found.append((child, stat[19]))  # PID + start time protects against reuse.
            found.extend(children(child))
        except (FileNotFoundError, ProcessLookupError):
            pass
    return found


def alive(processes):
    live = []
    for pid, started in processes:
        try:
            stat = Path(f'/proc/{pid}/stat').read_text().rsplit(')', 1)[1].split()
            if stat[19] == started:
                live.append((pid, stat[0]))
        except (FileNotFoundError, ProcessLookupError):
            pass
    return live


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('worker', type=Path)
    parser.add_argument('parent', type=Path)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--protocol-version',type=int,default=3)
    parser.add_argument('--auto-fonts',action='store_true')
    parser.add_argument('--corpus', type=Path, help='Replay the private paired corpus directory')
    args = parser.parse_args()
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    worker = args.worker.resolve(strict=True)
    parent = args.parent.resolve(strict=True)
    font_args = [v for p in FONTS for v in ['--font', p]]
    command = [str(parent), str(worker)] + (['--auto-fonts'] if args.auto_fonts else font_args)
    request = dict(version=args.protocol_version, source='flowchart LR\nA[Native 中文] --> B[Reader]', dark=False,
                   style=dict(style='classic', layout='dagre'))
    checks = []
    rows = []
    if args.corpus:
        prior = json.loads((args.corpus / 'results.json').read_text())
        for case in prior['cases']:
            body = (args.corpus / case['id'] / 'source.mmd').read_bytes()
            rows.append((case['id'], body.decode(), case['dark'], case['id'] != 'malformed', hashlib.sha256(body).hexdigest()))
    else:
        rows = [('mixed-dark', 'sequenceDiagram\nparticipant A as Input $$x^2$$\nA->>B: $$\\frac{a}{b}$$', True, True, None),
                ('flow-cjk', request['source'], False, True, None),
                ('elk', '%%{init: {"layout":"elk"}}%%\nflowchart LR\nA-->B & C\nB-->D\nC-->D', False, True, None),
                ('malformed', 'flowchart LR\nA[unclosed', False, False, None)]
    comparisons = []
    for name, source, dark, expected, original in rows:
        req = dict(request, source=source, dark=dark)
        start = time.monotonic()
        result = run(command, req)
        (out / f'{name}.stderr').write_text(result.stderr)
        assert (result.returncode == 0) == expected, (name, result.stderr)
        if expected:
            data = result.stdout.encode()
            assert len(data) <= 4 * 1024 * 1024
            svg = ET.fromstring(data)
            assert not any(x.tag.rsplit('}', 1)[-1] in ['text', 'foreignObject', 'image'] for x in svg.iter())
            (out / f'{name}.svg').write_bytes(data)
            if args.corpus:
                old = (args.corpus / name / 'merman.svg').read_bytes()
                comparisons.append(dict(case=name, equal=old == data))
        if original:
            assert hashlib.sha256((args.corpus / name / 'source.mmd').read_bytes()).hexdigest() == original
        checks.append(dict(case=name, expected=expected, processMs=round((time.monotonic() - start) * 1000, 2)))
    # Strict admission is exercised on the actual worker executable.
    direct = [str(worker)] + font_args
    for name, req in [('version', dict(request, version=args.protocol_version+1)),
                      ('unknown-field', dict(request, svgPath=str(out / 'must-not-exist'))),
                      ('oversized-source', dict(request, source='x' * (128 * 1024 + 1)))]:
        result = run(direct, req)
        assert result.returncode and not result.stdout, name
        (out / f'admission-{name}.log').write_text(result.stderr)
    assert not (out / 'must-not-exist').exists()
    # Observe the real worker after its own admission limits, while stdin is open.
    proc = subprocess.Popen(direct, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    try:
        time.sleep(0.1)
        limits = Path(f'/proc/{proc.pid}/limits').read_text()
        status = Path(f'/proc/{proc.pid}/status').read_text()
        assert 'NoNewPrivs:\t1' in status
        for label, value in [('Max cpu time', '4'), ('Max file size', '0'),
                             ('Max address space', '1073741824'), ('Max data size', '536870912'),
                             ('Max open files', '64'), ('Max processes', '0')]:
            line = next(x for x in limits.splitlines() if x.startswith(label))
            assert line[len(label):].split()[0] == value, line
        (out / 'worker-limits.txt').write_text(limits)
        proc.communicate('', timeout=3) if proc.text_mode else proc.communicate(b'', timeout=3)
    finally:
        if proc.poll() is None:
            proc.kill()
            proc.wait()
    # A disposable stand-in proves the namespace itself, rather than depending
    # on the renderer to reject an attempted access before it reaches the OS.
    secret = out / 'private-marker.txt'
    secret.write_text('this host file must remain outside the namespace')
    listener = socket.socket()
    listener.bind(('127.0.0.1', 0))
    listener.listen()
    port = listener.getsockname()[1]
    code = r'''
use std::{io::{Read,Write},time::Duration,process::{Command,Stdio}};
fn main(){
 if std::env::args().any(|a|a=="--descendant"){loop{std::thread::sleep(Duration::from_secs(1));}}
 let mut input=String::new();std::io::stdin().read_to_string(&mut input).unwrap();
 if input.contains("sandbox-probe"){
  let denied=std::fs::read(SECRET).is_err();
  let env=std::env::var_os("STUDIO_MERMAID_PRIVATE_ENV").is_none();
  let home=std::fs::read_dir("/home").is_err();
  let network=std::net::TcpStream::connect_timeout(&"127.0.0.1:PORT".parse().unwrap(),Duration::from_millis(100)).is_err();
  let output=format!(r#"<svgSVG_METADATA xmlns='http://www.w3.org/2000/svg'><!--file:{denied},env:{env},home:{home},network:{network}--></svg>"#);
  print!("{output}");return;
 }
 if input.contains("flood-probe"){loop{if std::io::stdout().write_all(&[b'x';65536]).is_err(){return;}}}
 let _child=Command::new("/proc/self/exe").arg("--descendant").stdin(Stdio::null()).spawn().unwrap();
 loop{std::thread::sleep(Duration::from_secs(1));}
}
'''.replace('SECRET', json.dumps(str(secret), ensure_ascii=False)).replace('PORT', str(port)).replace('SVG_METADATA', ' data-studio-math-labels="0"' if args.protocol_version >= 3 else '')
    source = out / 'sandbox-probe.rs'
    source.write_text(code)
    fake = out / 'sandbox-probe'
    subprocess.run(['rustc', '--edition=2021', '-O', str(source), '-o', str(fake)], check=True, capture_output=True, timeout=30)
    fake_command = [str(parent), str(fake)] + font_args
    environ = os.environ.copy()
    environ['STUDIO_MERMAID_PRIVATE_ENV'] = 'must not reach worker'
    result = subprocess.run(fake_command, input=json.dumps(dict(request, source='sandbox-probe')),
                            text=True, capture_output=True, timeout=5, env=environ)
    listener.close()
    assert result.returncode == 0 and 'file:true,env:true,home:true,network:true' in result.stdout, result.stderr
    (out / 'namespace.svg').write_text(result.stdout)
    cleanup = []
    for flag in ['--timeout-ms', '--cancel-ms']:
        proc = subprocess.Popen(fake_command + [flag, '350'], stdin=subprocess.PIPE,
                                stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        try:
            proc.stdin.write(json.dumps(dict(request, source='wait-probe')))
            proc.stdin.close()
            proc.stdin = None
            owned = []
            until = time.monotonic() + 0.25
            while time.monotonic() < until:
                owned = list(set(owned + children(proc.pid)))
                time.sleep(0.01)
            stdout, stderr = proc.communicate(timeout=5)
        finally:
            if proc.poll() is None:
                proc.kill()  # The sandbox also dies with its owning parent.
                proc.wait()
        assert proc.returncode and 'budget' in stderr, stderr
        assert len(owned) >= 3, ('descendant did not start', owned)
        until = time.monotonic() + 2
        while alive(owned) and time.monotonic() < until:
            time.sleep(0.02)
        survivors = alive(owned)
        assert not survivors, (flag, survivors)
        cleanup.append(dict(mode=flag, observedProcesses=len(owned), survivors=survivors))
    result = run(fake_command, dict(request, source='flood-probe'), timeout=5)
    assert result.returncode and 'budget' in result.stderr, result.stderr
    validation = dict(status='passed', automaticFonts=args.auto_fonts, protocolVersion=args.protocol_version, cases=checks, namespace='private host file, home, network and environment denied',
                      cleanup=cleanup, realWorkerLimits=True, strictAdmission=True, outputLimit=True,
                      byteIdenticalToPairedCorpus=sum(x['equal'] for x in comparisons), comparisons=comparisons,
                      workerSha256=hashlib.sha256(worker.read_bytes()).hexdigest(),
                      parentSha256=hashlib.sha256(parent.read_bytes()).hexdigest(),
                      applicationLaunchPerformed=False, goalComplete=False)
    (out / 'VALIDATED.json').write_text(json.dumps(validation, indent=2) + '\n')
    print(json.dumps({k:validation[k] for k in ['status','realWorkerLimits','strictAdmission','outputLimit','byteIdenticalToPairedCorpus','cleanup']}))


if __name__ == '__main__':
    main()
