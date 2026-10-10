"""Linux source-hidden differential qualification of the private cache adapter.

Run only against prepared releases. Fixtures and all write effects are disposable;
no network namespace, developer checkout, home, or global installation is exposed.
"""
import argparse
import hashlib
import json
import os
import re
from pathlib import Path
import subprocess
import tempfile
import tomllib

SYMBOLS = ['free', 'close', 'dup', 'strdup', 'openat', 'mkdirat', 'unlinkat',
           'renameat', 'fchmod', 'fsync', 'memfd_create', 'sendfile', 'lseek', 'getpid', 'getuid']


BACKEND_FIXTURE = r"""
let state=map{mode:"",closes:0,created:false};
fn configure(mode){state.mode=mode;}
fn open(path,create=false,protect=false){
 if path=="new" and not state.created{return map{ok:false};}
 return map{ok:true,path:path};
}
fn open_absolute(path,create=false,protect=false){return open(path,create,protect);}
fn private_root_status(fs){if state.mode=="policy-throw"{throw "injected-primary-policy";}return map{ok:true};}
fn mkdir_new(fs,name,create=false){if state.mode=="mkdir-throw"{throw "injected-primary-mkdir";}state.created=true;return map{ok:true};}
fn close(fs){state.closes=state.closes+1;print "close:"+str(state.closes);
 if state.mode=="close-throw" or str_ends_with(state.mode,"close-throw"){throw "injected-close";}
 if state.mode=="close-nil"{return nil;}if state.mode=="close-number"{return 1;}
 if state.mode=="close-false" or (str_ends_with(state.mode,"close-false") and state.mode!="first-close-false") or (state.mode=="first-close-false" and state.closes==1){return false;}return true;}
fn read_bytes(fs,name,limit){if state.mode=="read-throw"{throw "injected-primary-read";}if str_starts_with(state.mode,"read-error-"){return map{ok:false,error:"injected-known-primary",detail:7};}return map{ok:true,bytes:[65],size:1};}
fn write_atomic(fs,name,bytes,create=false){if state.mode=="write-throw"{throw "injected-primary-write";}if str_starts_with(state.mode,"write-error-"){return map{ok:false,error:"injected-known-primary",detail:7};}return map{ok:true};}
fn remove(fs,name){if state.mode=="remove-throw"{throw "injected-primary-remove";}if str_starts_with(state.mode,"remove-error-"){return map{ok:false,error:"injected-known-primary",detail:7};}return map{ok:true};}
export configure,open,open_absolute,private_root_status,mkdir_new,close,read_bytes,write_atomic,remove;
"""

