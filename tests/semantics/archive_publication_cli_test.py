"""Linux integration tests. Synthetic data only; strace injects real syscall failures."""
import io
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import zipfile

repo = Path(__file__).resolve().parents[2]
runtime = str(Path(sys.argv[1]).resolve())
checks = []
evidence = Path(sys.argv[3]) if len(sys.argv) > 3 else None
if evidence:
    evidence.mkdir(parents=True, exist_ok=True)
group = sys.argv[2] if len(sys.argv) > 2 else 'all'
assert group in ('all', 'basic', 'faults', 'close')


def zip_bytes(size=12):
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, 'w', zipfile.ZIP_STORED) as z:
        z.writestr('folder/', b'')
        z.writestr('folder/binary.bin', bytes(range(256)) * (size // 256) + b'x' * (size % 256))
    return buf.getvalue()


with tempfile.TemporaryDirectory(prefix='archive-cli-') as temporary:
    root = Path(temporary)
    source = root / 'input.zip'
    source.write_bytes(zip_bytes())
    original = source.read_bytes()
    sentinel = root / 'unrelated'
    sentinel.write_bytes(b'keep')

    def run(label, dest, *, src=source, inject=None, success=False, trace=False, args=None):
        command = [runtime, 'run', str(repo / 'tools/archive_publish.sapp'), '--allow-all', '--',
                   '--publish', str(src), str(dest)]
        if args is not None:
            command = command[:5] + args
        log = root / (label + '.trace')
        if inject or trace:
            command = ['strace', '-ff', '-qq', '-o', str(log), '-e', 'trace=write,fsync,close,openat,memfd_create'] + (
                sum((['-e', 'inject=' + item] for item in (inject if isinstance(inject, list) else [inject])), []) if inject else []) + command
        p = subprocess.run(command, cwd=repo, capture_output=True, text=True, timeout=45)
        if inject or trace:
            lines = []
            for part in sorted(root.glob(log.name + '.*')):
                tid = part.name.rsplit('.', 1)[1]
                lines.extend(tid + ' ' + line for line in part.read_text().splitlines())
            log.write_text('\n'.join(lines) + '\n')
        assert p.returncode == (0 if success else 1), (label, p.returncode, p.stdout, p.stderr)
        result = json.loads(p.stdout)
        assert result['ok'] == success, (label, result)
        assert str(root) not in p.stdout and 'folder/binary.bin' not in p.stdout, label
        checks.append(label)
        if evidence and log.exists():
            lines = [line for line in log.read_text().splitlines() if any(token in line for token in ('write(', 'fsync(', 'close(', 'INJECTED'))]
            redacted = [re.sub(r'"(?:\\.|[^"\\])*"', '"<redacted>"', line) for line in lines]
            (evidence / (label + '.trace')).write_text('\n'.join(redacted) + '\n')
        return result, log

    if group in ('all', 'basic'):
        dest = root / 'new.zip'
        r, _ = run('success', dest, success=True)
        assert r['publication_confirmed'] and r['durability_verified'] and r['wrapper_handles_closed']
        assert dest.read_bytes() == original and dest.stat().st_mode & 0o777 == 0o600
        with zipfile.ZipFile(dest) as z:
            assert z.testzip() is None and z.read('folder/binary.bin') == b'x' * 12
        inode = dest.stat().st_ino
        run('collision', dest)
        assert dest.stat().st_ino == inode and dest.read_bytes() == original
        run('same_input', source)
        for kind in ['symlink', 'hardlink', 'directory']:
            path = root / (kind + '.zip')
            if kind == 'symlink': path.symlink_to(sentinel)
            elif kind == 'hardlink': os.link(sentinel, path)
            else: path.mkdir()
            before = path.lstat()
            run(kind, path)
            assert path.lstat().st_ino == before.st_ino and sentinel.read_bytes() == b'keep'
        race = root / 'race.zip'
        command = [runtime, 'run', str(repo / 'tools/archive_publish.sapp'), '--allow-all', '--',
                   '--publish', str(source), str(race)]
        writers = [subprocess.Popen(command, cwd=repo, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                    text=True) for _ in range(2)]
        results = [writer.communicate(timeout=45) for writer in writers]
        assert sorted(writer.returncode for writer in writers) == [0, 1], results
        assert race.read_bytes() == original
        checks.append('two_writer_no_replace')
        linked = root / 'linked-parent'
        linked.symlink_to(root, target_is_directory=True)
        r, _ = run('parent_symlink', linked / 'escaped.zip')
        assert not r['publication_attempted'] and not (root / 'escaped.zip').exists()
        run('missing_parent', root / 'missing' / 'out.zip')
        linkinput = root / 'input-link.zip'
        linkinput.symlink_to(source)
        run('input_symlink', root / 'not-made.zip', src=linkinput)
        run('explicit_opt_in', root / 'not-made.zip', args=[str(source), str(dest)])
        for label, data in [('malformed', b'not zip'), ('truncated', original[:-1]),
                            ('oversize', b'x' * 4194305)]:
            bad = root / (label + '.input')
            bad.write_bytes(data)
            out = root / (label + '.zip')
            r, log = run(label, out, src=bad, trace=True)
            assert not r['publication_attempted'] and not out.exists()
            assert '"archive-publication"' not in log.read_text() and 'O_TMPFILE' not in log.read_text()
    if group in ('all', 'faults'):
        for label, index, exists in [('before_fsync', 1, False), ('after_fsync', 2, True)]:
            out = root / (label + '.zip')
            r, log = run(label, out, inject=f'fsync:error=EIO:when={index}')
            assert 'INJECTED' in log.read_text() and out.exists() == exists
            assert r['output_may_exist'] and not r['partial_output_possible']
            if exists:
                assert out.read_bytes() == original and r['written'] and r['publication_confirmed']
                assert not r['durability_verified']
            else:
                assert 'written' not in r
        # Determine the write ordinal from an actual large successful publication.
        large = root / 'large.input'
        large.write_bytes(zip_bytes(70000))
        _, log = run('large_trace', root / 'large.zip', src=large, success=True, trace=True)
        lines = log.read_text().splitlines()
        chunks = [line for line in lines if re.search(r'write\(.*?, 65536\) = 65536$', line)]
        assert chunks, lines
        tid = chunks[-1].split()[0]
        writes = [line for line in lines if line.startswith(tid + ' ') and 'write(' in line]
        chunk_index = writes.index(chunks[-1]) + 1
        # The next write belongs to the second anonymous output chunk: the first
        # 65536 bytes have really been written before ENOSPC stops completion.
        ordinal = chunk_index + 1
        partial = root / 'partial.zip'
        r, log = run('partial_write', partial, src=large, inject=f'write:error=ENOSPC:when={ordinal}')
        trace_text = log.read_text()
        assert 'INJECTED' in trace_text and re.search(r', 65536\) = 65536', trace_text)
        assert not partial.exists() and not r['partial_output_possible'] and 'written' not in r
    if group in ('all', 'close'):
        _, log = run('close_control', root / 'close-control.zip', success=True, trace=True)
        lines = log.read_text().splitlines()
        synced = [i for i, line in enumerate(lines) if re.search(r'fsync\(.*\)\s+= 0$', line)]
        assert len(synced) == 2
        tid = lines[synced[1]].split()[0]
        next_close = next(i for i in range(synced[1] + 1, len(lines))
                          if lines[i].startswith(tid + ' ') and 'close(' in lines[i])
        ordinal = sum(line.startswith(tid + ' ') and 'close(' in line for line in lines[:next_close + 1])
        for label, injections in [('internal_close', [f'close:error=EIO:when={ordinal}']),
                                  ('fsync_internal_close', ['fsync:error=EIO:when=2', f'close:error=EIO:when={ordinal}'])]:
            out = root / (label + '.zip')
            r, trace = run(label, out, inject=injections)
            assert out.read_bytes() == original and not r['publication_confirmed']
            assert r['output_may_exist'] and 'written' not in r and r['wrapper_handles_closed']
            assert 'INJECTED' in trace.read_text()
    assert source.read_bytes() == original and sentinel.read_bytes() == b'keep'
    assert not any(p.name.startswith('.pblang-stage-') for p in root.iterdir())
print(json.dumps({'passed': checks, 'count': len(checks), 'partial_write_after_bytes': 65536 if 'partial_write' in checks else None}))
