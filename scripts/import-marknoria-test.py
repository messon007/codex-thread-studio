"""Synthetic archive/import regression; no upstream checkout or download."""
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location('reader_import', Path(__file__).with_name('import-marknoria.py'))
reader = importlib.util.module_from_spec(spec)
spec.loader.exec_module(reader)


class ReaderImportTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        (self.root / 'crates/marknoria').mkdir(parents=True)
        self.retained = self.root / 'crates/marknoria/retained.txt'
        self.retained.write_text('Retain on any failed import')
        files = {'src/lib.rs': hashlib.sha256(b'// Synthetic reader\n').hexdigest()}
        tree = hashlib.sha256(json.dumps(files, sort_keys=True, separators=(',', ':')).encode()).hexdigest()
        self.pin = dict(version=1, revision='a' * 40, treeSha256=tree, apiVersion=1, bridgeVersion=1)
        self.entries = [('marknoria-reader/reader-manifest.json', json.dumps({**self.pin, 'files': files}).encode()),
                        ('marknoria-reader/src/lib.rs', b'// Synthetic reader\n')]

    def archive(self, entries=None):
        archive = self.root / 'reader.tar.gz'
        with tarfile.open(archive, 'w:gz') as output:
            for name, value in self.entries if entries is None else entries:
                info = tarfile.TarInfo(name)
                if isinstance(value, bytes):
                    info.size = len(value)
                    output.addfile(info, io.BytesIO(value))
                else:
                    info.type = value
                    info.linkname = 'src/lib.rs'
                    output.addfile(info)
        self.pin['archiveSha256'] = reader.digest(archive)
        return archive

    def rejected(self, entries=None, change=None):
        archive = self.archive(entries)
        if change:
            change()
        with self.assertRaises((ValueError, FileNotFoundError)):
            reader.install(archive, self.root, self.pin)
        self.assertEqual(self.retained.read_text(), 'Retain on any failed import')
        self.assertEqual(sorted(path.name for path in (self.root / 'crates').iterdir()), ['marknoria'])

    def test_valid_check_and_install(self):
        archive = self.archive()
        self.assertFalse(reader.install(archive, self.root, self.pin, check=True)['installed'])
        self.assertTrue(self.retained.exists())
        self.assertTrue(reader.install(archive, self.root, self.pin)['installed'])
        self.assertFalse(self.retained.exists())
        self.assertEqual(reader.verify(self.root / 'crates/marknoria', self.pin)['files'], 1)

    def test_external_digest(self):
        self.rejected(change=lambda: self.pin.update(archiveSha256='0' * 64))

    def test_revision_pin(self):
        self.rejected(change=lambda: self.pin.update(revision='b' * 40))

    def test_extra_file(self):
        self.rejected(self.entries + [('marknoria-reader/unexpected.txt', b'private')])

    def test_missing_file(self):
        self.rejected(self.entries[:1])

    def test_modified_file(self):
        self.rejected([self.entries[0], ('marknoria-reader/src/lib.rs', b'changed')])

    def test_unsafe_paths(self):
        for path in ['../escape', '/absolute', 'marknoria-reader/../escape', 'other/file',
                     'marknoria-reader/./file', 'marknoria-reader/x\\y']:
            with self.subTest(path=path):
                self.rejected(self.entries + [(path, b'bad')])

    def test_duplicate_path(self):
        self.rejected(self.entries + [self.entries[1]])

    def test_special_entries(self):
        for kind in [tarfile.SYMTYPE, tarfile.LNKTYPE, tarfile.FIFOTYPE]:
            with self.subTest(kind=kind):
                self.rejected(self.entries + [('marknoria-reader/unsafe', kind)])

    def test_byte_budget(self):
        old = reader.MAX_TOTAL
        reader.MAX_TOTAL = 1
        try:
            self.rejected()
        finally:
            reader.MAX_TOTAL = old

    def test_failed_replacement_rolls_back(self):
        archive = self.archive()
        replace = reader.os.replace
        def fail_candidate(source, destination):
            if Path(source).name == 'marknoria-reader':
                raise OSError('Synthetic replacement failure')
            return replace(source, destination)
        with patch.object(reader.os, 'replace', side_effect=fail_candidate):
            with self.assertRaisesRegex(OSError, 'Synthetic'):
                reader.install(archive, self.root, self.pin)
        self.assertEqual(self.retained.read_text(), 'Retain on any failed import')
        self.assertEqual(sorted(path.name for path in (self.root / 'crates').iterdir()), ['marknoria'])

    def test_failed_rollback_preserves_original_source(self):
        archive = self.archive()
        replace = reader.os.replace
        def fail_after_backup(source, destination):
            if Path(source).name in ('marknoria-reader', 'previous'):
                raise OSError('Synthetic unavailable destination')
            return replace(source, destination)
        with patch.object(reader.os, 'replace', side_effect=fail_after_backup):
            with self.assertRaisesRegex(RuntimeError, 'previous source preserved'):
                reader.install(archive, self.root, self.pin)
        backups = list((self.root / 'crates').glob('.marknoria-import-*/previous/retained.txt'))
        self.assertEqual(len(backups), 1)
        self.assertEqual(backups[0].read_text(), 'Retain on any failed import')

    def test_destination_symlink(self):
        if sys.platform == 'win32':
            self.skipTest('Creating symlinks requires a Windows developer grant')
        archive = self.archive()
        destination = self.root / 'crates/marknoria'
        destination.rename(destination.with_name('retained'))
        destination.symlink_to('retained', target_is_directory=True)
        with self.assertRaisesRegex(ValueError, 'destination'):
            reader.install(archive, self.root, self.pin)
        self.assertTrue(destination.is_symlink())
        self.assertTrue(self.retained.exists())


if __name__ == '__main__':
    unittest.main()
