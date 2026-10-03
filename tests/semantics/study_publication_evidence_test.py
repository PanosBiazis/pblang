"""Wrapper error-projection model. Backend outcomes are simulated, not filesystem evidence."""
from pathlib import Path
import subprocess,sys,tempfile
repo=Path(__file__).resolve().parents[2]
source=(repo/'stdlib/study_publication.sapp').read_text()
assert source.count('import("stdlib:linux_dirfs.sapp")')==1
with tempfile.TemporaryDirectory(prefix='study-publication-model-') as temp:
 root=Path(temp)
 (root/'publication.sapp').write_text(source.replace('import("stdlib:linux_dirfs.sapp")','import("dir.sapp")'))
 (root/'dir.sapp').write_text('''let state=map{mode:"",closes:0,copies:0};
fn configure(mode) {state.mode=mode;state.closes=0;state.copies=0;}
fn open(path,a,b) {return map{ok:true};} fn open_absolute(path,a,b) {return open(path,a,b);}
fn sealed_child_bytes(fs,name,bytes) {return map{ok:state.mode!="seal_failure"};}
fn copy_held_range_new_atomic(a,b,c,d,e,f,g,h) {
 state.copies=state.copies+1;
 if state.mode=="throws" {throw "SECRET_RAW_BACKEND";}
 if state.mode=="ambiguous" {return map{ok:false,error:"SECRET_RAW_BACKEND"};}
 if state.mode=="unsynced" or state.mode=="both" {return map{ok:false,published:true,durability_verified:false,error:"SECRET_RAW_BACKEND"};}
 return map{ok:true,atomic_visibility:true,durability_verified:true};
}
fn close(fs) {state.closes=state.closes+1;return state.mode!="cleanup" and state.mode!="both";}
export configure,open,open_absolute,sealed_child_bytes,copy_held_range_new_atomic,close,state;
''')
 (root/'main.sapp').write_text('''let D=import("dir.sapp");let P=import("publication.sapp");
for mode in ["success","seal_failure","ambiguous","throws","unsynced","cleanup","both"] {
 D.configure(mode);let r=P.save("target.pbdeck","x");assert(D.state.closes==1,"one root close, no numeric retry");
 assert(not r.partial_output_possible and not str_contains(json_stringify(r),"SECRET_RAW_BACKEND"),"no partial claim or raw backend disclosure");
 if mode=="seal_failure" {assert(not r.ok and not r.written and D.state.copies==0,"before publication");}
 elif mode=="ambiguous" or mode=="throws" {assert(not r.ok and not map_has(r,"written") and r.output_may_exist and not r.publication_confirmed,"ambiguous outcomes never claim absent output");}
 else {assert(r.written and r.publication_confirmed and r.atomic_visibility,"positive evidence retained");}
 if mode=="cleanup" or mode=="both" {assert(not r.ok and not r.owned_handles_closed and map_has(r,"cleanup_error"),"cleanup failure preserved alongside publication");}
 if mode=="unsynced" or mode=="both" {assert(not r.durability_verified and r.code=="publication_unconfirmed","original durability failure preserved");}
 if mode=="success" {assert(r.ok and r.durability_verified and r.owned_handles_closed,"success requires all outcomes");}
}
print "study_publication_evidence_model_7_pass";
''')
 r=subprocess.run([str(Path(sys.argv[1]).resolve()),'run',str(root/'main.sapp'),'--allow-all'],cwd=repo,text=True,capture_output=True,timeout=45)
 print(r.stdout+r.stderr);assert r.returncode==0
