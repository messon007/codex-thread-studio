#!/usr/bin/env python3
"""Exercise selectable Preview through the real reader actor on owned Wayland."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import resource
import shutil
import subprocess
import sys
import tempfile
import time

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).with_name('test-support')))
from memory_sampler import ProcessMemory
from private_wayland import isolated_environment, OwnedMutterInput

spec = importlib.util.spec_from_file_location(
    'flow_runtime', Path(__file__).with_name('smoke-gnome-router-runtime.py'))
runtime = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runtime)


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def child(native, gateway, root):
    before = {p.name: digest(p) for p in (root/'project').glob('*.md')}
    owned = OwnedMutterInput()
    owned.enable_clipboard_reader()
    with (root/'app.log').open('w') as log:
        app = subprocess.Popen([native], cwd=root, env=os.environ,
                               stdout=log, stderr=log)
        memory = ProcessMemory(app.pid, gateway)
        try:
            deadline = time.monotonic()+110
            while app.poll() is None:
                owned.respond(root)
                if time.monotonic()>deadline:
                    raise TimeoutError(('Reader pipeline timed out', root))
                time.sleep(.01)
            code = app.returncode
        except BaseException:
            app.terminate()
            try:
                app.wait(timeout=10)
            except subprocess.TimeoutExpired:
                app.kill()
                app.wait(timeout=5)
            raise
        finally:
            measured = memory.close()
            owned.close()
    assert (root/'flow-pipeline.json').is_file(), (
        root, code, (root/'app.log').read_text()[-2500:])
    result = json.loads((root/'flow-pipeline.json').read_text())
    assert code == 0 and result['ok'], (root, result)
    assert not any(s in (root/'app.log').read_text()
                   for s in ['CRITICAL', 'panicked', 'Gtk-WARNING']), root
    assert runtime.starts(root) == [], 'Reader test sent a provider prompt'
    assert before == {p.name: digest(p) for p in (root/'project').glob('*.md')}
    repeated=result['repeatedMath']
    assert repeated['ok'] and repeated['formulas']==64 and repeated['mountedRasters']==64
    assert repeated['renderedObjects']==1 and repeated['reusedObjects']==63
    assert all(repeated[k] for k in ['overBudgetNoticePersisted','overBudgetSourceReadable','noticeClearsAfterNewFile'])
    assert repeated['exactIndependentCopy'] and repeated['sourcePreviewRoundTrip'] and repeated['retiredObjectsUnmapped']
    surfaces = result['codeSurfaces']
    assert surfaces['normalDocumentActor'] and surfaces['styledAndPlain']
    assert len(surfaces['blankLinePixels']) == 9 and surfaces['noProsePaint']
    assert surfaces['prosePixelSamples'] >= 2
    assert all(s['firstLinePixel'] == s['expected'] == s['blankLinePixel']
               for s in surfaces['blankLinePixels'])
    assert len(surfaces['codeGutters']) == 18 and all(row['painted'] for row in surfaces['codeGutters'])
    assert surfaces['wrappedGutterChecks'] == 3 and surfaces['retiredGutterRejected']
    mixed=result['mixedPreview']
    assert mixed['ok'] and mixed['formulas']==16 and mixed['tables']==8
    assert mixed['renderedObjects']==1 and mixed['reusedObjects']==15
    assert mixed['visibleCellFormulasPainted']
    assert 0 < mixed['initialActiveTables'] < 8 and mixed['initialUntouchedTables'] > 0
    assert all(mixed[k] for k in ['tableFactoriesReleased','tablePageAndWidthRetained','tableScrollRetained','idleBindingsStable'])
    assert mixed['exactFormulaCopy'] and mixed['fullTableCopy'] and mixed['nativeEnd'] and mixed['sourceRetirement']
    dense = result['densePreview']
    assert dense['ok'] and dense['normalDocumentActor'] and dense['realEnginesAndImages']
    assert {case['kind'] for case in dense['cases']} == {'images', 'diagrams'}
    for case in dense['cases']:
        assert case['objects'] == 64 and case['initialLoaded'] < 64, case
        assert case['maximumObservedActiveDiagrams'] <= 2, case
        assert all(case[key] for key in [
            'offscreenDeferred', 'oldPixelsReleased', 'nativeEndKey',
            'nativeHomeKey', 'pageUpOverridesEnd', 'repeatedEnd', 'sourceCleanup']), case
    for name in ['code-surfaces-light.png', 'code-surfaces-dark.png', 'pipeline-small.png', 'pipeline-code.png', 'pipeline-empty-code.png', 'pipeline-wide.png', 'pipeline-report.png',
                 'dense-mixed-first.png', 'dense-mixed-last.png', 'dense-mixed-restored.png',
                 'repeated-math-first.png', 'repeated-math-last.png', 'distinct-math-rejected.png',
                 'dense-images-first.png', 'dense-images-last.png',
                 'dense-diagrams-first.png', 'dense-diagrams-last.png']:
        assert (root/name).stat().st_size > 1000, name
    for path in root.glob('*.pid'):
        for _ in range(100):
            if not Path('/proc', path.read_text().strip()).exists():
                break
            time.sleep(.05)
        assert not Path('/proc', path.read_text().strip()).exists(), path
    measurement = dict(result, memory=measured, artifacts=str(root),
                       sourcesUnchanged=True, noProviderPrompts=True, ownedCleanup=True)
    measurement['requestedRenderer'] = os.environ.get('GSK_RENDERER') or 'automatic'
    measurement['openingTimingScope'] = 'after-first-shell-frame'
    if (root/'graphics-cache-seed.json').is_file():
        measurement['graphicsCacheSeed'] = json.loads((root/'graphics-cache-seed.json').read_text())
    (root/'measurement.json').write_text(json.dumps(measurement, indent=2)+'\n')
    print(json.dumps(measurement))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('native')
    parser.add_argument('gateway')
    parser.add_argument('--reference', type=Path)
    parser.add_argument('--graphics-cache-from', type=Path,
                        help='Reuse only GPU caches from a completed owned fixture; app/document caches stay fresh')
    parser.add_argument('--private-root', type=Path)
    args = parser.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    native, gateway = str(Path(args.native).resolve()), str(Path(args.gateway).resolve())
    if args.private_root:
        return child(native, gateway, args.private_root)
    if not args.reference or not args.reference.is_file():
        parser.error('--reference must name an existing large Markdown file (copied, never edited)')
    base = Path(__file__).resolve().parents[1]/'target/gnome-native-smoke'
    base.mkdir(parents=True, exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix='flow-pipeline-', dir=base))
    if args.graphics_cache_from:
        source = args.graphics_cache_from.absolute()
        if source.parent != base.absolute() or not source.name.startswith('flow-pipeline-'):
            parser.error('--graphics-cache-from must name an owned flow-pipeline fixture')
        previous = json.loads((source/'measurement.json').read_text())
        if not all(previous.get(k) for k in ['ok', 'sourcesUnchanged', 'ownedCleanup']):
            parser.error('--graphics-cache-from requires a completed successful fixture')
        seeds = []
        for name in ['nvidia/GLCache', 'mesa_shader_cache', 'mesa_shader_cache_db', 'mesa_shader_cache_sf']:
            directory = source/'cache'/name
            if not directory.exists():
                continue
            if directory.is_symlink() or not directory.is_dir() or not directory.resolve().is_relative_to(source.resolve()):
                parser.error('GPU cache must be an owned directory')
            for item in directory.rglob('*'):
                if item.is_symlink() or not (item.is_dir() or item.is_file()):
                    parser.error('GPU cache contains a link or special file')
                if item.is_file():
                    seeds.append(item)
        if not seeds or len(seeds) > 8192 or sum(p.stat().st_size for p in seeds) > 128*1024*1024:
            parser.error('GPU cache seed must contain at most 8192 files / 128 MiB')
        manifest = {}
        for item in seeds:
            relative = item.relative_to(source/'cache')
            target = root/'cache'/relative
            target.parent.mkdir(parents=True, exist_ok=True)
            expected = digest(item)
            shutil.copyfile(item, target)
            assert digest(target) == expected, 'GPU cache changed while being copied'
            manifest[str(relative)] = expected
        (root/'graphics-cache-seed.json').write_text(json.dumps(dict(
            source=source.name, files=manifest), indent=2)+'\n')
    runtime.create_profile(root)
    settings = root/'config/codex-thread-studio/settings.json'
    raw = json.loads(settings.read_text())
    raw['language'] = 'en-US'
    settings.write_text(json.dumps(raw))
    (settings.parent/'gnome.json').write_text(json.dumps(
        dict(selectedBackend='codex', selectedSession='target')))
    code = runtime.CODEX.replace(
        "    elif method=='thread/read':",
        "    elif method=='model/list':answer(identity,{'data':[]})\n"
        "    elif method=='thread/items/list':answer(identity,error='Method not found')\n"
        "    elif method=='thread/read':")
    for name, body in [('fake-codex.py', code), ('fake-opencode.py', runtime.OPENCODE)]:
        path = root/name
        path.write_text(body)
        path.chmod(0o700)
    (root/'project/pipeline.md').write_text(
        '# Pipeline\n\nFirst **Unicode 中文😀**.\n\nSecond paragraph.\n\n'
        '```rust\nlet value = 7;\n```\n\n'
        '| Name | Value |\n| --- | --- |\n| First | One |\n\n'
        '## Target section\n\nSection destination.\n')
    (root/'project/empty-code.md').write_text('```\n```\n')
    (root/'project/code-surfaces.md').write_text(
        '# Code surfaces\n\n'
        '```rust\nlet text = "中文😀";\n\n// Short line\n```\n\nFollowing styled code.\n\n'
        '```unknown-syntax\nPlain 中文😀\n\nEnd plain code.\n```\n\nFollowing plain code.\n\n'
        '```rust\n// Long line limit\n\n' + 'x'*4100 + '\n```\n\nFollowing limited code.\n')

    (root/'project/dense-image.svg').write_text(
        '<svg xmlns="http://www.w3.org/2000/svg" width="320" height="96">'
        '<rect width="320" height="96" fill="#078878"/>'
        '<text x="20" y="55" font-size="24" fill="white">Native dense image</text></svg>')
    (root/'project/dense-images.md').write_text(''.join(
        f'![Image {i}](dense-image.svg)\n\n' for i in range(64)))
    (root/'project/dense-diagrams.md').write_text(''.join(
        f'```mermaid\nflowchart LR\nA{i}[Start {i}]-->B{i}[Done {i}]\n```\n\n'
        for i in range(64)))
    (root/'project/wide.md').write_text(
        '# Wide\n\n| '+' | '.join(f'Column {i}' for i in range(256))+' |\n| '+
        ' | '.join('---' for _ in range(256))+' |\n| '+
        ' | '.join(f'Value {i}' for i in range(256))+' |\n')
    (root/'project/dense-mixed.md').write_text(''.join(
        f'## Mixed section {i}\n\nFirst $\\frac{{x^2+1}}{{1+y}}$ and second $\\frac{{x^2+1}}{{1+y}}$.\n\n'
        '| Item | Formula | Note |\n| --- | --- | --- |\n'+''.join(
            f'| MIXED-{i}-{n} | $z^2$ | Wrapping native table text 中文😀 {n} |\n' for n in range(60))+'\n'
        for i in range(8)))
    (root/'project/repeated-math.md').write_text('# Repeated formulas\n\n'+''.join(
        f'REPEAT-{i}: $\\frac{{x^2+1}}{{1+y}}$ · 中文😀.\n\n' for i in range(64)))
    (root/'project/distinct-math.md').write_text('# Distinct formulas\n\n'+''.join(
        f'DISTINCT-{i}: $x_{{{i}}}$.\n\n' for i in range(33)))
    shutil.copyfile(args.reference, root/'project/large.md')
    env = {k: v for k, v in os.environ.items() if not k.startswith('STUDIO_GNOME_SMOKE_')}
    env.update(
        CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway,
        CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),
        CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),
        CODEX_HOME=str(root/'codex'), XDG_CONFIG_HOME=str(root/'config'),
        XDG_DATA_HOME=str(root/'data'), XDG_CACHE_HOME=str(root/'cache'),
        READER_CACHE_DIR=str(root/'reader-cache'), GDK_BACKEND='wayland',
        G_DEBUG='fatal-criticals', STUDIO_OWNED_MUTTER_INPUT='1', STUDIO_GNOME_SMOKE_DIR=str(root),
        STUDIO_GNOME_SMOKE_FLOW_PIPELINE='1', STUDIO_GNOME_CONTINUOUS_PREVIEW='1')
    with isolated_environment(env, headless=True) as private:
        subprocess.run([
            'dbus-run-session', '--', shutil.which('mutter') or 'mutter',
            '--headless', '--wayland', '--no-x11', '--wayland-display',
            'studio-test', '--virtual-monitor', '1600x1000', '--',
            sys.executable, str(Path(__file__).resolve()), native, gateway,
            '--private-root', str(root)], cwd=root, env=private,
            check=True, timeout=140)


if __name__ == '__main__':
    main()
