"""Prefix cleanup ownership and UTF-8 regressions in isolated Linux processes.

The caller serializes the finite suite with the shared test lock. Tests use
complete facade copies, never production hooks. Backend exceptions are synthetic;
real parent descriptors are counted separately. No browser is launched.
"""
import argparse
import hashlib
import json
import os
import sys
from pathlib import Path
import tempfile
import tomllib

from check_cli_exact_argv import bounded_process, OUTPUT_CAP, CHILD_SECONDS
from secure_file_exclusive_close_check import replace_once, snapshot, SYMBOLS

BACKEND = '''
let state=map{};
fn configure(read_mode,close_mode) {
 state.read_mode=read_mode;state.close_mode=close_mode;
 state.opens=0;state.reads=0;state.closes=0;state.admit=true;
}
fn open(path,create=false,protect=false) {
 state.opens=state.opens+1;return map{ok:state.admit};
}
fn open_absolute(path,create=false,protect=false) {return open(path,create,protect);}
fn close(fs) {
 state.closes=state.closes+1;
 if state.close_mode=="throw" {throw "synthetic-private-value";}
 if state.close_mode=="nil" {return nil;}
 if state.close_mode=="number" {return 1;}
 if state.close_mode=="map" {return map{ok:true};}
 return state.close_mode=="success";
}
fn read_prefix_bytes(fs,name,limit) {
 state.reads=state.reads+1;
 if state.read_mode=="throw" {throw "synthetic-private-value";}
 if state.read_mode=="failure" {return map{ok:false,error:"synthetic failure",marker:7};}
 return map{ok:true,bytes:[206,177],source_size:2,truncated:false};
}
export state,configure,open,open_absolute,close,read_prefix_bytes;
'''


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def execute(runtime, files, real=False):
    """Use streaming pre-append output cap at the actual process boundary."""
    with tempfile.TemporaryDirectory(prefix='prefix-cleanup-') as tmp:
        root = Path(tmp)
        scratch = root/'scratch'
        scratch.mkdir(mode=0o700)
        for name, data in files.items():
            path = root/name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data if isinstance(data, bytes) else data.encode())
        before = {p.name: snapshot(p) for p in scratch.iterdir()}
        manifest = ('[package]\nname="prefix-cleanup-test"\nversion="1.0.0"\nentry="main.sapp"\n'
                    '[dependencies]\nstdlib={path='+json.dumps(str(runtime/'stdlib'))+'}\n')
        (root/'pblang.toml').write_text(manifest)
        dep = tomllib.loads((runtime/'pblang.lock').read_text())['dependencies']['stdlib']
        (root/'pblang.lock').write_text('manifest_sha256='+json.dumps(hashlib.sha256(manifest.encode()).hexdigest())
            +'\ncontent_hash_algorithm="pblang-package-tree-v2"\n[dependencies.stdlib]\npath='
            +json.dumps(str(runtime/'stdlib'))+'\nsha256='+json.dumps(dep['sha256'])+'\n')
        box = ['bwrap', '--unshare-all', '--die-with-parent', '--clearenv']
        for path in ['/usr', '/bin', '/lib', '/lib64']:
            box += ['--ro-bind', path, path]
        box += ['--proc', '/proc', '--dev', '/dev', '--tmpfs', '/tmp',
                '--ro-bind', str(runtime), str(runtime), '--ro-bind', tmp, tmp,
                '--bind', str(scratch), str(scratch), '--chdir', tmp,
                '--setenv', 'PATH', '/usr/bin:/bin', '--setenv', 'LANG', 'C.UTF-8']
        flags = ['--deny-'+s for s in ['net', 'http', 'env', 'spawn', 'gfx', 'sqlite']]
        if real:
            flags += ['--allow-exec', '--allow-ffi', '--ffi-lib=/usr/lib/x86_64-linux-gnu/libc.so.6']
            flags += ['--ffi-symbol='+s for s in SYMBOLS]
        else:
            flags += ['--deny-ffi', '--deny-exec']
        result = bounded_process(box+[str(runtime/'bin/pblang'), 'run', 'main.sapp', *flags])
        after = {p.name: snapshot(p) for p in scratch.iterdir()}
        result['inputs_unchanged'] = before == after
        return result


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--runtime', required=True, type=Path, help='materialized retained release directory')
    ap.add_argument('--baseline-source', required=True, type=Path, help='unchanged old secure_file.sapp')
    ap.add_argument('--report', required=True, type=Path, help='new output file, never overwritten')
    args = ap.parse_args()
    os.umask(0o077)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    # Reserve before any fixture executes; prior evidence is never overwritten.
    with args.report.open('x') as out:
        out.write('{}\n')
    runtime = args.runtime.resolve()
    root = Path(__file__).resolve().parents[2]
    raw = (root/'stdlib/secure_file.sapp').read_text()
    baseline = args.baseline_source.read_text()
    semantic = (root/'tests/semantics/secure_file_prefix_cleanup_test.sapp').read_text()
    web = (root/'stdlib/polyglot_web.sapp').read_text()
    rows = []
    pins = {name: digest(runtime/name) for name in ['bin/pblang', 'bin/pblang-native', 'pblang.lock',
        'stdlib/linux_dirfs.sapp', 'stdlib/linux_dirfs_read_core.sapp', 'stdlib/secure_file.sapp']}
    report = dict(passed=False, rows=rows, runtime_pins=pins,
        runtime_facade_matches_source=digest(runtime/'stdlib/secure_file.sapp')==hashlib.sha256(raw.encode()).hexdigest(),
        source_sha256=hashlib.sha256(raw.encode()).hexdigest(), baseline_sha256=digest(args.baseline_source),
        harness_sha256=digest(Path(__file__)),
        helper_sha256={name:digest(root/'tests/tooling'/name) for name in
            ['check_cli_exact_argv.py','secure_file_exclusive_close_check.py']},
        semantic_sha256=hashlib.sha256(semantic.encode()).hexdigest(),
        caller_sha256=hashlib.sha256(web.encode()).hexdigest(),
        output_cap_bytes=OUTPUT_CAP, per_child_seconds=CHILD_SECONDS,
        source_hidden=True, network_isolated=True, native_fault_injected=False)
    # Admit the actual release before spawning any fixture. Recording a mismatch
    # without refusing it would let a different runtime certify the source.
    if not report['runtime_facade_matches_source']:
        report['admission_error'] = 'runtime facade does not match reviewed source'
        args.report.write_text(json.dumps(report, indent=2)+'\n')
        print(report['admission_error'], file=sys.stderr)
        raise SystemExit(1)
    args.report.write_text(json.dumps(report, indent=2)+'\n')

    def record(label, files, expected, real=False):
        result = execute(runtime, files, real)
        good = (result['exit'] == 0 and result['stderr'] == '' and result['failure'] is None
            and not result['group_survived_normal_exit'] and result['inputs_unchanged'])
        if good:
            good = json.loads(result['stdout']) == expected
        rows.append(dict(label=label, passed=bool(good), result=result,
            fixture_sha256={k: hashlib.sha256(v if isinstance(v,bytes) else v.encode()).hexdigest()
                for k,v in files.items()}))
        args.report.write_text(json.dumps(report, indent=2)+'\n')
        print(label, bool(good), flush=True)
        assert good, rows[-1]

    model = replace_once(raw, 'let LinuxDirFS = import("stdlib:linux_dirfs.sapp");',
                         'let LinuxDirFS = import("backend.sapp");')
    record('modeled ownership matrix', {'facade.sapp': model, 'backend.sapp': BACKEND,
        'semantic.sapp': semantic, 'main.sapp': 'let B=import("backend.sapp");let S=import("facade.sapp");'
            'let T=import("semantic.sapp");print json_stringify(T.run(S,B));'},
        dict(ok=True,matrix_cases=18,admission_cases=6))

    old_model = replace_once(baseline, 'let LinuxDirFS = import("stdlib:linux_dirfs.sapp");',
                             'let LinuxDirFS = import("backend.sapp");')
    record('old modeled source rejected by new contract', {'facade.sapp':old_model,
        'backend.sapp':BACKEND,'semantic.sapp':semantic,
        'main.sapp':'let B=import("backend.sapp");let S=import("facade.sapp");let T=import("semantic.sapp");'
            'let rejected=try {T.run(S,B);false} catch true;print json_stringify(map{rejected:rejected});'},
        dict(rejected=True))

    real_main = '''let S=import("facade.sapp");let W=import("caller.sapp");
let before=len(fs_listdir("/proc/self/fd"));let checks=0;
for i 12 {
 let whole=S.read_text_prefix("scratch/input.html",5);
 assert(whole.ok and whole.text=="A€Z" and whole.source_size==5 and whole.size==5
  and whole.returned_bytes==5 and not whole.truncated and whole.utf8_boundary_trimmed_bytes==0,"exact prefix");
 let large=S.read_text_prefix("scratch/input.html",8);assert(large.ok and large.text=="A€Z" and not large.truncated,"larger cap");
 let zero=S.read_text_prefix("scratch/input.html",0);assert(zero.ok and zero.text=="" and zero.source_size==5
  and zero.returned_bytes==0 and zero.truncated,"zero cap");
 for cap in [2,3] {let split=S.read_text_prefix("scratch/input.html",cap);assert(split.ok and split.text=="A"
  and split.returned_bytes==1 and split.source_size==5 and split.truncated
  and split.utf8_boundary_trimmed_bytes==cap-1,"split scalar");}
 let scalar=S.read_text_prefix("scratch/input.html",4);assert(scalar.ok and scalar.text=="A€"
  and scalar.returned_bytes==4 and scalar.truncated and scalar.utf8_boundary_trimmed_bytes==0,"exact scalar");
 assert(not S.read_text_prefix("scratch/malformed",2).ok,"invalid UTF-8");
 assert(not S.read_text_prefix("scratch/incomplete",2).ok,"incomplete EOF");
 assert(not S.read_text_prefix("scratch/malformed-tail",2).ok,"malformed truncated UTF-8");
 let empty=S.read_text_prefix("scratch/empty",0);assert(empty.ok and empty.text=="" and not empty.truncated,"empty file");
 assert(not S.read_text_prefix("scratch/missing",1).ok,"missing backend file");
 assert(W.browser_plan("scratch/input.html").ok,"real browser plan without launch");checks=checks+12;
}
let after=len(fs_listdir("/proc/self/fd"));assert(before==after,"no parent descriptor growth");
print json_stringify(map{ok:true,checks:checks,iterations:12,fd_delta:after-before});'''
    caller = replace_once(web, 'let SECURE_FILE = import("stdlib:secure_file.sapp");',
                          'let SECURE_FILE = import("facade.sapp");')
    record('real UTF-8 limits consumer and descriptors', {'facade.sapp':raw,'caller.sapp':caller,
        'main.sapp':real_main,'scratch/input.html':'A€Z','scratch/empty':'',
        'scratch/malformed':b'A\xff','scratch/incomplete':b'A\xc2','scratch/malformed-tail':b'A\xffZ'},
        dict(ok=True,checks=144,iterations=12,fd_delta=0), True)

    record('canonical runtime prefix and browser-plan imports', {
        'main.sapp':real_main.replace('import("facade.sapp")','import("stdlib:secure_file.sapp")')
            .replace('import("caller.sapp")','import("stdlib:polyglot_web.sapp")'),
        'scratch/input.html':'A€Z','scratch/empty':'','scratch/malformed':b'A\xff',
        'scratch/incomplete':b'A\xc2','scratch/malformed-tail':b'A\xffZ'},
        dict(ok=True,checks=144,iterations=12,fd_delta=0),True)

    branch = 'if platform() == "linux" { return LinuxDirFS.read_prefix_bytes(fs, name, max_bytes); }'
    throwing = 'if platform() == "linux" { throw "synthetic-private-value"; }'
    for name, source, expected_leak in [('candidate',raw,False),('baseline',baseline,True)]:
        for consumer in [False, True]:
            call = 'W.browser_plan("scratch/input.html")' if consumer else 'S.read_text_prefix("scratch/input.html",1)'
            main = 'let S=import("facade.sapp");let W=import("caller.sapp");let before=len(fs_listdir("/proc/self/fd"));let exceptions=0;let denied=0;'
            main += 'for i 4 {let r=try {'+call+'} catch {exceptions=exceptions+1;nil};if r!=nil and not r.ok {denied=denied+1;assert(not map_has(r,"text") and not map_has(r,"bytes"),"no data");}}'
            main += 'let after=len(fs_listdir("/proc/self/fd"));print json_stringify(map{exceptions:exceptions,denied:denied,fd_delta:after-before});'
            record(name+(' browser-plan' if consumer else ' prefix')+' backend exception',
                {'facade.sapp':replace_once(source,branch,throwing),'caller.sapp':caller,
                 'main.sapp':main,'scratch/input.html':'A'},
                dict(exceptions=4 if expected_leak else 0,denied=0 if expected_leak else 4,
                     fd_delta=4 if expected_leak else 0),True)

    # Real successful close followed by an ambiguous facade result: confirms
    # result suppression, not a native close-before failure or leakproof claim.
    close_branch = 'if platform() == "linux" { return LinuxDirFS.close(fs); }'
    for mode in ['false','throw']:
        close_end = 'throw "synthetic-private-value";' if mode=='throw' else 'return false;'
        facade = replace_once(raw, close_branch,
            'if platform() == "linux" { LinuxDirFS.close(fs); '+close_end+' }')
        main = '''let S=import("facade.sapp");let before=len(fs_listdir("/proc/self/fd"));for i 12 {
let r=S.read_text_prefix("scratch/input.html",1);assert(not r.ok and r.cleanup_uncertain
 and not r.parent_close_confirmed and not map_has(r,"text") and not map_has(r,"bytes"),"cleanup no data");}
let after=len(fs_listdir("/proc/self/fd"));print json_stringify(map{ok:true,iterations:12,fd_delta:after-before});'''
        record('real close then '+mode, {'facade.sapp':facade,'main.sapp':main,'scratch/input.html':'A'},
            dict(ok=True,iterations=12,fd_delta=0),True)
    assert all(digest(runtime/name)==value for name,value in pins.items()), 'runtime changed during checks'
    report['passed'] = all(row['passed'] for row in rows)
    args.report.write_text(json.dumps(report, indent=2)+'\n')


if __name__ == '__main__':
    main()
