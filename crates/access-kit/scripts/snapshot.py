#!/usr/bin/env python3
"""Create/verify an immutable access-kit source snapshot; no network or secret input."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil

ROOT = Path(__file__).resolve().parent.parent
SELECT = ['.gitignore','Cargo.toml','LICENSE','README.md','INTERFACES.md','VERIFICATION.md','src','migrations','tests','examples','scripts']

def source_files(root):
    result = []
    for name in SELECT:
        path = root / name
        if path.is_symlink():
            raise ValueError("Symlinks are forbidden in snapshots")
        candidates = [path] if path.is_file() else list(path.rglob('*'))
        for candidate in candidates:
            if candidate.is_symlink():
                raise ValueError('Symlinks are forbidden in snapshots')
            if candidate.is_file() and '__pycache__' not in candidate.parts and candidate.suffix != '.pyc':
                if not candidate.resolve().is_relative_to(root.resolve()):
                    raise ValueError('Snapshot path escaped root')
                result.append(candidate)
    return sorted(result, key=lambda p: p.relative_to(root).as_posix())

def manifest(root):
    files = {p.relative_to(root).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest() for p in source_files(root)}
    source = ''.join(f'{path}:{digest}\n' for path,digest in files.items()).encode()
    return {'package':'access-kit','version':'0.1.0','schema_version':max(int(Path(p).name.split('_',1)[0]) for p in files if p.startswith('migrations/') and p.endswith('.sql')),'source_sha256':hashlib.sha256(source).hexdigest(),'schema_sha256':hashlib.sha256(b''.join((root / p).read_bytes() for p in files if p.startswith('migrations/') and p.endswith('.sql'))).hexdigest(),'files':files}

def check(root):
    expected = json.loads((root / 'SNAPSHOT.json').read_text())
    actual = manifest(root)
    if actual != expected:
        raise ValueError('Snapshot content/hash mismatch')
    return actual

def destination_path(destination):
    if not destination.is_absolute() or destination.name != 'access-kit' or destination.parent.name != 'crates':
        raise ValueError('Destination must be an absolute host/crates/access-kit path')
    # Validate the lexical path before resolving; resolve() erases symlink evidence.
    if any(path.is_symlink() for path in (destination, *destination.parents)):
        raise ValueError('Destination and its ancestors must not be symlinks; use a physical path')
    return destination.resolve()

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--write',action='store_true')
    parser.add_argument('--check',action='store_true')
    parser.add_argument('--destination',type=Path)
    args = parser.parse_args()
    if args.write:
        (ROOT / 'SNAPSHOT.json').write_text(json.dumps(manifest(ROOT),indent=2,sort_keys=True)+'\n')
    result = check(ROOT)
    if args.destination:
        target = destination_path(args.destination)
        if not args.destination.is_absolute() or target.name != "access-kit" or target.parent.name != "crates":
            raise ValueError("Destination must be an absolute host/crates/access-kit path")
        if target == ROOT or target.is_relative_to(ROOT):
            raise ValueError('Destination cannot be authoring root or its child')
        if target.exists():
            if target.is_symlink():
                raise ValueError('Destination symlink forbidden')
            if (target / ".git").exists():
                raise ValueError("Destination must not contain a git repository")
            allowed_top=set(SELECT + ["SNAPSHOT.json","Cargo.lock","target","__pycache__"])
            if any(p.name not in allowed_top for p in target.iterdir()):
                raise ValueError("Destination contains unrecognized files")
            check(target)  # Refuse to erase modified or untracked draft source.
            shutil.rmtree(target)
        target.mkdir(parents=True)
        for path in source_files(ROOT):
            destination = target / path.relative_to(ROOT)
            destination.parent.mkdir(parents=True,exist_ok=True)
            shutil.copyfile(path,destination)
        shutil.copyfile(ROOT / 'SNAPSHOT.json',target / 'SNAPSHOT.json')
        check(target)
    print(f"PASS {result['source_sha256']} schema {result['schema_sha256']}")

if __name__ == '__main__':
    main()
