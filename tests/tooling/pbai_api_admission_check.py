"""Bounded Linux qualification of the actual one-shot request-file bridge.

Run serially under the coordinator's external heavy lock. All state is temporary;
the tested command never acquires permissions on behalf of an operator.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import socket
import subprocess
import tempfile

LIMIT = 983040
SECRET = "synthetic-admission-private-marker"
DENIED = ["--deny-" + x for x in ("net", "http", "spawn", "gfx", "sqlite")]


def execute(command, cwd, env):
    child = subprocess.Popen(command, cwd=cwd, env=env, stdout=subprocess.PIPE,
                             stderr=subprocess.PIPE, text=True, start_new_session=True)
    try:
        out, err = child.communicate(timeout=45)
    except (subprocess.TimeoutExpired, KeyboardInterrupt):
        try:
            os.killpg(child.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        child.communicate()
        raise RuntimeError("bounded admission test timed out") from None
    return {"exit": child.returncode, "stdout": out, "stderr": err}


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--product", choices=["pbai", "pblang"], required=True)
    ap.add_argument("--baseline-ref", required=True)
    args = ap.parse_args()
    root = Path(__file__).resolve().parents[2]
    provider = root if args.product == "pblang" else root.parent / "pblang"
    relative = Path("stdlib/ai/pbai") if args.product == "pblang" else Path(".")
    owner = root / relative
    runtime = str(Path(os.environ["PBLANG_BIN"]).resolve(strict=True))
    source = (owner / "tools/pbai_api.sapp").read_text()
    helper = (owner / "tools/pbai_api_request.sapp").read_text()
    baseline = subprocess.check_output(
        ["git", "show", f"{args.baseline_ref}:{relative / 'tools/pbai_api.sapp'}"],
        cwd=root, text=True)
    temp = Path(tempfile.mkdtemp(prefix="pbai-admission-check-"))
    temp.chmod(0o700)
    results = []
    sock = None
    passed = False
    pure = {}
    try:
        files = temp / "requests"
        files.mkdir(mode=0o700)
        state = temp / "state"
        state.mkdir(mode=0o700)
        env = os.environ.copy()
        env["PBAI_STATE_ROOT"] = str(state)
        env["PBAI_ADMISSION_FIXTURES"] = str(files)
        embedded = temp / "pblang"
        shutil.copytree(provider / "stdlib", embedded / "stdlib")
        (embedded / "pblang.toml").write_text(
            '[package]\nname="admission-provider"\nversion="0.1.0"\n'
            'entry="main.sapp"\n[dependencies]\nstdlib={path="stdlib"}\n')
        (embedded / "main.sapp").write_text('print "provider";\n')
        staged_root = temp / args.product
        if args.product == "pbai":
            shutil.copytree(root / "stdlib", staged_root / "stdlib")
            (staged_root / "pblang.toml").write_text(
                '[package]\nname="admission-product"\nversion="0.1.0"\n'
                'entry="tools/pbai_api.sapp"\n[dependencies]\n'
                'stdlib={path="../pblang/stdlib"}\n')
        staged = staged_root
        (staged / "tools").mkdir(parents=True, exist_ok=True)
        (staged / "tools/pbai_api.sapp").write_text(source)
        (staged / "tools/pbai_api_request.sapp").write_text(helper)
        (staged / "pure.sapp").write_text(
            (owner / "tests/semantics/pbai_api_request_test.sapp").read_text())

        def command(cwd, path="tools/pbai_api.sapp", argv=(), hidden=True, deny=()):
            cmd = ["/usr/bin/bwrap", "--die-with-parent", "--unshare-net", "--ro-bind", "/", "/",
                   "--dev", "/dev", "--bind", str(temp), str(temp)]
            if hidden:
                for original in sorted({str(root), str(provider)}):
                    cmd += ["--tmpfs", original]
            return cmd + ["--chdir", str(cwd), runtime, "run", path,
                          "--allow-all", *DENIED, *deny, "--", *map(str, argv)]

        def run(label, cwd=staged, path="tools/pbai_api.sapp", argv=(), hidden=True,
                deny=(), rejection=False):
            value = execute(command(cwd, path, argv, hidden, deny), cwd, env)
            assert value["exit"] == 0, (label, value)
            rows = [json.loads(x) for x in value["stdout"].splitlines() if x.strip()]
            assert len(rows) == 1, (label, value)
            data = rows[0]
            if rejection:
                assert data["ok"] is False and data["action"] == data["command"] == "request", (label, data)
                assert data["value"] is None and isinstance(data["error"], str), (label, data)
                assert len(value["stdout"].encode()) < 256, (label, data)
                assert SECRET not in value["stdout"] + value["stderr"], (label, data)
                assert str(files) not in value["stdout"] + value["stderr"], (label, data)
            results.append({"case": label, "exit": 0, "rejected": rejection,
                            "source_hidden": hidden, "network_denied": True})
            return data

        def refresh():
            for project in [embedded] + ([staged_root] if args.product == "pbai" else []):
                built = execute([runtime, "build"], project, env)
                assert built["exit"] == 0, built
        refresh()
        malformed = files / "malformed.json"
        malformed.write_bytes(b'{"prompt":')
        good = files / "valid.json"
        good.write_text('{"prompt":"Ελληνικά"}')
        token_results = {}
        for place, cwd, hidden in [("source", root, False), ("staged", staged, True)]:
            bridge = str(relative / "tools/pbai_api.sapp") if place == "source" else "tools/pbai_api.sapp"
            run(place + "-malformed-learn", cwd, path=bridge, argv=("learn", malformed), hidden=hidden, rejection=True)
            run(place + "-malformed-tool", cwd, path=bridge, argv=("tool_call", malformed), hidden=hidden, rejection=True)
            data = run(place + "-omitted-status", cwd, path=bridge, argv=("status",), hidden=hidden,
                       deny=("--deny-exec", "--deny-ffi"))
            assert data["ok"] is True and data["action"] == "status", data
            data = run(place + "-valid-tokenize", cwd, path=bridge, argv=("tokenize", good), hidden=hidden)
            assert data["ok"] is True and data["action"] == "tokenize", data
            token_results[place] = data
        assert token_results["source"] == token_results["staged"]
        pure = run("pure-contracts", path="pure.sapp", deny=("--deny-exec", "--deny-ffi"))
        assert pure["checks"] >= 20

        # Probe the real bridge with inert product modules; no model/training calls.
        (staged / "stdlib").mkdir(exist_ok=True)
        stub = '''fn model_status(){return map{probe:true};}
fn learn(prompt,epochs){return map{dispatch:true};}
export model_status,learn;
'''
        def model_stub(filename, content):
            (staged / "stdlib" / filename).write_text(content)
            if args.product == "pblang":
                (staged / relative / "stdlib" / filename).write_text(content)
        for filename, content in [("pbai.sapp", stub), ("pbai_tokenizer.sapp", ""), ("pbai_brain.sapp", "")]:
            model_stub(filename, content)
        refresh()
        forged = files / "forged.json"
        forged.write_text(json.dumps({"format": "pbai.api.invalid-request.v1", "error": SECRET}))
        (staged / "tools/baseline.sapp").write_text(baseline)
        if args.product == "pbai":
            data = run("baseline-forged-error", path="tools/baseline.sapp", argv=("status", forged))
            assert data["error"] == SECRET, data
        else:
            data = run("baseline-malformed-dispatch", path="tools/baseline.sapp", argv=("learn", malformed))
            assert data["ok"] and data["value"]["dispatch"], data
        data = run("forged-fields-are-data", argv=("status", forged))
        assert data["ok"] and data["value"]["probe"], data
        # An import tripwire makes rejection-before-model-loading observable.
        model_stub("pbai.sapp", 'throw "MODEL_IMPORT_MUST_NOT_RUN";\n')
        refresh()
        invalid = {
            "empty": b"", "one-byte": b"{", "array": b"[]", "null": b"null",
            "scalar": b"1", "trailing": b"{}{}", "overflow": b'{"x":1e999}',
            "utf8": b'{"x":"\xff"}', "utf8-overlong": b'{"x":"\xc0\xaf"}',
            "utf8-surrogate": b'{"x":"\xed\xa0\x80"}', "utf8-truncated": b'{"x":"\xe2\x82"}',
            "bom": b"\xef\xbb\xbf{}", "depth33": (b'{"x":' * 32) + b"{}" + b"}" * 32,
            "overlimit": b"{}" + b" " * (LIMIT - 1),
            "unicode-overlimit": ('{"x":"' + 'α' * (LIMIT // 2) + '"}').encode(),
        }
        for label, payload in invalid.items():
            file = files / (label + ".json")
            file.write_bytes(payload)
            before = hashlib.sha256(payload).hexdigest()
            run(label, argv=("learn", file), rejection=True)
            assert hashlib.sha256(file.read_bytes()).hexdigest() == before
        unreadable = files / "unreadable.json"
        unreadable.write_text("{}")
        unreadable.chmod(0)
        if os.geteuid() != 0:
            run("unreadable", argv=("learn", unreadable), rejection=True)
        unreadable.chmod(0o600)
        sparse = files / "sparse.json"
        with sparse.open("wb") as stream:
            stream.truncate(LIMIT * 8)
        run("oversized-sparse", argv=("learn", sparse), rejection=True)
        run("unknown-action-private", argv=(SECRET, malformed), rejection=True)
        run("oversized-action-private", argv=(SECRET * 512, malformed), rejection=True)
        run("missing", argv=("tool_call", files / "missing.json"), rejection=True)
        (files / "final-link.json").symlink_to(good)
        (files / "parent-link").symlink_to(files, target_is_directory=True)
        (files / "broken-link").symlink_to(files / "absent")
        os.mkfifo(files / "fifo")
        sock = socket.socket(socket.AF_UNIX)
        sock.bind(str(files / "socket"))
        for label, path in [("final-link", files / "final-link.json"),
                            ("parent-link", files / "parent-link/valid.json"),
                            ("broken-link", files / "broken-link"), ("directory", files),
                            ("fifo", files / "fifo"), ("socket", files / "socket"),
                            ("device-null", Path("/dev/null"))]:
            run(label, argv=("tool_call", path), rejection=True)
        reserved = staged / "root"
        reserved.mkdir(exist_ok=True)
        (reserved / "file.json").write_text("{}")
        for path in ["root", "root/file.json", "./root/file.json"]:
            run("reserved-" + path, argv=("learn", path), rejection=True)
        for permission in ["--deny-ffi", "--deny-exec"]:
            run(permission, argv=("learn", good), deny=(permission,), rejection=True)
        # A byte-exact boundary succeeds; unknown payload keys remain compatible.
        model_stub("pbai.sapp", stub)
        refresh()
        for label, payload in [
            ("limit-minus-one", b"{}" + b" " * (LIMIT - 3)),
            ("exact-limit", b"{}" + b" " * (LIMIT - 2)),
            ("depth32", (b'{"x":' * 31) + b"{}" + b"}" * 31),
        ]:
            file = files / (label + ".json")
            file.write_bytes(payload)
            data = run(label, argv=("status", file))
            assert data["ok"] and data["value"]["probe"], data

        # Faults are injected only in a private helper dependency copy.
        fake = '''let counts=map{opens:0,reads:0,closes:0,status:0};
fn scan_mount_admission_status(){counts.status=counts.status+1;
 let mode=env_get("PBAI_ADMISSION_FAULT");
 let poison=mode=="poison-before" or (mode=="poison-after" and counts.closes>0);
 return map{ready:not poison,process_admission_poisoned:poison};}
fn open(parent,create,protect){counts.opens=counts.opens+1;
 if env_get("PBAI_ADMISSION_FAULT")=="open-fail" {return map{ok:false};}
 return map{ok:true};}
fn open_absolute(parent,create,protect){return open(parent,create,protect);}
fn read_prefix_bytes(handle,name,limit){counts.reads=counts.reads+1;
 let mode=env_get("PBAI_ADMISSION_FAULT");
 if mode=="read-throw" {throw "synthetic-private-reader-error";}
 if mode=="read-fail" {return map{ok:false};}
 let result=map{ok:true,bytes:[123,125],size:2,source_size:2,truncated:false,
 kind:"file",descriptor_bound_metadata:true};
 if mode=="size" {result.source_size=3;}
 if mode=="truncated" {result.truncated=true;}
 if mode=="kind" {result.kind="other";}
 return result;}
fn close(handle){counts.closes=counts.closes+1;let mode=env_get("PBAI_ADMISSION_FAULT");
 if mode=="close-throw" {throw "synthetic-private-close-error";}
 if mode=="close-false" {return false;}
 if mode=="close-nonbool" {return 1;}
 return true;}
fn observed(){return counts;}
export scan_mount_admission_status,open,open_absolute,read_prefix_bytes,close,observed;
'''
        (staged / "tools/request_fake_dir.sapp").write_text(fake)
        injected = helper.replace('import("stdlib:linux_dirfs.sapp")', 'import("tools/request_fake_dir.sapp")')
        assert injected != helper
        (staged / "tools/request_fault.sapp").write_text(injected)
        (staged / "fault.sapp").write_text('''let R=import("tools/request_fault.sapp");
let F=import("tools/request_fake_dir.sapp");
let result=R.read_request("request.json");
print json_stringify(map{accepted:result.ok,counts:F.observed()});
''')
        for mode in ["success", "open-fail", "poison-before", "poison-after", "read-throw", "read-fail",
                     "close-throw", "close-false", "close-nonbool", "size", "truncated", "kind"]:
            env["PBAI_ADMISSION_FAULT"] = mode
            data = run("fault-" + mode, path="fault.sapp", deny=("--deny-exec", "--deny-ffi"))
            assert data["accepted"] == (mode == "success"), data
            assert data["counts"]["closes"] == (0 if mode in {"open-fail", "poison-before"} else 1), data
        # Backend import tripwire proves the omitted-file fast path precedes import.
        (staged / "tools/request_fake_dir.sapp").write_text('throw "BACKEND_MUST_NOT_LOAD";\n')
        (staged / "empty.sapp").write_text('let R=import("tools/request_fault.sapp");print json_stringify(R.read_request(""));\n')
        assert run("omitted-backend-tripwire", path="empty.sapp", deny=("--deny-exec", "--deny-ffi"))["ok"]
        assert run("explicit-empty-filename", argv=("status", ""), deny=("--deny-exec", "--deny-ffi"))["ok"]
        unsupported = helper.replace('platform()!="linux"', 'true')
        (staged / "tools/request_platform.sapp").write_text(unsupported)
        (staged / "platform.sapp").write_text('let R=import("tools/request_platform.sapp");print json_stringify(R.read_request("request.json"));\n')
        assert not run("unsupported-platform-fixture", path="platform.sapp", deny=("--deny-exec", "--deny-ffi"))["ok"]
        passed = True
    finally:
        if sock is not None:
            sock.close()
        shutil.rmtree(temp)
        print(json.dumps({"format": "pbai.api.admission-check.v1", "product": args.product,
                          "cases": results, "pure_assertions": pure.get("checks"),
                          "passed": passed, "private_fixtures_removed": not temp.exists(),
                          "physical_memory_release_verified": False}))


if __name__ == "__main__":
    main()
