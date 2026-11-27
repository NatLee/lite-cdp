"""Snapshot integrity tests independent from any host worktree."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('snapshot',Path(__file__).with_name('snapshot.py'))
snapshot = importlib.util.module_from_spec(spec)
spec.loader.exec_module(snapshot)

class SnapshotTests(unittest.TestCase):
    def test_deterministic_hash_detects_source_changes(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp)
            (root/'src').mkdir();(root/'migrations').mkdir()
            (root/'src/lib.rs').write_text('original')
            (root/'migrations/0001_access.sql').write_text('schema')
            first=snapshot.manifest(root)
            self.assertEqual(first,snapshot.manifest(root))
            (root/'SNAPSHOT.json').write_text(json.dumps(first))
            self.assertEqual(snapshot.check(root),first)
            (root/'src/lib.rs').write_text('changed')
            with self.assertRaises(ValueError):snapshot.check(root)
    def test_symlink_source_is_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);(root/'src').mkdir()
            (root/'outside').write_text('private')
            (root/'src/escape').symlink_to(root/'outside')
            with self.assertRaises(ValueError):snapshot.source_files(root)

    def test_directory_symlink_is_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);(root/'outside').mkdir()
            (root/'src').symlink_to(root/'outside',target_is_directory=True)
            with self.assertRaises(ValueError):snapshot.source_files(root)

if __name__ == '__main__':unittest.main()
