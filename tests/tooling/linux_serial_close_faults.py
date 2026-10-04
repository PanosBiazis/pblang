"""Run the actual PBLang serial close body with synthetic FFI primitives only.

No serial device, PTY, native library, or descriptor is opened by this test.
The private source copy substitutes the two FFI primitive names; all production
restoration, aggregation, release ordering and state updates remain unchanged.
"""
import argparse
import os
from pathlib import Path
import subprocess
import tempfile


FIXTURE = r'''
let fault = map{modem:0,termios:0,calls:[]};
fn fake_ffi_call_typed(lib,name,signature,args) {
    array_push(fault.calls,name);
    if name=="calloc" { return 300; }
    if name=="ioctl" {
        if fault.modem==2 { throw "synthetic modem failure"; }
        return if fault.modem==1 {-1} else {0};
    }
    if name=="tcsetattr" {
        if fault.termios==2 { throw "synthetic termios failure"; }
        return if fault.termios==1 {-1} else {0};
    }
    return 0;
}
fn fake_ffi_close(lib) {array_push(fault.calls,"ffi_close");return nil;}
fn expect(value,label){if not value{throw "serial close fixture: "+label;}}
fn fresh(){return map{format:FORMAT,libc:10,crypto:20,fd:123,closed:false,
    original:100,working:200,configured:true,original_modem:3,modem_captured:true};}
fn reset(modem,termios){fault.modem=modem;fault.termios=termios;fault.calls=[];}
let both=["calloc","memset","memset","memset","memset","ioctl","free",
    "tcsetattr","close","free","free","ffi_close","ffi_close"];
let cases=0;
for m 3 {for t 3 {
    reset(m,t);let state=fresh();let result=close(state);
    expect(result.ok==(m==0 and t==0),"combined restoration result "+str(m)+"/"+str(t));
    expect(result.termios_restored==result.ok,"legacy result fields agree");
    expect(fault.calls==both,"all cleanup attempted after restoration failure");
    expect(state.closed and state.fd==-1 and state.original==0 and state.working==0,
        "terminal released state");
    let again=close(state);
    expect(again.ok and fault.calls==both,"repeat close does not repeat effects");
    cases=cases+1;
}}
for captured 2 {for configured 2 {
    reset(1,1);let state=fresh();state.modem_captured=captured==1;
    state.configured=configured==1;let result=close(state);
    expect(result.ok==(captured==0 and configured==0),"only required restoration contributes");
    let expected=[];
    if captured==1 {expected=["calloc","memset","memset","memset","memset","ioctl","free"];}
    if configured==1 {array_push(expected,"tcsetattr");}
    for name in ["close","free","free","ffi_close","ffi_close"] {array_push(expected,name);}
    expect(fault.calls==expected,"configuration-dependent cleanup order");cases=cases+1;
}}
reset(1,1);let unopened=fresh();unopened.fd=-1;
expect(close(unopened).ok,"no restoration without open descriptor");
expect(fault.calls==["free","free","ffi_close","ffi_close"],"unopened cleanup");cases=cases+1;
reset(1,0);let no_original=fresh();no_original.original=0;
expect(not close(no_original).ok,"modem failure retained without termios snapshot");
expect(fault.calls==["calloc","memset","memset","memset","memset","ioctl","free",
    "close","free","ffi_close","ffi_close"],"missing original cleanup");cases=cases+1;
reset(0,0);expect(close(nil).ok and len(fault.calls)==0,"nil remains harmless");cases=cases+1;
print "linux_serial_close_faults_pass "+str(cases);
'''


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--runtime", required=True, type=Path)
    parser.add_argument("--source", type=Path, default=Path(__file__).resolve().parents[2] / "stdlib/linux_serial.sapp")
    args = parser.parse_args()
    source = args.source.read_text()
    assert "fn close(state)" in source
    assert "fake_ffi_" not in source
    transformed = source.replace("ffi_call_typed(", "fake_ffi_call_typed(").replace("ffi_close(", "fake_ffi_close(")
    env = {key: value for key, value in os.environ.items() if not key.startswith("PB_PROJECT_TEST_")}
    with tempfile.TemporaryDirectory(prefix="pblang-serial-close-") as folder:
        fixture = Path(folder) / "close_faults.sapp"
        fixture.write_text(transformed + "\n" + FIXTURE)
        (Path(folder) / "pblang.toml").write_text(
            '[package]\nname = "serial-close-fixture"\nversion = "0.1.0"\nentry = "close_faults.sapp"\n')
        built = subprocess.run([str(args.runtime.resolve()), "build", folder],
            cwd=folder, env=env, capture_output=True, text=True, timeout=45)
        if built.returncode:
            print(built.stdout + built.stderr, end="")
            raise SystemExit("serial close fixture build failed")
        result = subprocess.run([str(args.runtime.resolve()), "run", str(fixture),
            "--deny-ffi", "--deny-exec", "--deny-spawn", "--deny-net", "--deny-http",
            "--deny-env", "--deny-gfx", "--deny-sqlite"], cwd=folder, env=env,
            capture_output=True, text=True, timeout=45)
        print(result.stdout, end="")
        if result.returncode or "linux_serial_close_faults_pass 16" not in result.stdout:
            print(result.stderr, end="")
            raise SystemExit("serial close fault matrix failed")


if __name__ == "__main__":
    main()
