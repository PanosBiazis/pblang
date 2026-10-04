"""Package this trusted example with PBLang's Linux source-folder packager."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile


# This example supports literal imports only. Skip comments and strings rather
# than treating documentation examples as dependencies; reject other import forms.
TOKENS = re.compile(
    r'//[^\n]*|/\*.*?\*/|"(?:\\.|[^"\\])*"'
    r'|\bimport\s*\(\s*"(?P<path>[^"\\]+)"\s*\)|(?P<unsupported>\bimport\b)',
    re.S,
)
ENTRY = Path('examples/declarative_browser/main.sapp')


def sources(repo, entry=ENTRY):
    pending, captured = [entry], {}
    while pending:
        relative = pending.pop()
        if relative in captured:
            continue
        path = repo / relative
        if path.is_symlink() or not path.is_file():
            raise ValueError(f'Expected a regular module: {relative}')
        data = path.read_bytes()
        if len(data) > 1048576 or len(captured) >= 128:
            raise ValueError('Reader module budget exceeded')
        captured[relative] = data
        for token in TOKENS.finditer(data.decode('utf-8')):
            if token.group('unsupported'):
                raise ValueError(f'Only literal import("path") is packaged: {relative}')
            name = token.group('path')
            if name is None:
                continue
            target = (repo / 'stdlib' / name[7:] if name.startswith('stdlib:') else
                      repo / name if name.startswith('stdlib/') else path.parent / name)
            # Reject links and escapes before normalizing; this is a trusted
            # checkout helper, not a sandbox for concurrently hostile sources.
            if target.is_symlink():
                raise ValueError(f'Linked module is unsupported: {name}')
            target = target.resolve().relative_to(repo)
            if target.suffix != '.sapp' or not (target.parts[0] == 'stdlib' or
                    target.is_relative_to(entry.parent)):
                raise ValueError(f'Module outside reader profile: {target}')
            pending.append(target)
    return captured


def tree_digest(root):
    digest = hashlib.sha256(b'pblang-package-tree-v2\0')
    for path in sorted(root.rglob('*'), key=lambda p: p.relative_to(root).as_posix().encode()):
        name = path.relative_to(root).as_posix().encode()
        data = path.read_bytes() if path.is_file() else b''
        digest.update(b'f' if path.is_file() else b'd')
        digest.update(len(name).to_bytes(8, 'big')); digest.update(name)
        digest.update(len(data).to_bytes(8, 'big')); digest.update(data)
    return digest.hexdigest()


def metadata(root, entry=ENTRY):
    manifest = ('[package]\nname="pb-document-reader"\nversion="0.1.0"\n'
                f'entry="{entry.as_posix()}"\n[dependencies]\nstdlib={{path="stdlib"}}\n')
    (root / 'pblang.toml').write_text(manifest)
    (root / 'pblang.lock').write_text(
        f'manifest_sha256 = "{hashlib.sha256(manifest.encode()).hexdigest()}"\n'
        'content_hash_algorithm = "pblang-package-tree-v2"\n[dependencies.stdlib]\n'
        f'path = "stdlib"\nsha256 = "{tree_digest(root / "stdlib")}"\n')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--runtime', required=True, type=Path, help='Trusted regular Linux x64 runtime')
    parser.add_argument('--output', required=True, type=Path, help='Absent destination in an existing private (0700) parent')
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[2]
    output = args.output.absolute()
    runtime = args.runtime.absolute()
    if os.path.lexists(output):
        parser.error('Output must be absent; existing packages are never replaced')
    if output.parent.is_symlink() or not output.parent.is_dir() or output.parent.stat().st_mode & 0o777 != 0o700:
        parser.error('Output parent must already exist with mode 0700')
    if output.resolve().is_relative_to(repo) or repo.is_relative_to(output.resolve()):
        parser.error('Output must be outside the source checkout')
    captured = sources(repo)
    # Only this freshly created temporary stage is cleaned up. The packaging
    # library owns destination admission/publication; partial outputs are retained.
    with tempfile.TemporaryDirectory(prefix='reader-stage-', dir=output.parent) as temporary:
        stage = Path(temporary) / 'source'
        tool = Path(temporary) / 'tool'
        tool_entry = Path('tools/pbpackage.sapp')
        for root, modules, entry in ((stage,captured,ENTRY),(tool,sources(repo,tool_entry),tool_entry)):
            for relative, data in modules.items():
                target = root / relative
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(data)
            metadata(root,entry)
        (stage / 'run-reader.sh').write_bytes((repo / 'examples/declarative_browser/run-reader.sh').read_bytes())
        (stage / 'LICENSE').write_bytes((repo / 'LICENSE').read_bytes())
        (stage / 'README.txt').write_text(
            'PBLang offline document reader — Linux x86-64 candidate\n\n'
            'Open: sh /path/to/this/folder/run-reader.sh\n'
            'Check without a display: sh /path/to/this/folder/run-reader.sh --check\n'
            'The GUI needs a compatible Linux desktop, native libraries and system fonts.\n'
            'This folder includes readable application/library source and the selected runtime.\n'
            'It is not a global installation or a sandbox. No activation has occurred.\n'
            'package-receipt.json records publication, not acceptance.\n')
        inventory = {p.as_posix(): hashlib.sha256(data).hexdigest() for p, data in sorted(captured.items())}
        (stage / 'reader-source-sha256.json').write_text(json.dumps(inventory, indent=2) + '\n')
        run = subprocess.run([str(runtime), 'run', str(tool / tool_entry),
                              '--allow-all', '--', str(stage), str(output), '--runtime', str(runtime)],
                             cwd=tool, timeout=120)
        if run.returncode:
            raise SystemExit(run.returncode)
    print(f'Launch: sh {output / "run-reader.sh"}')
    print('This is a candidate folder, not an installed or activated product.')


if __name__ == '__main__':
    main()
