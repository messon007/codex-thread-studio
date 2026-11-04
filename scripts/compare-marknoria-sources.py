#!/usr/bin/env python3
"""Compare committed reader sources only; this does not establish compatibility.

Explicit maintainer command. Normal builds/CI need no private MarkNoria checkout.
Uncommitted files, generated assets, caches and running applications are not read.
"""
import argparse
import datetime
import difflib
import hashlib
import json
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
NATIVE_PAIRS = [
    ('crates/markdown-native/src/structured.rs', 'src/core/documents/structured.rs'),
    ('crates/svg-resources/src/lib.rs', 'src/core/documents/svg_resources.rs'),
    ('src-tauri/src/native_image_animation.rs', 'src/core/documents/image_animation.rs'),
]


def git(repo, *args):
    return subprocess.check_output(['git', '-C', str(repo), *args], stderr=subprocess.PIPE)


def commit(repo, ref):
    return git(repo, 'rev-parse', '--verify', ref + '^{commit}').decode().strip()


def source(repo, ref, path):
    data = git(repo, 'show', ref + ':' + path)
    return {'path': path, 'sha256': hashlib.sha256(data).hexdigest(),
            'lines': len(data.decode('utf-8').splitlines())}, data


def compare(args):
    studio = ROOT
    mark = args.marknoria_checkout.resolve()
    refs = {'studioMain': commit(studio, args.studio_ref),
            'marknoriaMain': commit(mark, args.marknoria_ref)}
    provenance = git(studio, 'show', refs['studioMain'] + ':crates/marknoria/UPSTREAM.md').decode()
    refs['vendoredBase'] = re.search(r'^Revision: ([a-f0-9]{40})$', provenance, re.M).group(1)
    paths = git(studio, 'ls-tree', '-r', '--name-only', refs['studioMain'],
                'crates/marknoria/web').decode().splitlines()
    upstream = set(git(mark, 'ls-tree', '-r', '--name-only', refs['marknoriaMain'],
                       'web').decode().splitlines())
    web = []
    for path in paths:
        target = 'web/' + Path(path).name
        if (Path(path).parent.as_posix() != 'crates/marknoria/web'
                or not path.endswith('.mjs') or target not in upstream):
            continue
        left, a = source(studio, refs['studioMain'], path)
        right, b = source(mark, refs['marknoriaMain'], target)
        web.append({'studio': left, 'marknoria': right, 'identical': a == b})
    matched_studio = {row['studio']['path'] for row in web}
    matched_mark = {row['marknoria']['path'] for row in web}
    studio_only = [source(studio, refs['studioMain'], path)[0] for path in paths
                   if Path(path).parent.as_posix() == 'crates/marknoria/web'
                   and path.endswith('.mjs') and path not in matched_studio]
    upstream_only = [source(mark, refs['marknoriaMain'], path)[0] for path in sorted(upstream)
                     if Path(path).parent.as_posix() == 'web'
                     and path.endswith('.mjs') and path not in matched_mark]
    samples = []
    native = {'status': 'Not scanned; native parity is unknown.'}
    if args.native_checkout:
        repo = args.native_checkout.resolve()
        refs['studioNative'] = commit(repo, args.native_ref)
        native = {'label': args.native_label, 'status':
                  'Reference only; no main-branch integration or publication is implied. '
                  'Only committed files were read; working-tree modifications were excluded.'}
        for left_path, right_path in NATIVE_PAIRS:
            left, a = source(repo, refs['studioNative'], left_path)
            right, b = source(mark, refs['marknoriaMain'], right_path)
            samples.append({'studioNative': left, 'marknoria': right, 'identical': a == b,
                            'lineSimilarity': round(difflib.SequenceMatcher(
                                None, a.decode().splitlines(), b.decode().splitlines()).ratio(), 3)})
    patches = {}
    for path in ['scripts/marknoria-queries.patch', 'scripts/marknoria-embedding.mjs']:
        patches[path], _ = source(studio, refs['studioMain'], path)
    return {'schemaVersion': 1, 'analysedOn': args.date,
            'purpose': 'Immutable source comparison evidence; not a live sync manifest, '
                       'compatibility guarantee, or feature-parity score.',
            'snapshots': refs, 'nativeReference': native,
            'webSummary': {'commonModules': len(web),
                           'identicalModules': sum(row['identical'] for row in web)},
            'webModules': web, 'studioOnlyWebModules': studio_only,
            'upstreamOnlyWebModules': upstream_only,
            'webScope': 'Top-level reader .mjs source presence only; excludes bundled assets/CSS. '
                        'An upstream-only module is not automatically a required downstream feature.',
            'nativeSourceSamples': samples, 'currentPatchFiles': patches,
            'method': 'git show <recorded commit>:<path>; SHA-256 over exact file bytes; '
                      'line counts from splitlines; sampled line similarity from Python '
                      'difflib.SequenceMatcher. Similarity does not establish behavior, '
                      'performance, source-anchor, or security equivalence.'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--marknoria-checkout', type=Path, required=True)
    parser.add_argument('--studio-ref', default='HEAD')
    parser.add_argument('--marknoria-ref', default='HEAD')
    parser.add_argument('--native-checkout', type=Path)
    parser.add_argument('--native-ref')
    parser.add_argument('--native-label', default='native-reference')
    parser.add_argument('--date', default=datetime.datetime.now(datetime.timezone.utc).date().isoformat())
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    if bool(args.native_checkout) != bool(args.native_ref):
        parser.error('--native-checkout and --native-ref must be supplied together')
    try:
        report = compare(args)
    except (subprocess.CalledProcessError, OSError, UnicodeError, AttributeError) as error:
        parser.exit(1, 'Comparison unavailable; compatibility is unknown: ' + str(error) + '\n')
    text = json.dumps(report, ensure_ascii=False, indent=2) + '\n'
    if args.output:
        args.output.write_text(text)
    else:
        print(text, end='')


if __name__ == '__main__':
    main()
