"""Bounded Linux exclusive-write cleanup qualification, using an existing runtime.

No production injection hooks. Complete source copies are substituted only in
private fixtures. Baseline and candidate use identical payloads and fault models.
Run serially under the coordinator's existing heavy lock (never create a new one).
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import resource
import signal
import subprocess
import tempfile
import time
import tomllib

BACKEND = r'''
let state=map{};
fn configure(success,closed) {
 state.success=success;state.closed=closed;state.opens=0;state.writes=0;state.closes=0;
 state.admit=true;state.backend_throw=false;state.close_throw=false;state.object_error=false;
 state.result=nil;state.bytes=nil;state.create=nil;
}
fn open(path,create=false,protect=false) {
 state.opens=state.opens+1;
 return if state.admit {map{ok:true}} else {map{ok:false,error:"admission denied"}};
}
fn open_absolute(path,create=false,protect=false) {return open(path,create,protect);}
fn close(fs) {
 state.closes=state.closes+1;
 if state.close_throw {print "close:"+str(state.closes)+";writes:"+str(state.writes);throw "synthetic-close-error";}
 return state.closed;
}
fn write_new_exclusive(fs,name,bytes,create=false) {
 state.writes=state.writes+1;state.bytes=bytes;state.create=create;
 if state.backend_throw {print "write:"+str(state.writes)+";closes:"+str(state.closes);throw "synthetic-backend-error";}
 state.result=map{ok:state.success,size:len(bytes),no_replace:true,atomic_visibility:false,
  published:true,durability_verified:false,extra:map{value:7,items:[1,2]}};
 if not state.success {state.result.error=if state.object_error {map{kind:"synthetic",code:7}} else {"αρχικό σφάλμα"};}
 return state.result;
}
export configure,state,open,open_absolute,close,write_new_exclusive;
'''

CALLER_ADAPTER = r'''
let S=import("facade.sapp");
let state=map{removes:0,atomic_writes:0};
fn create_private_temporary_directory_new(path) {return map{ok:true};}
fn write_text_new_exclusive(path,text,create=true,private=false) {
 // Model admission independently; the real facade's write/close is unchanged.
 return S.write_text_new_exclusive("owned/value",text,false,false);
}
fn remove(path,private=false) {state.removes=state.removes+1;return map{ok:true};}
fn write_text_atomic(path,text,create=true,private=false) {
 state.atomic_writes=state.atomic_writes+1;return map{ok:true};
}
export state,create_private_temporary_directory_new,write_text_new_exclusive,remove,write_text_atomic;
'''

SYMBOLS = ['free', 'close', 'dup', 'strdup', 'openat', 'getpid', 'getuid',
           'memfd_create', 'sendfile', 'lseek', 'fchmod', 'fsync', 'calloc', 'pread', 'write']


def replace_once(text, old, new):
    assert text.count(old) == 1, old
    return text.replace(old, new)


def execute(runtime, files, real=False, dependency=None):
    with tempfile.TemporaryDirectory(prefix='exclusive-close-') as tmp:
        root = Path(tmp)
        scratch = root/'scratch'
        scratch.mkdir(mode=0o700)
        (scratch/'input').write_bytes(b'original input')
        (scratch/'existing').write_bytes(b'KEEP')
        os.link(scratch/'input', scratch/'alias')
        (scratch/'link').symlink_to('input')
        for name, text in files.items():
            (root/name).write_text(text)
        before = {p.name: snapshot(p) for p in scratch.iterdir()}
        stdlib = runtime/'stdlib' if dependency is None else dependency[0]
        manifest = ('[package]\nname="exclusive-close-test"\nversion="1.0.0"\nentry="main.sapp"\n'
                    '[dependencies]\nstdlib={path='+json.dumps(str(stdlib))+'}\n')
        (root/'pblang.toml').write_text(manifest)
        dep = tomllib.loads((runtime/'pblang.lock').read_text())['dependencies']['stdlib'] if dependency is None else {'sha256': dependency[1]}
        (root/'pblang.lock').write_text('manifest_sha256='+json.dumps(hashlib.sha256(manifest.encode()).hexdigest())
            +'\ncontent_hash_algorithm="pblang-package-tree-v2"\n[dependencies.stdlib]\npath='
            +json.dumps(str(stdlib))+'\nsha256='+json.dumps(dep['sha256'])+'\n')
        box = ['bwrap', '--unshare-all', '--die-with-parent', '--clearenv']
        for path in ['/usr', '/bin', '/lib', '/lib64']:
            box += ['--ro-bind', path, path]
        box += ['--proc', '/proc', '--dev', '/dev', '--tmpfs', '/tmp',
                '--ro-bind', str(runtime), str(runtime), '--ro-bind', tmp, tmp,
                '--bind', str(scratch), str(scratch), '--chdir', tmp,
                '--setenv', 'PATH', '/usr/bin:/bin', '--setenv', 'LANG', 'C.UTF-8']
        if dependency is not None:
            box += ['--ro-bind', str(stdlib.parent), str(stdlib.parent),
                    '--ro-bind', str(stdlib.parent), str(root.parent/'pblang')]
        flags = ['--deny-'+s for s in ['net', 'http', 'env', 'spawn', 'gfx', 'sqlite']]
        if real:
            flags += ['--allow-exec', '--allow-ffi', '--ffi-lib=/usr/lib/x86_64-linux-gnu/libc.so.6']
            flags += ['--ffi-symbol='+s for s in SYMBOLS]
        else:
            flags += ['--deny-ffi', '--deny-exec']
        def limits():
            for kind, value in [(resource.RLIMIT_AS, 512*1024*1024), (resource.RLIMIT_CPU, 10),
                                (resource.RLIMIT_FSIZE, 1024*1024), (resource.RLIMIT_CORE, 0)]:
                resource.setrlimit(kind, (value, value))
        p = subprocess.Popen(box+[str(runtime/'bin/pblang'), 'run', 'main.sapp', *flags],
                             stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                             start_new_session=True, preexec_fn=limits)
        timed_out = False
        try:
            stdout, stderr = p.communicate(timeout=15)
        except subprocess.TimeoutExpired:
            timed_out = True
            os.killpg(p.pid, signal.SIGKILL)
            stdout, stderr = p.communicate()
        remaining = False
        deadline = time.monotonic()+0.5
        while True:
            try:
                os.killpg(p.pid, 0)
            except ProcessLookupError:
                break
            if time.monotonic() >= deadline:
                remaining = True
                os.killpg(p.pid, signal.SIGKILL)
                break
            time.sleep(0.005)
        after = {f.name: snapshot(f) for f in scratch.iterdir()}
        return dict(exit=p.returncode, stdout=stdout.decode(errors='replace'),
                    stderr=stderr.decode(errors='replace'), timed_out=timed_out,
                    process_group_remaining=remaining, inputs_unchanged=all(after.get(k)==v for k,v in before.items()),
                    output_modes={f.name: f.stat().st_mode & 0o777 for f in scratch.iterdir() if f.name not in before},
                    new_outputs={f.name: f.read_bytes().hex() for f in scratch.iterdir() if f.name not in before},
                    bounded=not timed_out and not remaining and len(stdout)<65536 and len(stderr)<65536)


def snapshot(p):
    s = p.lstat()
    return dict(sha256=hashlib.sha256(p.read_bytes()).hexdigest() if not p.is_symlink() else None,
                link=os.readlink(p) if p.is_symlink() else None,
                **{k: getattr(s, 'st_'+k) for k in ['dev', 'ino', 'uid', 'gid', 'mode', 'size', 'mtime_ns', 'ctime_ns']})


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--runtime', type=Path, required=True)
    ap.add_argument('--baseline', type=Path, required=True, help='immutable source root of original main')
    ap.add_argument('--report', type=Path, required=True)
    ap.add_argument('--product', type=Path, action='append', default=[],
                    help='optional product root beside its unchanged pinned pblang dependency')
    a = ap.parse_args()
    os.umask(0o077)
    runtime = a.runtime.resolve()
    candidate = Path(__file__).resolve().parents[2]
    semantic = (candidate/'tests/semantics/secure_file_exclusive_close_test.sapp').read_text()
    rows = []

    def record(label, result, passed):
        passed = bool(passed and result['bounded'] and result['inputs_unchanged']
                      and (result['exit'] != 0 or result['stderr'] == ''))
        rows.append(dict(label=label, passed=passed, result=result))
        a.report.write_text(json.dumps(rows, indent=2, ensure_ascii=False)+'\n')
        a.report.chmod(0o600)
        print(label, passed, flush=True)
        assert passed, rows[-1]

    if a.product:
        check_products(a.product, candidate, runtime, record)
        return

    for variant, source in [('baseline', a.baseline), ('candidate', candidate)]:
        raw = (source/'stdlib/secure_file.sapp').read_text()
        facade = replace_once(raw, 'let LinuxDirFS = import("stdlib:linux_dirfs.sapp");',
                              'let LinuxDirFS = import("backend.sapp");')
        prefix = 'let B=import("backend.sapp");let S=import("facade.sapp");'
        files = {'backend.sapp': BACKEND, 'facade.sapp': facade, 'semantic.sapp': semantic}
        files['main.sapp'] = prefix+'let T=import("semantic.sapp");print json_stringify(T.run(S,B,'+str(variant=='baseline').lower()+'));'
        result = execute(runtime, files)
        record(variant+'/matrix', result, result['exit']==0 and '"matrix_cases":16' in result['stdout'])
        for api in ['write_new_exclusive("owned/value",[65],false)', 'write_text_new_exclusive("owned/value","A",false)']:
            for mode in ['backend', 'close-success', 'close-failure']:
                setup = 'B.configure('+str(mode!='close-failure').lower()+',true);B.state.'
                setup += 'backend_throw=true;' if mode=='backend' else 'close_throw=true;'
                files['main.sapp'] = prefix+setup+'S.'+api+';'
                result = execute(runtime, files)
                error = 'synthetic-backend-error' if mode=='backend' else 'synthetic-close-error'
                expected_stdout = 'write:1;closes:0\n' if mode=='backend' else 'close:1;writes:1\n'
                record(variant+'/'+mode+'/'+api.split('(')[0], result,
                       result['exit']==1 and error in result['stderr'] and result['stdout']==expected_stdout)
        for caller in ['polyglot_web', 'polyglot_build']:
            text = (source/'stdlib'/f'{caller}.sapp').read_text()
            text = replace_once(text, 'let SECURE_FILE = import("stdlib:secure_file.sapp");',
                                'let SECURE_FILE = import("caller_adapter.sapp");')
            # Call a real public builder; no toolchain or private-function export.
            call = 'C.bundle("synthetic","","")' if caller=='polyglot_web' else 'C.build_language("python","sample","synthetic","/tmp",map{unsafe_polyglot:true,trusted_source:true,allow_toolchain_effects:true,unsafe_polyglot_sha256:H.sha256_hex_bytes(str_to_bytes("synthetic"))})'
            for success in [True, False]:
                files['caller.sapp'] = text
                files['caller_adapter.sapp'] = CALLER_ADAPTER
                files['main.sapp'] = prefix+'let H=import("stdlib:sha256_pb.sapp");let A=import("caller_adapter.sapp");let C=import("caller.sapp");B.configure('+str(success).lower()+',false);let failed=try {'+call+';false} catch true;print json_stringify(map{failed:failed,writes:B.state.writes,closes:B.state.closes,removes:A.state.removes,atomic_writes:A.state.atomic_writes});'
                result = execute(runtime, files)
                expected = dict(failed=True, writes=1, closes=1, removes=0, atomic_writes=0)
                if variant=='baseline':
                    expected = dict(failed=not success,writes=1 if success or caller=='polyglot_build' else 8,
                                    closes=1 if success or caller=='polyglot_build' else 8,
                                    removes=int(not success and caller=='polyglot_build'),
                                    atomic_writes=int(success and caller=='polyglot_build'))
                record(variant+'/'+caller+'/'+str(success), result,
                       result['exit']==0 and json.loads(result['stdout'])==expected)
        real = {'facade.sapp': raw, 'main.sapp': '''let S=import("facade.sapp");
let a=S.write_text_new_exclusive("scratch/new-text","α",false,false);
let b=S.write_new_exclusive("scratch/new-bytes",[0,127,255],false,false);
assert(a.ok and b.ok and not a.atomic_visibility and not b.atomic_visibility,"exclusive creates");
for path in ["scratch/existing","scratch/alias","scratch/link","scratch/missing/file"] {
 assert(not S.write_text_new_exclusive(path,"replace",false,false).ok,"refusal");
}
print "real-pass";'''}
        result = execute(runtime, real, True)
        record(variant+'/real-io', result, result['exit']==0 and result['stdout']=='real-pass\n'
               and result['new_outputs']=={'new-text':'ceb1','new-bytes':'007fff'})



def check_products(products, candidate, runtime, record):
    for product in products:
        product = product.resolve()
        name = product.name
        dep = product.parent/'pblang'
        dependency = (dep/'stdlib', tomllib.loads((product/'pblang.lock').read_text())['dependencies']['stdlib']['sha256'])
        spec = json.loads((product/'tests/cli_cases.json').read_text())
        wanted = {'pbtablecheck': 'export-success', 'pbtaskscope': 'scope-success', 'pbsmooth': 'smooth-success'}[name]
        case = next(c for c in spec['cases'] if c['name']==wanted)
        input_path = 'scratch/'+case['args'][1]
        output_path = 'scratch/'+case['output']
        modules = ['task_plan_files.sapp'] if name=='pbtaskscope' else ['table_files.sapp']
        if name=='pbsmooth':
            modules += ['smoothing_files.sapp']
        original = (dep/'stdlib/secure_file.sapp').read_text()
        improved = (candidate/'stdlib/secure_file.sapp').read_text()
        expected = None
        for variant, facade, fault in [('pinned-normal', original, 'true'), ('pinned-false', original, 'false'),
                                       ('candidate-false', improved, 'false'), ('candidate-throw', improved, 'throw')]:
            files = {'scratch/'+k: v for k,v in spec['files'].items()}
            for module in modules:
                text = (dep/'stdlib'/module).read_text()
                text = text.replace('"stdlib:secure_file.sapp"', '"facade.sapp"')
                text = text.replace('"stdlib:table_files.sapp"', '"table_files.sapp"')
                files[module] = text
            behavior = 'throw "synthetic-close-error";' if fault=='throw' else 'return '+fault+';'
            facade = replace_once(facade, 'if platform() == "linux" { return LinuxDirFS.close(fs); }',
                'if platform() == "linux" { let real=LinuxDirFS.close(fs); faults.calls=faults.calls+1; faults.real_closed=faults.real_closed and real; '+behavior+' }')
            files['facade.sapp'] = 'let faults=map{calls:0,real_closed:true};\n'+facade+'\nexport faults;\n'
            if name=='pbtablecheck':
                code = 'let C=import("stdlib:sheet_csv.sapp");let Q=import("stdlib:table_quality.sapp");let F=import("table_files.sapp");let result=F.save(output,Q.select_flagged(C.decode(fs_read(input)),true),"sheet");'
            elif name=='pbtaskscope':
                code = 'let D=import("stdlib:task_plan.sapp");let T=import("stdlib:task_scope.sapp");let F=import("task_plan_files.sapp");let result=F.save(output,T.extract(D.decode(fs_read(input)),["task_2"]));'
            else:
                code = 'let C=import("stdlib:sheet_csv.sapp");let S=import("stdlib:smoothing_table.sapp");let F=import("smoothing_files.sapp");let result=F.save(output,S.from_sheet(C.decode(fs_read(input)),2));'
            files['main.sapp'] = ('let B=import("facade.sapp");let input='+json.dumps(input_path)
                                  +';let output='+json.dumps(output_path)+';'+code
                                  +'print json_stringify(map{result:result,faults:B.faults});')
            result = execute(runtime, files, True, dependency)
            actual = json.loads(result['stdout']) if result['exit']==0 else None
            raw = result['new_outputs'].get(case['output'])
            exact_document = True
            if variant=='pinned-normal':
                expected = raw
                if raw is not None and 'output_json' in case:
                    exact_document = json.loads(bytes.fromhex(raw))==case['output_json']
                if raw is not None and 'output_csv' in case:
                    import csv
                    import io
                    exact_document = list(csv.reader(io.StringIO(bytes.fromhex(raw).decode())))==case['output_csv']
            record(name+'/'+variant, result, result['exit']==0 and result['stderr']=='' and actual is not None
                   and actual['faults']=={'calls':1, 'real_closed':True}
                   and actual['result']['ok']==variant.startswith('pinned-')
                   and raw is not None and raw==expected and exact_document
                   and set(result['new_outputs'])=={case['output']}
                   and result['output_modes']=={case['output']:0o600})


if __name__ == '__main__':
    main()