def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--candidate', type=Path, required=True)
    ap.add_argument('--reference', type=Path, required=True)
    ap.add_argument('--report', type=Path, required=True)
    ap.add_argument('--group', choices=['parity', 'denials', 'integration', 'faults'], required=True)
    args = ap.parse_args()
    rows = []
    source_root = Path(__file__).resolve().parents[2]
    libc = next(p for p in ['/usr/lib/x86_64-linux-gnu/libc.so.6',
                          '/lib/x86_64-linux-gnu/libc.so.6',
                          '/usr/lib/aarch64-linux-gnu/libc.so.6'] if Path(p).is_file())

    def record(label, passed, **details):
        rows.append(dict(label=label, passed=bool(passed), **details))
        args.report.write_text(json.dumps(rows, indent=2, ensure_ascii=False) + '\n')
        print(label, bool(passed), flush=True)
        assert passed, rows[-1]

    def run(release, source, omit=None, facade=None, backend=None):
        release = release.resolve()
        with tempfile.TemporaryDirectory(prefix='pblang-cache-io-') as tmp:
            root = Path(tmp)
            (root/'private').mkdir(mode=0o700)
            (root/'private/folder').mkdir(mode=0o700)
            (root/'shared').mkdir(mode=0o755)
            (root/'shared').chmod(0o755)
            (root/'outside').write_text('outside')
            os.link(root/'outside', root/'private/hardlink')
            (root/'private/final-link').symlink_to('../outside')
            (root/'parent-link').symlink_to('private')
            os.mkfifo(root/'private/fifo')
            (root/'stdlib').symlink_to(release/'stdlib', target_is_directory=True)
            manifest = ('[package]\nname="cache-io-test"\nversion="1.0.0"\nentry="main.sapp"\n'
                        '[dependencies]\nstdlib={path='+json.dumps(str(release/'stdlib'))+'}\n')
            (root/'pblang.toml').write_text(manifest)
            dep = tomllib.loads((release/'pblang.lock').read_text())['dependencies']['stdlib']
            (root/'pblang.lock').write_text('manifest_sha256='+json.dumps(hashlib.sha256(manifest.encode()).hexdigest())
                +'\ncontent_hash_algorithm="pblang-package-tree-v2"\n[dependencies.stdlib]\npath='
                +json.dumps(str(release/'stdlib'))+'\nsha256='+json.dumps(dep['sha256'])+'\n')
            (root/'main.sapp').write_text(source)
            if facade is not None:
                (root/'facade.sapp').write_text(facade)
                (root/'backend.sapp').write_text(backend)
            box = ['bwrap', '--unshare-all', '--die-with-parent', '--clearenv']
            for d in ['/usr', '/bin', '/lib', '/lib64']:
                box += ['--ro-bind', d, d]
            box += ['--proc', '/proc', '--dev', '/dev', '--tmpfs', '/tmp',
                    '--ro-bind', str(release), str(release), '--bind', tmp, tmp,
                    '--chdir', tmp, '--setenv', 'PATH', '/usr/bin:/bin', '--setenv', 'LANG', 'C.UTF-8']
            grants = ['--allow-exec', '--allow-ffi', '--ffi-lib='+libc] + ['--ffi-symbol='+s for s in SYMBOLS if s != omit]
            denies = ['--deny-'+s for s in ['net', 'http', 'env', 'spawn', 'gfx', 'sqlite']]
            if omit == 'ffi':
                denies += ['--deny-ffi']
            p = subprocess.run(box+[str(release/'bin/pblang'), 'run', 'main.sapp', *grants, *denies],
                               text=True, capture_output=True, timeout=15)
            effects = dict(outside=(root/'outside').read_text(), shared_mode=oct((root/'shared').stat().st_mode & 0o777),
                           shared_file=(root/'shared/file').exists(), stage_files=[str(p.relative_to(root)) for p in root.rglob('.pblang-*')])
            return dict(exit=p.returncode, stdout=p.stdout, stderr=p.stderr), effects

    def old_calls(source):
        source = source.replace('stdlib:compiler/module_cache_file.sapp', 'stdlib:secure_file.sapp')
        # Use tiny forwarding wrappers so nested argument expressions remain intact.
        source = source.replace('let C = import(', 'let S = import(')
        end = source.index(';', source.index('let S =')) + 1
        wrappers = ('\nfn read_bytes(p,n){return S.read_bytes(p,n,true);}'
                    '\nfn read_text(p,n){return S.read_text(p,n,true);}'
                    '\nfn write_atomic(p,b){return S.write_atomic(p,b,true,true);}'
                    '\nfn write_text_atomic(p,t){return S.write_text_atomic(p,t,true,true);}'
                    '\nfn remove(p){return S.remove(p,true);}\n')
        return source[:end] + wrappers + source[end:].replace('C.', '')

    if args.group == 'parity':
        source = (source_root/'tests/semantics/module_cache_file_test.sapp').read_text()
        expected, before = run(args.reference, old_calls(source))
        record('existing private-file contract', expected['exit'] == 0 and expected['stdout'].endswith('module_cache_file_pass\n'), result=expected)
        actual, after = run(args.candidate, source)
        record('same results and diagnostics', actual == expected, result=actual)
        record('outside inode and parent modes preserved', before == after == dict(outside='outside', shared_mode='0o755', shared_file=False, stage_files=[]))
    elif args.group == 'denials':
        source = ('let C = import("stdlib:compiler/module_cache_file.sapp");'
                  'print json_stringify(C.write_text_atomic("private/denied", "x"));'
                  'print json_stringify(C.read_bytes("private/denied", 8));')
        for omitted in ['ffi', 'free', 'close', 'strdup', 'openat']:
            expected, _ = run(args.reference, old_calls(source), omitted)
            actual, _ = run(args.candidate, source, omitted)
            record('denied '+omitted, actual == expected and actual['exit'] == 0
                   and all(json.loads(line)['ok'] is False for line in actual['stdout'].splitlines()), result=actual)
    elif args.group == 'integration':
        source = (source_root/'tests/semantics/module_cache_test.sapp').read_text()
        for name, release in [('reference', args.reference), ('candidate', args.candidate)]:
            result, _ = run(release, source)
            record(name+' persistent cache integration', result['exit'] == 0 and result['stdout'] == 'module_cache_pass\n', result=result)

    else:
        # Replace exactly one import in a private copy of each complete facade.
        # The model owns no native resources; actual I/O is tested separately.
        backend = BACKEND_FIXTURE
        cases = [('read-throw', 'read'), ('write-throw', 'write'), ('remove-throw', 'remove'),
                 ('policy-throw', 'read'), ('mkdir-throw', 'create'),
                 ('close-false', 'read'), ('close-false', 'write'), ('close-false', 'remove'),
                 ('close-nil', 'read'), ('close-number', 'read'), ('close-throw', 'read'),
                 ('first-close-false', 'create'), ('read-error-close-false', 'read'),
                 ('write-error-close-throw', 'write'), ('remove-error-close-false', 'remove')]
        for mode, operation in cases:
            pair = []
            for variant, release, filename in [('baseline', args.reference, 'stdlib/secure_file.sapp'),
                                               ('candidate', args.candidate, 'stdlib/compiler/module_cache_file.sapp')]:
                call = {'read': 'read_bytes("private/value",8)', 'write': 'write_atomic("private/value",[65])',
                        'remove': 'remove("private/value")', 'create': 'write_atomic("new/value",[65])'}[operation]
                if variant == 'baseline':
                    call = call[:-1] + (',true)' if operation in ['read', 'remove'] else ',true,true)')
                source = ('let B=import("backend.sapp");let C=import("facade.sapp");B.configure('
                          + json.dumps(mode) + ');print json_stringify(C.' + call + ');')
                facade = (release/filename).read_text()
                needle = 'let LinuxDirFS = import("stdlib:linux_dirfs.sapp");'
                assert facade.count(needle) == 1
                result, _ = run(release, source, 'ffi',
                                facade.replace(needle, 'let LinuxDirFS = import("backend.sapp");'), backend)
                errors = re.findall(r'^error\[.*?\]: (.*)$', result['stderr'], re.M)
                pair.append(dict(variant=variant, exit=result['exit'], stdout=result['stdout'],
                                 error=errors[0] if errors else None))
            before, after = pair
            if mode in ['read-throw', 'write-throw', 'remove-throw', 'policy-throw', 'mkdir-throw']:
                # OWN3-J02: the backend exception must still propagate with its
                # original diagnostic, but the opened parent is now closed exactly
                # once during unwinding. The reference secure_file path skips the
                # close (empty stdout); it is kept only as the diagnostic oracle.
                passed = (before['exit'] == after['exit'] == 1
                          and before['error'] == after['error'] and before['error'].startswith('injected-primary-')
                          and after['stdout'].count('close:') == 1
                          and after['stdout'].strip() == 'close:1')
                record(mode + '/opaque-exception-cleanup', passed, results=pair,
                       cleanup_fixed=True, reference_closes=before['stdout'].count('close:'),
                       candidate_closes=after['stdout'].count('close:'))
            else:
                assert after['exit'] == 0, after
                value = json.loads(after['stdout'].splitlines()[-1])
                passed = value['ok'] is False and value['cleanup_uncertain'] is True
                if '-error-' in mode:
                    passed = passed and value['error'] == 'injected-known-primary' and value['detail'] == 7
                passed = passed and after['stdout'].count('close:') == 1
                if mode.endswith('close-throw'):
                    passed = passed and before['exit'] == 1 and before['error'] == 'injected-close'
                elif '-error-' in mode:
                    passed = passed and before['exit'] == 0 and json.loads(before['stdout'].splitlines()[-1])['error'] == value['error']
                else:
                    passed = passed and before['exit'] == 0 and json.loads(before['stdout'].splitlines()[-1])['ok'] is True
                record(mode + '/' + operation, passed, results=pair, close_result_checked=True,
                       native_release_verified=False)


if __name__ == '__main__':
    main()
