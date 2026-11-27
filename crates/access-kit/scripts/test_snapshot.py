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
            self.assertEqual(first['schema_version'],1)
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

    def test_destination_symlink_and_parent_are_rejected_without_mutation(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp).resolve();target=root/'real';target.mkdir()
            sentinel=target/'keep';sentinel.write_text('build artifact')
            crates=root/'host/crates';crates.mkdir(parents=True)
            destination=crates/'access-kit';destination.symlink_to(target,target_is_directory=True)
            with self.assertRaises(ValueError):snapshot.destination_path(destination)
            self.assertEqual(sentinel.read_text(),'build artifact')
            destination.unlink();crates.rmdir();crates.symlink_to(target,target_is_directory=True)
            with self.assertRaises(ValueError):snapshot.destination_path(crates/'access-kit')
            self.assertEqual(sentinel.read_text(),'build artifact')

    def test_additive_schema_upgrade_keeps_previous_snapshot_verifiable(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);(root/'src').mkdir();(root/'migrations').mkdir()
            (root/'src/lib.rs').write_text('source')
            (root/'migrations/0001_access.sql').write_text('base')
            old=snapshot.manifest(root);(root/'SNAPSHOT.json').write_text(json.dumps(old))
            self.assertEqual(snapshot.check(root)['schema_version'],1)
            (root/'migrations/0002_client_registration.sql').write_text('upgrade')
            new=snapshot.manifest(root)
            self.assertEqual(new['schema_version'],2)
            self.assertNotEqual(old['schema_sha256'],new['schema_sha256'])

if __name__ == '__main__':unittest.main()
