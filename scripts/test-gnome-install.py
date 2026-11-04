#!/usr/bin/env python3
"""Verify a real bundle's standalone installer in private prefixes, without GUI."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile


def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def immutable_copy(source, destination):
    # Binaries/notices are immutable. Modified fixture files are unlinked first,
    # and the original complete bundle is verified again before returning.
    try:
        os.link(source, destination)
    except OSError:
        shutil.copy2(source, destination)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('bundle', type=Path)
    args = parser.parse_args()
    bundle = args.bundle.resolve(strict=True)
    repo = Path(__file__).resolve().parents[1]
    base = Path(os.environ.get('STUDIO_NATIVE_QA_DIR') or repo / 'target/gnome-native-smoke')
    base.mkdir(parents=True, exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix='bundle-install-', dir=base))
    prefix = root / 'native install 中文 % \' " $ space'
    tools = root / 'tools'
    tools.mkdir()
    for name in ['bash', 'sha256sum', 'mkdir', 'cp', 'chmod', 'mktemp', 'mv',
                 'rm', 'ln', 'install', 'dirname', 'update-desktop-database']:
        executable = shutil.which(name)
        if executable:
            (tools/name).symlink_to(executable)
    env = dict(os.environ, PATH=str(tools))
    assert shutil.which('python3', path=env['PATH']) is None
    (prefix/'bin').mkdir(parents=True)
    canaries = {}
    for name in ['d2', 'java', 'libpdfium.so', 'codex-thread-studio']:
        path = prefix/'bin'/name
        path.write_text('Unrelated existing installation: ' + name)
        canaries[name] = sha(path)

    def install(source, destination, success=True):
        with (root/'installer.log').open('a') as log:
            result = subprocess.run([str(tools/'bash'), str(source/'install.sh'),
                                     str(source), str(destination)], cwd=root,
                                    env=env, stdout=log, stderr=log)
        assert (result.returncode == 0) == success, (result.returncode, root)

    install(bundle, prefix)
    native = prefix/'bin/codex-thread-studio-gnome'
    first = native.resolve(strict=True).parent
    assert first.is_relative_to(prefix/'libexec/codex-thread-studio-gnome')
    assert sha(first/'codex-thread-studio-gnome') == sha(bundle/'codex-thread-studio-gnome')
    for name in ['codex-thread-studio-gateway', 'codex-thread-studio-mermaid']:
        assert (prefix/'bin'/name).resolve(strict=True).parent == first
    first_inventory = {str(p.relative_to(first)): sha(p) for p in first.rglob('*') if p.is_file()}
    install(bundle, prefix)
    assert native.resolve().parent == first, 'Reinstall changed immutable version identity'
    assert len(list((prefix/'libexec/codex-thread-studio-gnome').iterdir())) == 1

    changed = root/'changed bundle'
    shutil.copytree(bundle, changed, copy_function=immutable_copy)
    (changed/'README.txt').unlink()
    (changed/'README.txt').write_text('Changed package documentation.\n')
    if (changed/'d2').exists():
        (changed/'d2').unlink()
    subprocess.run(['bash', str(repo/'scripts/package-gnome-checksums.sh'), str(changed)], check=True)
    install(changed, prefix)
    second = native.resolve(strict=True).parent
    assert second != first and not (second/'d2').exists(), 'Update retained a stale optional engine'
    assert {str(p.relative_to(first)): sha(p) for p in first.rglob('*') if p.is_file()} == first_inventory
    for name, digest in canaries.items():
        assert sha(prefix/'bin'/name) == digest, 'Installer overwrote an unrelated executable'
    (changed/'README.txt').unlink()
    (changed/'README.txt').write_text('Unverified modification.\n')
    rejected = root/'must not be created'
    install(changed, rejected, success=False)
    assert not rejected.exists(), 'Corrupt bundle changed the installation prefix'
    assert not list((prefix/'libexec/codex-thread-studio-gnome').glob('.install.*'))
    subprocess.run(['sha256sum', '--check', '--strict', '--quiet', 'SHA256SUMS'], cwd=bundle, check=True)
    validator = shutil.which('desktop-file-validate')
    if validator:
        subprocess.run([validator, str(prefix/'share/applications/io.github.messon007.codexthreadstudio.Gnome.desktop')], check=True)
    result = dict(ok=True, standaloneInstall=True, installerNeedsNoPython=True,
                  privateSiblings=True, unrelatedToolsRetained=True, reinstallReusesVersion=True,
                  updateRetainsOldVersion=True, noStaleOptionalRuntime=True, corruptionRejected=True,
                  desktopEntryValidated=bool(validator), originalBundleUnchanged=True, artifacts=str(root))
    (root/'result.json').write_text(json.dumps(result, indent=2)+'\n')
    print(json.dumps(result))


if __name__ == '__main__':
    main()
