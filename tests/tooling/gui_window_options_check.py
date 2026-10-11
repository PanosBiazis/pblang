"""Headless GUI.run window-option forwarding contract (Linux/bubblewrap).

Run from any directory with --runtime /path/to/pblang --evidence NEW_DIRECTORY.
The exact source GUI module is copied and only its graphics import is replaced.
No installed library is modified. --gui-source permits a historical negative
control; the report records source and generated-module hashes separately.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import resource
import shutil
import signal
import subprocess
import time


MOCK = '''let calls=map{windows:[],runs:[]};
fn rgb(r,g,b) {return [r,g,b];}
fn Window(w,h,title,options=nil) {
    let win=map{tag:"fake-window"};
    push(calls.windows,map{w:w,h:h,title:title,options:options,window:win});return win;
}
fn run(win,state,update,draw,fps) {
    push(calls.runs,map{window:win,state:state,update:update,draw:draw,fps:fps});
}
export rgb,Window,run,calls;
'''

FIXTURE = '''let GUI=import("stdlib:gui.sapp");
let Mock=import("stdlib:window_options_mock.sapp");
let checks=0;
fn check(ok,message) {assert(ok,message);checks=checks+1;}
fn probe(mode,options=nil) {
    let f=GUI.form("Forwarding contract",800,600);
    let layouts=map{count:0};
    GUI.set_layout(f,fn(current,profile){layouts.count=layouts.count+1;});
    let index=len(Mock.calls.windows);
    let result=if mode=="omitted" {GUI.run(f)} else {GUI.run(f,options)};
    check(result==0,"GUI.run return value");
    check(layouts.count==1,"relayout occurs once");
    check(len(Mock.calls.windows)==index+1,"one constructor call");
    check(len(Mock.calls.runs)==index+1,"one event loop call");
    let made=Mock.calls.windows[index];let loop=Mock.calls.runs[index];
    check(made.w==800 and made.h==600 and made.title=="Forwarding contract","constructor dimensions and title");
    check(loop.fps==30 and loop.state.title==f.title and loop.window.tag=="fake-window","event loop contract");
    check(type(loop.update)=="closure" and type(loop.draw)=="closure","callbacks passed");
    if options==nil {check(made.options==nil,"nil remains nil");}
    else {check(made.options.ordered_input==options.ordered_input,"ordered input forwarded exactly");}
}
probe("omitted");probe("explicit",nil);
for flag in [true,false] {
    let opts=map{ordered_input:flag,sentinel:"untouched"};
    probe("explicit",opts);
    check(len(map_keys(opts))==2 and opts.ordered_input==flag and opts.sentinel=="untouched","caller options unchanged");
}
let empty=map{};
let f=GUI.form("Empty",800,600);GUI.run(f,empty);
check(len(map_keys(empty))==0 and len(map_keys(Mock.calls.windows[4].options))==0,"empty map accepted unchanged");
for bad in [true,false,7,"yes",[]] {
    let form=GUI.form("Invalid",800,600);let changed=map{count:0};
    GUI.set_layout(form,fn(current,profile){changed.count=changed.count+1;});
    let before=len(Mock.calls.windows);let caught=try {GUI.run(form,bad);false} catch true;
    check(caught,"invalid options rejected");
    check(changed.count==0 and len(Mock.calls.windows)==before,"invalid options rejected before relayout/window");
}
for bad in [nil,0,1,"true",[],map{}] {
    let form=GUI.form("Invalid field",800,600);let changed=map{count:0};
    GUI.set_layout(form,fn(current,profile){changed.count=changed.count+1;});
    let before=len(Mock.calls.windows);let opts=map{ordered_input:bad};
    let caught=try {GUI.run(form,opts);false} catch true;
    check(caught,"invalid field rejected");
    check(changed.count==0 and len(Mock.calls.windows)==before,"invalid field rejected before relayout/window");
    check(len(map_keys(opts))==1,"invalid caller map unchanged");
}
print "gui_window_options_pass:"+str(checks);
'''


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run_bounded(command, stage, output, label):
    def limits():
        resource.setrlimit(resource.RLIMIT_FSIZE, (65536, 65536))
        resource.setrlimit(resource.RLIMIT_CORE, (0, 0))

    argv = ["bwrap", "--die-with-parent", "--unshare-net", "--unshare-pid",
            "--ro-bind", "/", "/", "--bind", str(stage), str(stage),
            "--proc", "/proc", "--dev", "/dev", "--chdir", str(stage), "--", *command]
    start = time.monotonic()
    with (output / (label + ".stdout")).open("xb") as stdout, \
            (output / (label + ".stderr")).open("xb") as stderr:
        child = subprocess.Popen(argv, stdout=stdout, stderr=stderr,
                                 start_new_session=True, preexec_fn=limits)
        timed_out = False
        try:
            child.wait(timeout=30)
        except subprocess.TimeoutExpired:
            timed_out = True
        finally:
            try:
                os.killpg(child.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            child.wait(timeout=3)
    return dict(exit=child.returncode, timed_out=timed_out,
                seconds=time.monotonic() - start,
                stdout=(output / (label + ".stdout")).read_text(errors="replace"),
                stderr=(output / (label + ".stderr")).read_text(errors="replace"))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--runtime", type=Path, required=True)
    parser.add_argument("--evidence", type=Path, required=True)
    parser.add_argument("--gui-source", type=Path)
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[2]
    source = (args.gui_source or repo / "stdlib/gui.sapp").resolve()
    if not shutil.which("bwrap"):
        parser.error("bubblewrap is required for isolated offline execution")
    text = source.read_text()
    old = 'let gfx  = import("stdlib/graphics.sapp");'
    if text.count(old) != 1:
        parser.error("expected exactly one original graphics import; source not transformed")
    args.evidence.mkdir(mode=0o700)
    output = args.evidence.resolve()
    stage = output / "stage"
    stage.mkdir(mode=0o700)
    shutil.copytree(repo / "stdlib", stage / "stdlib")
    transformed = text.replace(old, 'let gfx  = import("stdlib:window_options_mock.sapp");')
    (stage / "stdlib/gui.sapp").write_text(transformed)
    (stage / "stdlib/window_options_mock.sapp").write_text(MOCK)
    (stage / "check.sapp").write_text(FIXTURE)
    (stage / "pblang.toml").write_text('[package]\nname="gui-window-options-test"\nversion="0.0.0"\n[dependencies]\nstdlib={path="stdlib"}\n')
    runtime = args.runtime.resolve()
    report = dict(source_sha256=digest(source), transformed_sha256=digest(stage / "stdlib/gui.sapp"),
                  fixture_sha256=digest(stage / "check.sapp"), runtime_launcher_sha256=digest(runtime),
                  import_replacements=1, scope="headless forwarding; not native input acceptance")
    report["install"] = run_bounded([str(runtime), "install"], stage, output, "install")
    if report["install"]["exit"] == 0 and not report["install"]["timed_out"]:
        report["check"] = run_bounded([str(runtime), "run", "check.sapp", "--allow-read",
                                      "--deny-exec", "--deny-net", "--deny-http", "--deny-spawn"],
                                     stage, output, "check")
    result = report.get("check", {})
    report["passed"] = (result.get("exit") == 0 and not result.get("timed_out", True)
                        and result.get("stderr") == ""
                        and result.get("stdout") == "gui_window_options_pass:63\n")
    (output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(dict(passed=report["passed"], report=str(output / "report.json"))))
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
