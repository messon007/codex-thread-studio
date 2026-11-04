#!/usr/bin/env python3
"""Assemble the native worker plus exact dependency licenses/source provenance."""
import argparse
import hashlib
import importlib.util
import json
import os
import re
from pathlib import Path
import shutil
import subprocess
import tomllib


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--metadata', required=True, type=Path)
    parser.add_argument('--binary', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    worker = root / 'native' / 'mermaid-worker'
    spec = importlib.util.spec_from_file_location('prepare_mermaid', root / 'scripts/prepare-gnome-mermaid.py')
    prepare = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(prepare)
    metadata = json.loads(args.metadata.read_text())
    admitted = {node['id'] for node in metadata['resolve']['nodes']}
    packages = [p for p in metadata['packages'] if p['id'] in admitted]
    patched = {}
    for package in packages:
        if package['name'] not in prepare.COMPONENTS:
            continue
        crate = package['name']
        version, archive, suffix = prepare.COMPONENTS[crate]
        source = Path(package['manifest_path']).resolve().parent
        patch = worker / 'patches' / f'{crate}-{version}-{suffix}.patch'
        if (package['version'] != version or package['source'] is not None
                or source != (worker / 'vendor' / crate).resolve()):
            raise RuntimeError(f'Cargo resolved an unexpected patched source: {crate}')
        prepare.verify(source, digest(patch), archive)
        patched[crate] = source
    if set(patched) != set(prepare.COMPONENTS):
        raise RuntimeError('Cargo metadata is missing a pinned patched component')
    lock = tomllib.loads((worker / 'Cargo.lock').read_text())
    checksums = {(p['name'], p['version']): p.get('checksum') for p in lock['package']}
    binary = args.binary.resolve(strict=True)
    if not os.access(binary, os.X_OK):
        raise RuntimeError('Worker binary is not executable')
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    dest = out / 'codex-thread-studio-mermaid'
    shutil.copy2(binary, dest)
    notices = out / 'licenses' / 'mermaid'
    notices.mkdir(parents=True)
    components = []
    missing = []
    for package in sorted(packages, key=lambda p: (p['name'], p['version'])):
        name, version = package['name'], package['version']
        source = Path(package['manifest_path']).parent
        component = notices / f'{name}-{version}'
        files = set()
        for directory, dirs, names in os.walk(source, followlinks=False):
            dirs[:] = [d for d in dirs if d not in ['target', '.git', 'vendor']]
            for file in names:
                path = Path(directory) / file
                legal_tree = any(part.lower().startswith(('license', 'third_party'))
                                 for part in path.relative_to(source).parts[:-1])
                if legal_tree or file.lower().startswith(('license', 'copying', 'notice', 'ofl', 'font_notice', 'third_party')):
                    if path.is_file():
                        files.add(path)
        if package.get('license_file'):
            files.add(source / package['license_file'])
        if name == 'codex-thread-studio-mermaid':
            component.mkdir(parents=True)
            shutil.copy2(root / 'LICENSE', component / 'LICENSE')
        elif name.startswith('ratex-') and version == '0.1.15' and package.get('license') == 'MIT' and package.get('repository') == 'https://github.com/erweixin/RaTeX':
            # These published subcrates omit their workspace's common MIT file.
            # Preserve the already pinned upstream license/provenance and OFL
            # font notices used by the app's identical RaTeX version.
            component.mkdir(parents=True)
            for notice in ['LICENSE', 'UPSTREAM.md', 'OFL.txt', 'FONT_NOTICE.txt']:
                shutil.copy2(root / 'native/gnome/vendor/ratex' / notice, component / notice)
        elif not files:
            missing.append(f'{name}-{version}')
        for file in sorted(files):
            target = component / file.relative_to(source)
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(file, target)
        if name in ['merman-elk-layered', 'merman-layout-elk']:
            # Preserve the translated port's original Eclipse ELK/elkjs adapter
            # revisions, in addition to the crate download provenance below.
            shutil.copy2(source / 'README.md', component / 'UPSTREAM-README.md')
        notice_count = sum(p.is_file() for p in component.rglob('*'))
        components.append(dict(name=name, version=version, license=package.get('license'),
                               repository=package.get('repository'), source=package.get('source'),
                               crateSha256=checksums.get((name, version)), notices=notice_count))
    if missing:
        raise RuntimeError(f'Inspect missing license files before using this component: {missing}; output retained at {out}')
    patch = worker / 'patches' / 'merman-render-0.9.3-native-paint.patch'
    patches = {}
    for file in sorted((worker / 'patches').glob('*.patch')):
        shutil.copy2(file, notices / file.name)
        patches[file.name] = digest(file)
    for crate, source in patched.items():
        origin = source / prepare.ORIGIN
        shutil.copy2(origin, notices / f'{crate}-source.json')
    shutil.copy2(worker / 'Cargo.lock', notices / 'Cargo.lock')
    (notices / 'COMPONENTS.json').write_text(json.dumps(components, indent=2) + '\n')
    (notices / 'SOURCES.txt').write_text(
        'Published crate sources: https://crates.io/crates/<name>/<version>/download\n'
        'Exact names, versions, repositories and archive checksums: COMPONENTS.json and Cargo.lock\n'
        'Merman 0.9.3 upstream commit: 3b915f69c02d55b873d252cf6c8a6048e7f3427d\n'
        'merman-render, merman-export and usvg carry the enclosed paint/font-context/font-cluster patches; other sources are unmodified.\n'
        'Merman ELK components include EPL-2.0 notices; RaTeX fonts include SIL OFL notices.\n')
    version = int(re.search(r'pub const VERSION: u8 = (\d+);', (worker / 'src/protocol.rs').read_text()).group(1))
    manifest = dict(protocolVersion=version, patchesSha256=patches, binarySha256=digest(dest), patchSha256=digest(patch),
                    patchedSources={p['name']: dict(
                        **json.loads((patched[p['name']] / prepare.ORIGIN).read_text()),
                        cargoPackageId=p['id']) for p in packages if p['name'] in patched},
                    rustc=subprocess.check_output(['rustc', '-vV'], text=True),
                    sourceSha256={str(p.relative_to(worker)): digest(p) for p in sorted((worker / 'src').glob('*.rs'))},
                    components=len(components), componentOnly=True)
    manifest['filesSha256'] = {str(p.relative_to(out)): digest(p) for p in sorted(out.rglob('*')) if p.is_file()}
    (out / 'MERMAID-WORKER.json').write_text(json.dumps(manifest, indent=2) + '\n')
    print(json.dumps(dict(component=str(out), binarySha256=manifest['binarySha256'], components=len(components))))


if __name__ == '__main__':
    main()
