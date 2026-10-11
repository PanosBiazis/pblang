"""Read cleanup matrix with bounded isolated processes; no production test hooks."""
import argparse
import hashlib
import json
from pathlib import Path

from secure_file_exclusive_close_check import execute, replace_once

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
fn read_bytes(fs,name,limit) {
 state.reads=state.reads+1;
 if state.read_mode=="throw" {throw "synthetic-private-value";}
 if state.read_mode=="failure" {return map{ok:false,error:"synthetic failure"};}
 return map{ok:true,bytes:[206,177],size:2};
}
export state,configure,open,open_absolute,close,read_bytes;
'''


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--runtime', required=True, type=Path)
    ap.add_argument('--report', required=True, type=Path)
    args = ap.parse_args()
    root = Path(__file__).resolve().parents[2]
    raw = (root/'stdlib/secure_file.sapp').read_text()
    facade = replace_once(raw, 'let LinuxDirFS = import("stdlib:linux_dirfs.sapp");',
                          'let LinuxDirFS = import("backend.sapp");')
    semantic = (root/'tests/semantics/secure_file_read_cleanup_test.sapp').read_text()
    files = {'facade.sapp': facade, 'backend.sapp': BACKEND, 'semantic.sapp': semantic,
             'main.sapp': 'let B=import("backend.sapp");let S=import("facade.sapp");'
                          'let T=import("semantic.sapp");print json_stringify(T.run(S,B));'}
    result = execute(args.runtime.resolve(), files)
    passed = (result['exit'] == 0 and result['stderr'] == '' and result['bounded']
              and result['inputs_unchanged']
              and json.loads(result['stdout']) == dict(ok=True, matrix_cases=36, admission_cases=4))
    report = {'passed': passed, 'result': result,
              'source_sha256': hashlib.sha256(raw.encode()).hexdigest(),
              'fixture_sha256': {k: hashlib.sha256(v.encode()).hexdigest() for k,v in files.items()}}
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2)+'\n')
    args.report.chmod(0o600)
    print(json.dumps(report))
    raise SystemExit(0 if passed else 1)


if __name__ == '__main__':
    main()
