"""Optional-read ownership, absence and shape checks with a retained runtime.

Complete private facade substitutions model dispatch failures. Native Linux
fixtures separately check real descriptors, listings and unchanged inputs.
Windows routing is modeled only. Caller holds the coordinator's shared lock.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
from secure_file_prefix_cleanup_check import execute, digest
from secure_file_exclusive_close_check import replace_once

BACKEND = '''
let state=map{};
fn configure(list_mode,read_mode,close_mode) {
 state.list_mode=list_mode;state.read_mode=read_mode;state.close_mode=close_mode;
 state.opens=0;state.lists=0;state.reads=0;state.closes=0;state.admit=true;
}
fn open(path,create=false,protect=false) {state.opens=state.opens+1;return map{ok:state.admit};}
fn open_absolute(path,create=false,protect=false) {return open(path,create,protect);}
fn close(fs) {
 state.closes=state.closes+1;let m=state.close_mode;
 if m=="throw" {throw "synthetic-private-value";} if m=="nil" {return nil;}
 if m=="number" {return 1;} if m=="map" {return map{ok:true};} return m=="success";
}
fn list(fs,path,maximum) {
 state.lists=state.lists+1;let m=state.list_mode;
 if m=="throw" {throw "synthetic-private-value";} if m=="nil" {return nil;}
 if m=="failure" {return map{ok:false,error:"synthetic failure"};}
 if m=="bad_ok" {return map{ok:1,entries:[],truncated:false};}
 if m=="bad_entries" {return map{ok:true,entries:map{},truncated:false};}
 if m=="bad_entry" {return map{ok:true,entries:[map{}],truncated:false};}
 if m=="oversize_entries" {let entries=[];for i 4097 {push(entries,"x");}return map{ok:true,entries:entries,truncated:false};}
 if m=="absent" {return map{ok:true,entries:[],truncated:false};}
 if m=="truncated_absent" {return map{ok:true,entries:[],truncated:true};}
 if m=="missing_truncated_absent" {return map{ok:true,entries:[]};}
 if m=="bad_truncated_absent" {return map{ok:true,entries:[],truncated:0};}
 if m=="missing_truncated_present" {return map{ok:true,entries:["input"]};}
 return map{ok:true,entries:["input"],truncated:m=="truncated_present"};
}
fn read_bytes(fs,name,maximum) {
 state.reads=state.reads+1;let m=state.read_mode;
 if m=="throw" {throw "synthetic-private-value";} if m=="nil" {return nil;}
 if m=="failure" {return map{ok:false,error:"synthetic failure",marker:7};}
 if m=="missing" {return map{ok:false,error:"synthetic failure",marker:7,missing:true};}
 if m=="bad_missing" {return map{ok:false,error:"synthetic failure",marker:7,missing:"true"};}
 if m=="bad_ok" {return map{ok:1,bytes:[65],size:1};}
 if m=="missing_bytes" {return map{ok:true,size:1};}
 if m=="missing_size" {return map{ok:true,bytes:[65]};}
 if m=="empty" {return map{ok:true,bytes:[],size:0};}
 if m=="invalid_utf8" {return map{ok:true,bytes:[255],size:1};}
 if m=="over_cap" {return map{ok:true,bytes:[65,65,65,65,65,65,65,65,65],size:9};}
 let bytes=if m=="bad_bytes" {"α"} else {[206,177]};let size=2;
 if m=="bad_size" {size="2";} if m=="fraction" {size=2.5;} if m=="negative" {size=0-1;}
 if m=="nan" {size=num_sqrt(0-1);} if m=="infinity" {size=num_exp(1000);} if m=="mismatch" {size=1;}
 return map{ok:true,bytes:bytes,size:size};
}
export state,configure,open,open_absolute,close,list,read_bytes;
'''


def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--runtime',required=True,type=Path)
    ap.add_argument('--baseline-source',required=True,type=Path)
    ap.add_argument('--report',required=True,type=Path)
    ap.add_argument('--consumer-root',type=Path,help='optional PBJobTracker source root for actual load checks')
    args=ap.parse_args();os.umask(0o077)
    args.report.parent.mkdir(parents=True,exist_ok=True)
    with args.report.open('x') as out:out.write('{}\n')
    runtime=args.runtime.resolve();root=Path(__file__).resolve().parents[2]
    raw=(root/'stdlib/secure_file.sapp').read_text();old=args.baseline_source.read_text()
    semantic=(root/'tests/semantics/secure_file_optional_cleanup_test.sapp').read_text()
    rows=[];pins={name:digest(runtime/name) for name in ['bin/pblang','bin/pblang-native','pblang.lock',
        'stdlib/secure_file.sapp','stdlib/linux_dirfs.sapp','stdlib/linux_dirfs_read_core.sapp']}
    report=dict(passed=False,rows=rows,runtime_pins=pins,source_sha256=hashlib.sha256(raw.encode()).hexdigest(),
        baseline_sha256=digest(args.baseline_source),harness_sha256=digest(Path(__file__)),
        semantic_sha256=hashlib.sha256(semantic.encode()).hexdigest(),
        helper_sha256={n:digest(root/'tests/tooling'/n) for n in ['secure_file_prefix_cleanup_check.py',
            'secure_file_exclusive_close_check.py','check_cli_exact_argv.py']},
        source_hidden=True,network_isolated=True,output_cap_bytes=262144,child_seconds=15,
        native_fault_injected=False,windows_native_verified=False)
    def save():args.report.write_text(json.dumps(report,indent=2)+'\n')
    if pins['stdlib/secure_file.sapp']!=report['source_sha256']:
        report['admission_error']='runtime facade does not match reviewed source';save();raise SystemExit(1)
    save()
    def record(label,files,expected,real=False):
        result=execute(runtime,files,real)
        good=result['exit']==0 and result['stderr']=='' and result['failure'] is None
        good=good and not result['group_survived_normal_exit'] and result['inputs_unchanged']
        if good:good=json.loads(result['stdout'])==expected
        rows.append(dict(label=label,passed=bool(good),result=result,
            fixture_sha256={n:hashlib.sha256(v if isinstance(v,bytes) else v.encode()).hexdigest() for n,v in files.items()}))
        save();print(label,good,flush=True)
        if not good:raise AssertionError(rows[-1])
    model=replace_once(raw,'let LinuxDirFS = import("stdlib:linux_dirfs.sapp");','let LinuxDirFS = import("backend.sapp");')
    for windows in [False,True]:
        source=model
        if windows:
            start=source.index('fn _read_optional_owned(');stop=source.index('\nfn read_optional_text(',start)
            part=source[start:stop].replace('platform() != "windows"','false').replace('platform() == "windows"','true')
            source=source[:start]+part+source[stop:]
        main='let S=import("facade.sapp");let B=import("backend.sapp");let T=import("semantic.sapp");print json_stringify(T.run(S,B,'+str(windows).lower()+'));'
        record('modeled '+('Windows routing' if windows else 'POSIX routing'),{'facade.sapp':source,'backend.sapp':BACKEND,'semantic.sapp':semantic,'main.sapp':main},
            dict(ok=True,matrix_cases=114 if windows else 198,admission_cases=6,windows_model=windows))
    old_model=replace_once(old,'let LinuxDirFS = import("stdlib:linux_dirfs.sapp");','let LinuxDirFS = import("backend.sapp");')
    record('old source rejected by ownership contract',{'facade.sapp':old_model,'backend.sapp':BACKEND,'semantic.sapp':semantic,
        'main.sapp':'let S=import("facade.sapp");let B=import("backend.sapp");let T=import("semantic.sapp");let rejected=try {T.run(S,B);false} catch true;print json_stringify(map{rejected:rejected});'},dict(rejected=True))
    normal='''let S=import("facade.sapp");let before=len(fs_listdir("/proc/self/fd"));for i 4 {
let r=S.read_optional_text("scratch/input",5);assert(r.ok and not r.missing and r.text=="A€Z" and r.size==5,"exact UTF8");
assert(not S.read_optional_text("scratch/input",4).ok,"over byte limit");
let empty=S.read_optional_text("scratch/empty",0);assert(empty.ok and not empty.missing and empty.text=="" and empty.size==0,"empty");
let missing=S.read_optional_text("scratch/missing",0);assert(missing.ok and missing.missing,"proven missing");
assert(not S.read_optional_text("scratch/invalid",4).ok,"malformed UTF8");
}print json_stringify(map{ok:true,checks:20,fd_delta:len(fs_listdir("/proc/self/fd"))-before});'''
    inputs={'scratch/input':'A€Z','scratch/empty':'','scratch/invalid':b'\xff'}
    for label,facade in [('candidate',raw),('baseline',old)]:
        record(label+' real normal and missing control',dict(inputs,**{'facade.sapp':facade,'main.sapp':normal}),dict(ok=True,checks=20,fd_delta=0),True)
        for branch in ['list','read']:
            call='LinuxDirFS.list(fs, path, max_entries)' if branch=='list' else 'LinuxDirFS.read_bytes(fs, name, max_bytes)'
            source=replace_once(facade,'if platform() == "linux" { return '+call+'; }','if platform() == "linux" { throw "synthetic-private-value"; }')
            main='let S=import("facade.sapp");let before=len(fs_listdir("/proc/self/fd"));let exceptions=0;let denied=0;for i 4 {let r=try S.read_optional_text("scratch/input",8) catch {exceptions=exceptions+1;nil};if r!=nil {assert(not r.ok and not map_has(r,"text") and not map_has(r,"bytes"),"no data");denied=denied+1;}}print json_stringify(map{exceptions:exceptions,denied:denied,fd_delta:len(fs_listdir("/proc/self/fd"))-before});'
            record(label+' real parent '+branch+' exception',{'facade.sapp':source,'main.sapp':main,'scratch/input':'A'},dict(exceptions=4 if label=='baseline' else 0,denied=0 if label=='baseline' else 4,fd_delta=4 if label=='baseline' else 0),True)
    for mode in ['false','throw']:
        source=replace_once(raw,'if platform() == "linux" { return LinuxDirFS.close(fs); }',
            'if platform() == "linux" { LinuxDirFS.close(fs); '+('return false;' if mode=='false' else 'throw "synthetic-private-value";')+' }')
        main='let S=import("facade.sapp");let before=len(fs_listdir("/proc/self/fd"));for i 4 {for path in ["scratch/input","scratch/missing"] {let r=S.read_optional_text(path,8);assert(not r.ok and r.cleanup_uncertain and not r.parent_close_confirmed and len(map_keys(r))==4,"no success on uncertainty");}}print json_stringify(map{ok:true,checks:8,fd_delta:len(fs_listdir("/proc/self/fd"))-before});'
        record('real close followed by '+mode,{'facade.sapp':source,'main.sapp':main,'scratch/input':'A'},dict(ok=True,checks=8,fd_delta=0),True)
    # Actual omitted entry is discovered from the real backend's returned set;
    # no assumption about OS directory ordering and no production substitution.
    trunc_main='''let S=import("facade.sapp");let D=import("stdlib:linux_dirfs.sapp");let before=len(fs_listdir("/proc/self/fd"));
let d=D.open("scratch",false);assert(d.ok,"open");let listing=D.list(d,"root",4096);assert(listing.ok,"list");assert(D.close(d),"close");
let all=fs_listdir("scratch");let included=map{};for name in listing.entries {included[name]=true;}
let omitted="";for name in all {if not map_has(included,name) {omitted=name;}}
let present=S.read_optional_text("scratch/"+listing.entries[0],8192);assert(present.ok and not present.missing,"found target retained");
let missing=S.read_optional_text("scratch/absent",8192);let unlisted=if omitted!="" {S.read_optional_text("scratch/"+omitted,8192)} else {map{ok:false}};
print json_stringify(map{entries:len(all),truncated:listing.truncated,missing_ok:missing.ok,missing:try missing.missing catch false,
unlisted_ok:unlisted.ok,unlisted_missing:try unlisted.missing catch false,fd_delta:len(fs_listdir("/proc/self/fd"))-before});'''
    for count in [4096,4097]:
        for label,facade in [('candidate',raw),('baseline',old)]:
            files={f'scratch/entry-{i:04}.txt':'A' for i in range(count)};files.update({'facade.sapp':facade,'main.sapp':trunc_main})
            record(label+' actual '+str(count)+' listing',files,dict(entries=count,truncated=count==4097,
                missing_ok=count==4096 or label=='baseline',missing=count==4096 or label=='baseline',
                unlisted_ok=count==4097 and label=='baseline',unlisted_missing=count==4097 and label=='baseline',fd_delta=0),True)
    if args.consumer_root:
        croot=args.consumer_root.resolve()
        consumer={n:(croot/'src'/n).read_text() for n in ['storage.sapp','service.sapp','policy.sapp']}
        report['consumer_pins']={n:hashlib.sha256(v.encode()).hexdigest() for n,v in consumer.items()};save()
        consumer['storage.sapp']=replace_once(consumer['storage.sapp'],'import("stdlib:secure_file.sapp")','import("facade.sapp")')
        state=json.dumps(dict(format='pbjobtracker.state.v2',records=[dict(format='pbjobtracker.application.v1',id=1,owner_id='fixture-owner',revision=1,company_name='Example',position='Test',status='planned')],audit_events=[]))
        for label,facade in [('candidate',raw),('baseline',old)]:
            for mode in ['normal','list_throw','read_throw','truncated']:
                source=facade
                if mode.endswith('_throw'):
                    call='LinuxDirFS.list(fs, path, max_entries)' if mode=='list_throw' else 'LinuxDirFS.read_bytes(fs, name, max_bytes)'
                    source=replace_once(source,'if platform() == "linux" { return '+call+'; }','if platform() == "linux" { throw "synthetic-private-value"; }')
                elif mode=='truncated':source=replace_once(source,'if platform() == "linux" { return LinuxDirFS.list(fs, path, max_entries); }','if platform() == "linux" { return map{ok:true,entries:[],truncated:true}; }')
                main='let C=import("storage.sapp");let before=len(fs_listdir("/proc/self/fd"));let exceptions=0;let denied=0;let records=-1;let missing=false;for i 4 {let r=try C.load("scratch/state.json") catch {exceptions=exceptions+1;nil};if r!=nil {if r.ok {records=len(r.state.records);missing=r.missing;} else {denied=denied+1;}}}print json_stringify(map{exceptions:exceptions,denied:denied,records:records,missing:missing,fd_delta:len(fs_listdir("/proc/self/fd"))-before});'
                fault=mode.endswith('_throw');bad=label=='baseline'
                expected=dict(exceptions=4 if fault and bad else 0,denied=4 if mode!='normal' and not bad else 0,
                    records=1 if mode=='normal' else (0 if mode=='truncated' and bad else -1),missing=mode=='truncated' and bad,fd_delta=4 if fault and bad else 0)
                record(label+' PBJobTracker '+mode,dict(consumer,**{'facade.sapp':source,'main.sapp':main,'scratch/state.json':state}),expected,True)
    assert all(digest(runtime/n)==h for n,h in pins.items()),'runtime changed'
    report['passed']=all(row['passed'] for row in rows);report['runtime_unchanged']=True;save()

if __name__=='__main__':main()
