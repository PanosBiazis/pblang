"""Whole-ZIP verification precedes output effects, including unselected members."""
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import zipfile

repo = Path(__file__).resolve().parents[2]
runtime = str(Path(sys.argv[1]).resolve())
checks = []

def archive(rows, method=zipfile.ZIP_STORED):
    b = io.BytesIO()
    with zipfile.ZipFile(b, 'w', method) as z:
        for name, data in rows: z.writestr(name, data)
    return b.getvalue()

with tempfile.TemporaryDirectory(prefix='archive-selection-') as tmp:
    root = Path(tmp)
    def run(label, data, name='chosen', ok=False):
        src, dst = root / 'input.zip', root / label
        src.write_bytes(data)
        command = [runtime, 'run', str(repo/'tools/archive_extract.sapp'), '--allow-all', '--', '--extract', str(src), name, str(dst)]
        p = subprocess.run(command, cwd=repo, capture_output=True, text=True, timeout=45)
        assert p.returncode == (0 if ok else 1), (label, p.stdout, p.stderr)
        r = json.loads(p.stdout)
        assert r['ok'] == ok and str(root) not in p.stdout and 'chosen' not in p.stdout
        if not ok: assert not dst.exists() and not r['publication_attempted'], (label, r)
        checks.append(label)
        return dst, r

    for method in [zipfile.ZIP_STORED, zipfile.ZIP_DEFLATED]:
        data = archive([('chosen', bytes(range(256))*274), ('later', b'valid')], method)
        dst, _ = run('full-'+str(method), data, ok=True)
        assert dst.read_bytes() == bytes(range(256))*274 and len(dst.read_bytes()) > 65536
    good = archive([('chosen', b'first'), ('later', b'secret')])
    corrupt = bytearray(good)
    with zipfile.ZipFile(io.BytesIO(good)) as z:
        offset = z.getinfo('later').header_offset
    corrupt[offset+30+len('later')] ^= 1
    run('unselected-crc', corrupt)
    for label, name in [('traversal', '../escape'), ('absolute', '/escape'), ('backslash', 'a\\b'), ('case-collision', 'CHOSEN'), ('file-ancestor', 'chosen/child')]:
        run(label, archive([('chosen', b'first'), (name, b'bad')]))
    link = zipfile.ZipInfo('link')
    link.create_system = 3
    link.external_attr = 0o120777 << 16
    run('symlink-member', archive([('chosen', b'first'), (link, b'target')]))
    encrypted = bytearray(good)
    with zipfile.ZipFile(io.BytesIO(good)) as z:
        for row in z.infolist(): encrypted[row.header_offset+6] |= 1
    cursor = good.index(b'PK\x01\x02')
    while encrypted[cursor:cursor+4] == b'PK\x01\x02':
        encrypted[cursor+8] |= 1
        cursor += 46 + int.from_bytes(encrypted[cursor+28:cursor+30], 'little')
    run('encryption-flags', encrypted)
    run('missing', good, name='absent')
    dst, _ = run('empty', archive([('chosen', b'')]), ok=True)
    assert dst.read_bytes() == b''
    run('empty-deflate', archive([('chosen', b'')], zipfile.ZIP_DEFLATED))
    run('directory', archive([('chosen/', b'')]), name='chosen/')
    run('expanded-limit', archive([('chosen', b'x'*1048577)], zipfile.ZIP_DEFLATED))
    run('entry-limit', archive([('chosen', b'x')]+[(f'f{i}',b'') for i in range(256)]))
print(json.dumps({'passed': checks, 'count': len(checks)}))
