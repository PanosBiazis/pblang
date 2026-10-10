"""Actual isolated checker processes: filename identity, exits and profile parity.

Requires independently prepared baseline/candidate installed-shaped releases.
No installation or package editing is performed. The caller serializes this
finite suite under the shared test lock. Only disposable synthetic inputs are
writable. --baseline records the expected old failures instead of hiding them.
"""
from pathlib import Path
import argparse
import hashlib
import json
import os
import resource
import selectors
import signal
import subprocess
import tempfile
import time


OUTPUT_CAP = 262144
CHILD_SECONDS = 15


def bounded_process(command):
    """Bound captured bytes while reading, and reap the complete owned group."""
    def limits():
        resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
        resource.setrlimit(resource.RLIMIT_AS, (768 * 1024**2, 768 * 1024**2))
        resource.setrlimit(resource.RLIMIT_CPU, (CHILD_SECONDS, CHILD_SECONDS))

    start = time.monotonic()
    proc = subprocess.Popen(command, stdin=subprocess.DEVNULL,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                            start_new_session=True, preexec_fn=limits)
    selector = selectors.DefaultSelector()
    chunks = {"stdout": bytearray(), "stderr": bytearray()}
    failure = None
    remaining = False
    try:
        for stream, name in [(proc.stdout, "stdout"), (proc.stderr, "stderr")]:
            os.set_blocking(stream.fileno(), False)
            selector.register(stream, selectors.EVENT_READ, name)
        while selector.get_map():
            if time.monotonic() - start > CHILD_SECONDS:
                failure = "deadline"
                break
            for key, _ in selector.select(0.05):
                data = os.read(key.fileobj.fileno(), 8192)
                if not data:
                    selector.unregister(key.fileobj)
                    continue
                used = sum(len(value) for value in chunks.values())
                if used + len(data) > OUTPUT_CAP:
                    failure = "output_cap"
                    break
                chunks[key.data].extend(data)
            if failure:
                break
        if failure:
            os.killpg(proc.pid, signal.SIGKILL)
        proc.wait(timeout=max(0.1, CHILD_SECONDS - (time.monotonic() - start)))
    except BaseException:
        try:
            os.killpg(proc.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        proc.wait(timeout=2)
        raise
    finally:
        selector.close()
        for stream in (proc.stdout, proc.stderr):
            stream.close()
        # A reaped leader is not enough if it left a child holding the group.
        until = time.monotonic() + 0.5
        while True:
            try:
                os.killpg(proc.pid, 0)
            except ProcessLookupError:
                break
            if time.monotonic() >= until:
                remaining = True
                os.killpg(proc.pid, signal.SIGKILL)
                break
            time.sleep(0.01)
    return dict(exit=proc.returncode, failure=failure,
                group_survived_normal_exit=remaining,
                seconds=time.monotonic() - start,
                **{key: bytes(value).decode("utf-8", errors="replace")
                   for key, value in chunks.items()})


def inventory(root):
    return {path.relative_to(root).as_posix(): hashlib.sha256(path.read_bytes()).hexdigest()
            for path in sorted(root.rglob("*")) if path.is_file()}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--release", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--baseline", action="store_true")
    parser.add_argument("--only", help="Run one named regression; report retains selection")
    opts = parser.parse_args()
    release = opts.release.resolve()
    binary = release / "bin/pblang"
    driver = release / "stdlib/check_cli.sapp"
    assert binary.is_file() and driver.is_file()
    rows = []
    started = time.monotonic()
    report = {"baseline": opts.baseline, "selected_case": opts.only, "rows": rows,
              "output_cap_bytes": OUTPUT_CAP, "per_child_seconds": CHILD_SECONDS,
              "memory_limit_bytes_not_measured_RSS": 768 * 1024**2,
              "source_hidden": True, "network_isolated": True,
              "release": str(release), "driver_sha256": hashlib.sha256(driver.read_bytes()).hexdigest(),
              "passed": False}

    def save():
        opts.report.write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n")
        opts.report.chmod(0o600)

    try:
        with tempfile.TemporaryDirectory(prefix="check-argv-") as folder:
            scratch = Path(folder)
            good = 'let answer = 42; print answer;\n'
            bad = 'let answer = ;\n'
            sources = {
                "valid.sapp": good, "invalid.sapp": bad,
                "bad.sapp ": bad, "bad.sapp": good,
                "good.sapp ": good, "good.sapp": bad,
                "line\nvalid.sapp": good, "line\nbad.sapp": bad,
                "split\n/scratch/piece.sapp": bad, "split": good, "piece.sapp": good,
                "ελληνικά.sapp": good, "two words.sapp": good,
                " leading.sapp": good, "tab\tname.sapp": good,
                "--": good, "--retained-source": good,
                "arity.sapp": 'fn identity(value:number){return value;} identity();',
                "type.sapp": 'let answer:number=nil-1;',
                "nested.sapp": 'for _i 1 {let p=map{run:fn(self){return 7;}};print (try {p:run()} catch 9);}',
                "lazy.sapp": 'fn load(){return import("PRIVATE_CANARY");} export load;',
                "effect.sapp": 'fs_write("/scratch/MUST_NOT_EXIST", "SOURCE_EXECUTED");',
                "throw.sapp": 'throw "SOURCE_EXECUTED";',
            }
            for name, text in sources.items():
                (scratch / name).parent.mkdir(parents=True, exist_ok=True)
                (scratch / name).write_text(text)
            # Native run overwrites the parent filename environment. Those
            # cases below prove routing only. Static inspection independently
            # binds the driver body to the no-environment filename contract.
            body = driver.read_text().split("fn _arg_files() {", 1)[1].split("fn check_files", 1)[0]
            report["filename_environment_static"] = {
                "contains_os_args": "os_args()" in body,
                "contains_filename_environment": "PBLANG_SCRIPT_ARGS" in body,
                "contains_env_get": "env_get(" in body,
                "runtime_env_override_proven": False,
                "parent_environment_test_scope": "end-to-end routing only"}
            if not opts.baseline:
                assert "os_args()" in body and "env_get(" not in body and "PBLANG_SCRIPT_ARGS" not in body
            before = inventory(scratch)
            report["synthetic_fixture_hashes"] = before
            box = ["bwrap", "--unshare-all", "--die-with-parent", "--clearenv"]
            for name in ["/usr", "/bin", "/lib", "/lib64"]:
                box += ["--ro-bind", name, name]
            box += ["--proc", "/proc", "--dev", "/dev", "--tmpfs", "/tmp",
                    "--ro-bind", str(release), str(release),
                    "--bind", str(scratch), "/scratch", "--chdir", "/scratch",
                    "--setenv", "PATH", "/usr/bin:/bin", "--setenv", "LANG", "C.UTF-8"]
            restrictions = ["--deny-net", "--deny-http", "--deny-exec", "--deny-ffi",
                            "--deny-spawn", "--deny-gfx", "--deny-sqlite"]

            def direct(files, flags=()):
                return ["run", str(driver), *restrictions, "--", *flags, *files]

            def case(name, args, code, count=None, contains=None, env=None, old_code=None):
                if opts.only is not None and name != opts.only:
                    return
                if time.monotonic() - started > 150:
                    raise TimeoutError("finite suite budget exceeded")
                environment = []
                for key, value in (env or {}).items():
                    environment += ["--setenv", key, value]
                result = bounded_process([*box, *environment, str(binary), *args])
                expected = old_code if opts.baseline and old_code is not None else code
                text = result["stdout"] + result["stderr"]
                unchanged = inventory(scratch) == before
                correct_status = result["exit"] == expected
                # On a deliberately wrong-file baseline, the expected old
                # output is preserved, not required to satisfy new assertions.
                old_bug = opts.baseline and old_code is not None
                checks = {"status": correct_status, "no_timeout_or_output_overflow": result["failure"] is None,
                          "process_group_clean": not result["group_survived_normal_exit"],
                          "fixtures_unchanged": unchanged,
                          "source_not_executed": "SOURCE_EXECUTED" not in text,
                          "batch_count": old_bug or count is None or f"checked {count} file(s):" in result["stdout"],
                          "expected_diagnostic": old_bug or contains is None or contains in text}
                row = dict(name=name, args=args, expected_exit=expected,
                           corrected_contract_exit=code, baseline_known_bug=old_bug,
                           checks=checks, passed=all(checks.values()), **result)
                rows.append(row)
                save()
                print(name, row["passed"], result["exit"], flush=True)
                # Continue independent semantic mismatches to preserve a full
                # finite matrix; stop immediately if isolation/cleanup fails.
                for safety in ["no_timeout_or_output_overflow", "process_group_clean",
                               "fixtures_unchanged", "source_not_executed"]:
                    assert checks[safety], row

            for mode in ["public", "direct"]:
                command = (lambda paths: ["check", *paths]) if mode == "public" else direct
                case(mode + " wrong-file rejection", command(["/scratch/bad.sapp "]), 1, 1, old_code=0)
                case(mode + " reverse wrong-file acceptance", command(["/scratch/good.sapp "]), 0, 1, old_code=1)
                case(mode + " newline single file", command(["/scratch/line\nvalid.sapp"]), 0, 1, old_code=1)
                case(mode + " split-decoy rejection", command(["/scratch/split\n/scratch/piece.sapp"]), 1, 1, old_code=0)
                case(mode + " valid control", command(["/scratch/valid.sapp"]), 0, 1)
            if not opts.baseline:
                for mode in ["public", "direct"]:
                    command = (lambda paths: ["check", *paths]) if mode == "public" else direct
                    for name in ["ελληνικά.sapp", "two words.sapp", " leading.sapp", "tab\tname.sapp", "--", "--retained-source"]:
                        case(mode + " exact " + repr(name), command(["/scratch/" + name]), 0, 1)
                    for name in ["invalid.sapp", "arity.sapp", "type.sapp", "line\nbad.sapp", "missing.sapp"]:
                        case(mode + " invalid " + repr(name), command(["/scratch/" + name]), 1, 1)
                    case(mode + " mixed batch", command(["/scratch/valid.sapp", "/scratch/line\nbad.sapp"]), 1, 2)
                    case(mode + " two valid", command(["/scratch/valid.sapp", "/scratch/line\nvalid.sapp"]), 0, 2)
                    case(mode + " empty argument", command([""]), 1, 1)
                    case(mode + " parent env decoy (routing only)", command(["/scratch/invalid.sapp"]), 1, 1,
                         env={"PBLANG_SCRIPT_ARGS": "/scratch/valid.sapp"})
                case("direct second separator is filename", direct(["--"]), 0, 1)
                case("direct no delimiter", ["run", str(driver), *restrictions], 1, contains="source files required")
                case("direct empty tail", direct([]), 1, contains="source files required")
                case("bare driver existing route", [str(driver), *restrictions, "--", "/scratch/valid.sapp"], 0, 1)
                for name in ["effect.sapp", "throw.sapp"]:
                    case("not executed " + name, ["check", "/scratch/" + name], 0, 1)
                for profile in ["", "default"]:
                    case("default profile " + repr(profile), ["check", "/scratch/nested.sapp"], 0, 1,
                         env={"PBLANG_CHECK_PROFILE": profile})
                for profile, filename, code in [("retained-source", "nested.sapp", "retained-nested-try"),
                                                 ("import-origin", "lazy.sapp", "import-origin-deferred")]:
                    case("environment " + profile, ["check", "/scratch/" + filename], 1, 1, code,
                         {"PBLANG_CHECK_PROFILE": profile})
                    case("explicit " + profile, direct(["/scratch/" + filename], ["--" + profile]), 1, 1, code)
                    case("duplicate " + profile, direct(["/scratch/valid.sapp"], ["--" + profile] * 2), 1,
                         contains="duplicate")
                case("unknown profile", ["check", "/scratch/valid.sapp"], 1, contains="unknown source profile",
                     env={"PBLANG_CHECK_PROFILE": "unknown"})
                for name, code in [("valid.sapp", 0), ("invalid.sapp", 1), ("line\nvalid.sapp", 0)]:
                    case("public selfhost unchanged " + repr(name), ["--check", "--via-pblang", "/scratch/" + name], code)
            report["passed"] = bool(rows) and all(row["passed"] for row in rows)
            report["case_count"] = len(rows)
            report["fixture_directory_removed"] = False
        report["fixture_directory_removed"] = not scratch.exists()
    finally:
        report["seconds"] = time.monotonic() - started
        save()
    print("PASS" if report["passed"] else "FAIL", len(rows))
    if not report["passed"]:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
