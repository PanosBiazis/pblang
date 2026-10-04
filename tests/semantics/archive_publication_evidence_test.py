"""Result projection model plus deterministic parent-swap with real held I/O.

The model is not native fault evidence. The swap replaces only root acquisition
with a test interposer; all descriptor operations/publication remain production.
"""
from pathlib import Path
import json
import subprocess
import sys
import tempfile

repo = Path(__file__).resolve().parents[2]
runtime = str(Path(sys.argv[1]).resolve())
source = (repo / 'stdlib/archive_publication.sapp').read_text()
with tempfile.TemporaryDirectory(prefix='archive-model-') as temporary:
    root = Path(temporary)
    (root / 'pblang.toml').write_text('[package]\nname="archive-evidence"\nversion="0.1.0"\nentry="main.sapp"\n[dependencies]\nstdlib={path=' + json.dumps(str(repo / 'stdlib')) + ',paths=["stdlib"]}\n')
    lock = subprocess.run([runtime, '--lock', str(root)], cwd=repo, text=True, capture_output=True, timeout=15)
    assert lock.returncode == 0, lock.stdout + lock.stderr
    (root / 'publication.sapp').write_text(source.replace('import("stdlib:linux_dirfs.sapp")', 'import("dir.sapp")'))
    (root / 'dir.sapp').write_text('''let state=map{mode:"",opens:0,closes:0,copies:0};
fn configure(mode) {state.mode=mode;state.opens=0;state.closes=0;state.copies=0;}
fn open(path,a,b) {state.opens=state.opens+1;return map{ok:true};}
fn open_absolute(path,a,b) {return open(path,a,b);}
fn sealed_child_bytes(fs,name,bytes) {return map{ok:state.mode!="seal_failure"};}
fn copy_held_range_new_atomic(a,b,c,d,e,f,g,h) {
 state.copies=state.copies+1;
 if state.mode=="throws" {throw "SECRET_BACKEND";}
 if state.mode=="ambiguous" {return map{ok:false,error:"SECRET_BACKEND"};}
 if state.mode=="unsynced" or state.mode=="wrapper_both" {return map{ok:false,published:true,durability_verified:false};}
 return map{ok:true,durability_verified:true};
}
fn close(fs) {state.closes=state.closes+1;return state.mode!="cleanup" and state.mode!="wrapper_both";}
export configure,open,open_absolute,sealed_child_bytes,copy_held_range_new_atomic,close,state;
''')
    (root / 'main.sapp').write_text('''let D=import("dir.sapp");let P=import("publication.sapp");
let A=import("stdlib:secure_archive.sapp");let bytes=A.pack([map{name:"a",data:[1]}],"store");
D.configure("success");let invalid=P.save("new.zip",[0]);
assert(D.state.opens==0 and not invalid.publication_attempted,"invalid bytes never open output root");
for mode in ["success","seal_failure","ambiguous","throws","unsynced","cleanup","wrapper_both"] {
 D.configure(mode);let r=P.save("new.zip",bytes);
 assert(D.state.closes==1,"one root close");
 if str_contains(json_stringify(r),"SECRET_BACKEND") {throw "private error exposed";}
 if mode=="seal_failure" {assert(not r.ok and not r.written and D.state.copies==0,"no publication call");}
 elif mode=="ambiguous" or mode=="throws" {assert(not r.ok and not map_has(r,"written") and r.output_may_exist and not r.publication_confirmed,"uncertain publication");}
 else {assert(r.written and r.publication_confirmed and r.atomic_visibility,"retain positive evidence");}
 if mode=="cleanup" or mode=="wrapper_both" {assert(not r.ok and not r.wrapper_handles_closed and map_has(r,"cleanup_error"),"cleanup ambiguity preserved");}
 if mode=="unsynced" or mode=="wrapper_both" {assert(r.code=="publication_unconfirmed" and not r.durability_verified,"primary failure retained");}
 if mode=="success" {assert(r.ok and r.durability_verified and r.wrapper_handles_closed,"complete success");}
}
print "archive_evidence_model_8_pass";
''')
    p = subprocess.run([runtime, 'run', str(root / 'main.sapp'), '--allow-all'], cwd=repo,
                       text=True, capture_output=True, timeout=45)
    assert p.returncode == 0 and 'archive_evidence_model_8_pass' in p.stdout, p.stdout + p.stderr
    print(p.stdout.strip())
    parent = root / 'destination'
    parent.mkdir()
    moved = root / 'held-destination'
    replacement = root / 'replacement'
    replacement.mkdir()
    sentinel = replacement / 'sentinel'
    sentinel.write_bytes(b'keep')
    (root / 'dir.sapp').write_text('''let Real=import("stdlib:linux_dirfs.sapp");
fn open_absolute(path,a,b) {
 let fs=Real.open_absolute(path,a,b);
 if fs.ok {
  assert(proc_spawn_wait("/bin/mv",["--",path,'''+json.dumps(str(moved))+''']).exit_code==0,"move synthetic parent");
  assert(proc_spawn_wait("/bin/ln",["-s","--",'''+json.dumps(str(replacement))+''',path]).exit_code==0,"replace synthetic parent with symlink");
 }
 return fs;
}
let open=Real.open;let sealed_child_bytes=Real.sealed_child_bytes;
let copy_held_range_new_atomic=Real.copy_held_range_new_atomic;let close=Real.close;
export open,open_absolute,sealed_child_bytes,copy_held_range_new_atomic,close;
''')
    (root / 'main.sapp').write_text('''let P=import("publication.sapp");let A=import("stdlib:secure_archive.sapp");
let bytes=A.pack([map{name:"a",data:[0,128,255]}],"store");
let r=P.save('''+json.dumps(str(parent / 'new.zip'))+''',bytes);assert(r.ok,"held parent publication");
assert(fs_read_bytes('''+json.dumps(str(moved / 'new.zip'))+''')==bytes,"held original receives exact bytes");
print "archive_parent_swap_pass";
''')
    p = subprocess.run([runtime, 'run', str(root / 'main.sapp'), '--allow-all'], cwd=repo,
                       text=True, capture_output=True, timeout=45)
    assert p.returncode == 0 and 'archive_parent_swap_pass' in p.stdout, p.stdout + p.stderr
    assert not (replacement / 'new.zip').exists() and sentinel.read_bytes() == b'keep'
    print(p.stdout.strip())
