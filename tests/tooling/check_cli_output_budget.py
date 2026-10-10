"""Finite real-process CLI output-budget regressions; no source execution.

Run under the caller's shared test lock with an installed-shaped release.
The existing capture helper enforces streaming output/time/address-space caps.
No production profile or installed pointer is modified by this test.
"""
from pathlib import Path
import argparse
import json
import os
import tempfile
import time
from check_cli_exact_argv import bounded_process, inventory


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--release', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--baseline', action='store_true')
    options = parser.parse_args()
    release = options.release.resolve()
    rows = []
    report = dict(passed=False, baseline=options.baseline, rows=rows,
                  source_hidden=True, network_isolated=True, stdout_limit=16384,
                  capture_limit=262144, child_seconds=15, suite_seconds=150)
    start = time.monotonic()
    try:
        with tempfile.TemporaryDirectory(prefix='cli-budget-') as directory:
            scratch = Path(directory)
            fixtures = {
                'valid.sapp': 'let answer=42; print answer;',
                'invalid.sapp': 'let answer=;',
                'warnings.sapp': '\n'.join('let unused_'+str(i)+'x'*700+'=1;' for i in range(24)),
                'effect.sapp': 'fs_write("/scratch/UNEXPECTED", "SOURCE_EXECUTED");',
                'nested.sapp': 'for _i 1 {let p=map{run:fn(self){return 7;}};print (try {p:run()} catch 9);}',
                'lazy.sapp': 'fn load(){return import("PRIVATE_CANARY");} export load;',
            }
            for name, text in fixtures.items():
                (scratch/name).write_text(text)
            before = inventory(scratch)
            box = ['bwrap', '--unshare-all', '--die-with-parent', '--clearenv']
            for path in ['/usr', '/bin', '/lib', '/lib64']:
                box += ['--ro-bind', path, path]
            box += ['--proc', '/proc', '--dev', '/dev', '--tmpfs', '/tmp',
                    '--ro-bind', str(release), str(release), '--bind', str(scratch), '/scratch',
                    '--setenv', 'PATH', '/usr/bin:/bin', '--setenv', 'LANG', 'C.UTF-8',
                    '--chdir', '/scratch']
            for route in ['public', 'direct']:
                prefix = [str(release/'bin/pblang'), 'check'] if route == 'public' else [
                    str(release/'bin/pblang'), 'run', str(release/'stdlib/check_cli.sapp'),
                    '--allow-all', '--']
                missing = ['/scratch/missing-'+str(i)+'x'*225+'.sapp' for i in range(96)]
                cases = [
                    ('missing batch', missing, 1, 'checked 96 file(s): 96 error(s), 0 warning(s)', True),
                    ('late valid', ['/scratch/warnings.sapp', '/scratch/valid.sapp'], 0, 'checked 2 file(s): 0 error(s), 24 warning(s)', True),
                    ('late invalid', ['/scratch/warnings.sapp', '/scratch/invalid.sapp'], 1, 'checked 2 file(s): 1 error(s), 25 warning(s)', True),
                    ('small valid', ['/scratch/valid.sapp'], 0, '0 error(s)', False),
                    ('small invalid', ['/scratch/invalid.sapp'], 1, '1 error(s), 1 warning(s)', False),
                    ('not executed', ['/scratch/effect.sapp'], 0, '0 error(s)', False),
                    ('retained rejection', ['--retained-source', '/scratch/nested.sapp'], 1, '1 error(s), 0 warning(s)', False),
                    ('origin rejection', ['--import-origin', '/scratch/lazy.sapp'], 1, '1 error(s), 0 warning(s)', False),
                ]
                # Native public CLI filters hyphen options; named profile flags
                # are therefore verified through the actual direct driver route.
                if route == 'public':
                    cases = cases[:6]
                for name, args, status, expected, truncated in cases:
                    if time.monotonic()-start > 135:
                        raise TimeoutError('suite deadline')
                    result = bounded_process(box+prefix+args)
                    result['name'] = route+' '+name
                    result['stdout_bytes'] = len(result['stdout'].encode())
                    summary = result['stdout'].splitlines()[-1] if result['stdout'].splitlines() else ''
                    result['final_summary'] = summary
                    passed = result['exit'] == status and (summary == expected if name in ['missing batch', 'late valid', 'late invalid'] else expected in summary)
                    passed &= '\ufffd' not in result['stdout']  # synthetic fixtures contain no replacement scalar
                    passed &= result['failure'] is None and not result['group_survived_normal_exit']
                    marker = '[diagnostic output truncated; final counts remain complete]'
                    if not options.baseline:
                        passed &= result['stdout_bytes'] <= 16384
                        passed &= (marker in result['stdout']) == truncated
                        passed &= result['stdout'].count(marker) <= 1
                    elif truncated:
                        passed &= result['stdout_bytes'] > 16384
                    if name == 'late invalid':
                        summary = result['stdout'].splitlines()[-1]
                        passed &= '0 error(s)' not in summary
                    result['passed'] = passed
                    rows.append(result)
                    if result['failure'] or result['group_survived_normal_exit']:
                        raise RuntimeError('resource/isolation failure')
            report['fixtures_unchanged'] = inventory(scratch) == before
            report['source_not_executed'] = not (scratch/'UNEXPECTED').exists()
            report['fixture_hashes'] = before
            report['passed'] = all(r['passed'] for r in rows) and report['fixtures_unchanged'] and report['source_not_executed']
    finally:
        report['seconds'] = time.monotonic()-start
        options.report.write_text(json.dumps(report, ensure_ascii=False, indent=2)+'\n')
        options.report.chmod(0o600)
    print(json.dumps(dict(passed=report['passed'], cases=len(rows))))
    raise SystemExit(0 if report['passed'] else 1)


if __name__ == '__main__':
    main()
