"""Linux source-admission qualification for a prepared smart-check release.

Only synthetic fixtures are writable; the source checkout is not mounted and
networking is isolated. This does not install or modify the prepared release.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import tempfile

import check_cli_exact_argv as process_checks

MIB = 1048576


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--release', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--only', help='One exact case name, recorded in the report')
    args = parser.parse_args()
    if args.report.exists():
        parser.error('report already exists')
    release = args.release.resolve()
    binary = release / 'bin/pblang'
    driver = release / 'stdlib/check_cli.sapp'
    assert binary.is_file() and driver.is_file()
    process_checks.CHILD_SECONDS = 90
    rows = []
    report = {'passed': False, 'rows': rows, 'selected_case': args.only,
              'source_hidden': True, 'network_isolated': True,
              'per_process_deadline_seconds': 90, 'memory_limit_bytes': 768 * MIB,
              'memory_limit_is_not_measured_peak': True,
              'captured_stream_limit_bytes': process_checks.OUTPUT_CAP,
              'driver_sha256': hashlib.sha256(driver.read_bytes()).hexdigest(),
              'native_sha256': hashlib.sha256((release / 'bin/pblang-native').read_bytes()).hexdigest()}
    def save():
        args.report.write_text(json.dumps(report, indent=2, ensure_ascii=False) + '\n')
        args.report.chmod(0o600)

    with tempfile.TemporaryDirectory(prefix='bounded-smart-') as tmp:
        root = Path(tmp)
        good = b'let answer=42; print answer;\n'
        fixtures = {'valid.sapp': good, 'malformed.sapp': b'let x=;',
                    'utf8.sapp': b'print "\xff";', 'empty.sapp': b'',
                    'line\nname.sapp ': good, 'root': good, 'TRACE_CANARY': good,
                    '--bounded-source': good,
                    'effect.sapp': b'fs_write("/scratch/EFFECT", "must not execute");',
                    'lazy.sapp': b'fn load(){return import("private-provider");} export load;'}
        # A single padded comment has tiny AST size, avoiding a token-flood test.
        for count, name in [(MIB, 'exact.sapp'), (MIB + 1, 'over.sapp'),
                            (MIB - 1, 'short.sapp')]:
            fixtures[name] = good + b'//' + b'x' * (count - len(good) - 3) + b'\n'
        for name, data in fixtures.items():
            (root / name).write_bytes(data)
        (root / 'link.sapp').symlink_to('valid.sapp')
        os.mkfifo(root / 'fifo')
        before = {name: hashlib.sha256(data).hexdigest() for name, data in fixtures.items()}
        report['fixture_hashes'] = before
        box = ['bwrap', '--unshare-all', '--die-with-parent', '--clearenv']
        for directory in ['/usr', '/bin', '/lib', '/lib64']:
            box += ['--ro-bind', directory, directory]
        box += ['--proc', '/proc', '--dev', '/dev', '--tmpfs', '/tmp',
                '--ro-bind', str(release), str(release), '--bind', str(root), '/scratch',
                '--chdir', str(release), '--setenv', 'PATH', '/usr/bin:/bin',
                '--setenv', 'LANG', 'C.UTF-8']
        def case(name, files, code=0, contains='no issues found', *, flags=None,
                 profile='', public=False, grant=True, trace_absent=None, trace_copy=None, trace_present=None):
            if args.only and args.only != name:
                return
            files = [name if name.startswith('/') else '/scratch/' + name for name in files]
            direct = ['run', str(driver)]
            direct += ['--allow-all'] if grant else ['--deny-ffi', '--deny-exec']
            direct += ['--', *(flags if flags is not None else ['--bounded-source']), *files]
            command = ['check', *files] if public else direct
            traced = trace_absent is not None or trace_copy is not None or trace_present is not None
            trace_name = f'trace-{len(rows)}.log'
            prefix = ['strace', '-f', '-yy', '-s', '256', '-e', 'trace=openat,sendfile',
                      '-o', '/scratch/' + trace_name] if traced else []
            result = process_checks.bounded_process([*box, '--setenv', 'PBLANG_CHECK_PROFILE', profile,
                                                     *prefix, str(binary), *command])
            text = result['stdout'] + result['stderr']
            checks = {'exit': result['exit'] == code, 'diagnostic': contains in text,
                      'bounded_process': result['failure'] is None,
                      'no_descendants': not result['group_survived_normal_exit'],
                      'output_budget': len(result['stdout'].encode()) <= 16384,
                      'fixtures_unchanged': all(hashlib.sha256((root / n).read_bytes()).hexdigest() == h
                                                for n, h in before.items()),
                      'source_not_executed': not (root / 'EFFECT').exists()}
            if traced:
                trace = (root / trace_name).read_text()
                trace_dir = args.report.with_suffix('.traces')
                trace_dir.mkdir(mode=0o700, exist_ok=True)
                trace_output = trace_dir / trace_name
                with trace_output.open('x') as saved_trace:
                    saved_trace.write(trace)
                trace_output.chmod(0o600)
                result['trace_file'] = str(trace_output)
                if trace_present is not None:
                    checks['trace_positive_control'] = trace_present in trace
                if trace_absent is not None:
                    checks['no_source_open_or_copy'] = trace_absent not in trace
                if trace_copy is not None:
                    copies = [line for line in trace.splitlines() if 'sendfile(' in line and trace_copy in line]
                    checks['bounded_copy'] = len(copies) == 1 and f', {MIB + 1})' in copies[0]
                result['trace_sha256'] = hashlib.sha256(trace.encode()).hexdigest()
                result['source_copy_lines'] = [line for line in trace.splitlines()
                                               if 'sendfile(' in line and '/scratch/' in line]
            rows.append(dict(name=name, checks=checks, passed=all(checks.values()), **result))
            save()
            print(name, rows[-1]['passed'], flush=True)
            if result['failure'] or result['group_survived_normal_exit']:
                raise RuntimeError('process bound or cleanup failed; remaining cases not run')

        case('direct_valid', ['valid.sapp'])
        case('public_profile', ['/scratch/valid.sapp'], public=True, profile='bounded-source')
        case('malformed', ['malformed.sapp'], 1, 'parse-error')
        case('strict_utf8', ['utf8.sapp'], 1, 'source admission rejected')
        case('empty_diagnosed', ['empty.sapp'], 1, 'source is empty')
        case('exact_filename', ['line\nname.sapp '])
        case('literal_root', ['root'])
        case('literal_option_filename', ['./--bounded-source'])
        case('symlink_refused', ['link.sapp'], 1, 'source admission rejected')
        case('fifo_refused', ['fifo'], 1, 'source admission rejected')
        case('no_implicit_grant', ['valid.sapp'], 1, 'source admission rejected', grant=False)
        case('source_not_executed', ['effect.sapp'])
        case('combined_origin', ['lazy.sapp'], 1, 'import-origin-deferred',
             flags=['--bounded-source', '--import-origin'])
        case('duplicate_flag', ['valid.sapp'], 1, 'duplicate bounded source option',
             flags=['--bounded-source', '--bounded-source'])
        case('default_unchanged', ['valid.sapp'], flags=[], grant=False)
        case('unknown_profile', ['valid.sapp'], 1, 'unknown source profile', profile='unknown')
        case('filecount_64', ['valid.sapp'] * 64, contains='checked 64 file(s): 0 error(s)')
        # Unique canary proves no requested source is opened for an invalid count.
        case('trace_existing_control', ['TRACE_CANARY'], trace_present='TRACE_CANARY')
        case('trace_missing_control', ['/scratch/MISSING_CANARY'], 1, 'source admission rejected',
             trace_present='MISSING_CANARY')
        case('filecount_65_before_read', ['/scratch/COUNT_CANARY'] * 65, 1,
             'at most 64 files', trace_absent='COUNT_CANARY')
        case('exact_per_file', ['exact.sapp'])
        case('one_over_per_file', ['over.sapp'], 1, 'source admission rejected', trace_copy='over.sapp')
        case('aggregate_exact', ['exact.sapp'] * 4, contains='checked 4 file(s): 0 error(s)')
        case('aggregate_no_further_read', ['exact.sapp'] * 4 + ['/scratch/TOTAL_CANARY'], 1,
             'aggregate source byte budget exhausted', trace_absent='TOTAL_CANARY')
        case('aggregate_remaining_one', ['exact.sapp'] * 3 + ['short.sapp', 'valid.sapp'], 1,
             'source admission rejected')
        case('failed_reads_charge_allowance', ['over.sapp'] * 4 + ['/scratch/FAILED_CANARY'], 1,
             'aggregate source byte budget exhausted', trace_absent='FAILED_CANARY')
    report['passed'] = bool(rows) and all(row['passed'] for row in rows)
    save()
    raise SystemExit(0 if report['passed'] else 1)


if __name__ == '__main__':
    main()
