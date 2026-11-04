#!/usr/bin/env python3
"""Compare committed Web desktop reader sources only; this does not establish compatibility.

Explicit maintainer command. Normal builds/CI need no private MarkNoria checkout.
Uncommitted files, generated assets, caches and running applications are not read.
"""
import argparse
import datetime
import hashlib
import json
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FEATURE_SOURCES = {
    'studio': ['scripts/marknoria-embedding.mjs', 'ui/large-markdown-host.mjs',
               'ui/document-workspace-controller.mjs', 'ui/table-reader.mjs',
               'ui/table-data.mjs', 'ui/pdf-reader.mjs', 'ui/epub-reader.mjs',
               'src-tauri/src/large_markdown.rs'],
    'marknoria': ['src/web/mod.rs', 'src/web/document_runtime.rs',
                  'src/core/documents/pdf.rs', 'src/core/documents/archive.rs',
                  'src/core/edit.rs', 'src/core/notes.rs',
                  'web/reader.mjs', 'web/table-layout.mjs', 'web/tables.mjs',
                  'web/code-view.mjs', 'web/notes.mjs', 'web/assets.mjs'],
}


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
    presentation = []
    for path in paths:
        target = 'web/' + Path(path).name
        if (Path(path).parent.as_posix() == 'crates/marknoria/web'
                and Path(path).suffix in {'.html', '.css', '.js'} and target in upstream):
            left, a = source(studio, refs['studioMain'], path)
            right, b = source(mark, refs['marknoriaMain'], target)
            presentation.append({'studio': left, 'marknoria': right, 'identical': a == b})
    evidence = {
        'studio': [source(studio, refs['studioMain'], path)[0]
                   for path in FEATURE_SOURCES['studio']],
        'marknoria': [source(mark, refs['marknoriaMain'], path)[0]
                     for path in FEATURE_SOURCES['marknoria']],
    }
    patches = {}
    for path in ['scripts/marknoria-queries.patch', 'scripts/marknoria-embedding.mjs']:
        patches[path], _ = source(studio, refs['studioMain'], path)
    return {'schemaVersion': 2, 'analysedOn': args.date,
            'purpose': 'Immutable source comparison evidence; not a live sync manifest, '
                       'compatibility guarantee, or feature-parity score.',
            'snapshots': refs,
            'scope': 'Web desktop reader and Rust dependencies used by that reader only; '
                     'platform-native desktop and unpublished Studio native branches are excluded.',
            'webSummary': {'commonModules': len(web),
                           'identicalModules': sum(row['identical'] for row in web)},
            'webModules': web, 'studioOnlyWebModules': studio_only,
            'upstreamOnlyWebModules': upstream_only,
            'webScope': 'Top-level reader .mjs source presence only; excludes bundled assets/CSS. '
                        'An upstream-only module is not automatically a required downstream feature.',
            'webPresentationFiles': presentation, 'featureSourceEvidence': evidence,
            'currentPatchFiles': patches,
            'method': 'git show <recorded commit>:<path>; SHA-256 over exact file bytes; '
                      'line counts from splitlines. Source equality or presence does not establish behavior, '
                      'performance, source-anchor, or security equivalence.'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--marknoria-checkout', type=Path, required=True)
    parser.add_argument('--studio-ref', default='HEAD')
    parser.add_argument('--marknoria-ref', default='HEAD')
    parser.add_argument('--date', default=datetime.datetime.now(datetime.timezone.utc).date().isoformat())
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
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
