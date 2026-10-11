"""Linux source-hidden check of the *direct* checkpoint-file example read/reload.

Unlike pbai_guarded_file_lifetime.py (explicit PBCK profile), this runs the
ordinary source example. A test-only observer is registered as the first
script-level defer, so it runs last: after the example's own cleanup, before
diagnostic release. --phase baseline only characterizes a historical caller
and requires that at least one fault omits a cleanup attempt.
Fixed acceptance also requires the behavior-tested native runtime contract.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import tempfile


def run(command, cwd, env, expected=None):
    child = subprocess.Popen(command, cwd=cwd, env=env, stdout=subprocess.PIPE,
                             stderr=subprocess.PIPE, text=True, start_new_session=True)
    try:
        stdout, stderr = child.communicate(timeout=40)
    except (subprocess.TimeoutExpired, KeyboardInterrupt):
        try:
            os.killpg(child.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        child.communicate()
        raise RuntimeError("direct caller check timed out") from None
    if expected is not None and child.returncode != expected:
        raise RuntimeError(stderr[-2000:])
    return {"exit": child.returncode, "stdout": stdout, "stderr": stderr}


MODES = {
    "read": ["success", "loaded_use_failure", "loaded_acquire_failure", "close_before",
             "close_after", "close_invalid", "loaded_use_close_before", "success"],
    "reload": ["success", "candidate_missing", "candidate_corrupt", "loaded_acquire_failure",
               "export_failure", "loaded_use_failure", "replacement_acquire_failure",
               "close_before", "close_after", "close_invalid", "loaded_use_close_before", "success"],
}
SUCCESS = {"success", "candidate_missing", "candidate_corrupt", "replacement_acquire_failure"}
PRIMARY = {
    "loaded_use_failure": "invalid student input",
    "loaded_use_close_before": "invalid student input",
    "loaded_acquire_failure": "synthetic restore acquisition failure",
    "export_failure": "synthetic export failure",
    "close_before": "synthetic close before release",
    "close_after": "synthetic close after release",
    "close_invalid": "invalid checkpoint close result",
}


RUNTIME_PROBES = {
    "pending_defer_gc": ('let events=[];\nfn work(){let a=[7];let m=map{x:9};'
                         'defer push(events,a[0]);defer push(events,m.x);'
                         'defer gc_collect();throw "probe primary";}\n'
                         'print (try work() catch "caught");print events;\n',
                         0, "caught\n[9, 7]", None),
    "primary_error": ('fn cleanup(){throw "probe cleanup";}\n'
                      'fn work(){defer cleanup();throw "probe primary";}work();\n',
                      1, "", "probe primary"),
}


def sha256(path):
    digest = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def tree_fingerprint(directory):
    # Relative names and individual content digests; never persist host paths.
    entries = [{"path": p.relative_to(directory).as_posix(), "sha256": sha256(p)}
               for p in sorted(directory.rglob("*.sapp")) if p.is_file()]
    encoded = json.dumps(entries, sort_keys=True, separators=(",", ":")).encode()
    return {"sha256": hashlib.sha256(encoded).hexdigest(), "files": len(entries)}


def summarize(results):
    missed = [r for r in results if r["mode"] not in SUCCESS
              and any(row["close_calls"] == 0 for row in r["observation"]["rows"])]
    unconfirmed = [r for r in results
                   if any(row["first_closes"] == 0 for row in r["observation"]["rows"])]
    return {
        "invocations": len(results),
        "distinct_phase_modes": len({(r["phase"], r["mode"]) for r in results}),
        "fault_invocations": sum(r["mode"] not in SUCCESS for r in results),
        "missing_cleanup_attempt_fault_cases": len(missed),
        # Compatibility alias only: NOT a count of live handles or RAM leaks.
        "unclosed_fault_cases": len(missed),
        "unconfirmed_release_cases": len(unconfirmed),
        "unconfirmed_release_handles": sum(row["first_closes"] == 0
                                           for r in results for row in r["observation"]["rows"]),
        "legacy_unclosed_fault_cases_semantics": "missing_cleanup_attempt_fault_cases",
    }


def emit(report, output):
    encoded = json.dumps(report, sort_keys=True) + "\n"
    if output is not None:
        # Exclusive creation avoids silently replacing earlier failures/evidence.
        descriptor = os.open(output, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        with os.fdopen(descriptor, "w") as stream:
            stream.write(encoded)
    print(encoded, end="")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--product", choices=["pblang", "pbai"], required=True)
    parser.add_argument("--phase", choices=["baseline", "fixed"], default="fixed")
    parser.add_argument("--caller", type=Path,
                        help="historical caller source (baseline; default: frozen pre-defer fixture)")
    parser.add_argument("--output", type=Path, help="new durable JSON file (exclusive creation, mode 0600)")
    parser.add_argument("--runtime-check-only", action="store_true", help="verify native defer/GC contract without running caller scenarios")
    args = parser.parse_args()
    if args.phase == "fixed" and args.caller is not None:
        parser.error("--caller is only valid with --phase baseline")
    root = Path(__file__).resolve().parents[2]
    provider = root if args.product == "pblang" else root.parent / "pblang"
    runtime = str(Path(os.environ["PBLANG_BIN"]).resolve(strict=True))
    native = Path(runtime)
    with native.open("rb") as stream:
        header = stream.read(4096)
    native_identity = "direct executable"
    if not header.startswith(b"\x7fELF"):
        # Installed bridge has a fixed sibling dispatch target. Custom launchers
        # are not accepted because their effective native target is unknown.
        if b'native=$bindir/pblang-native' not in header or b'exec "$native" "$@"' not in header:
            parser.error("use a native executable or the installed PBLang bridge")
        native = native.parent / "pblang-native"
        native_identity = "installed bridge sibling target"
    directory = "examples" if args.product == "pblang" else "demo"
    example = (root / directory / "pbai_student_checkpoint_file.sapp").read_text()
    if args.phase == "baseline" and args.caller is None:
        args.caller = root / "tests/fixtures/pbai_student_checkpoint_file_baseline.sapp"
    caller = args.caller.read_text() if args.caller else example
    provenance = {
        "runtime_sha256": sha256(runtime), "native_sha256": sha256(native),
        "native_identity": native_identity, "harness_sha256": sha256(__file__),
        "caller_sha256": hashlib.sha256(caller.encode()).hexdigest(),
        "example_sha256": hashlib.sha256(example.encode()).hexdigest(),
        "provider_stdlib": tree_fingerprint(provider / "stdlib"),
        "product_stdlib": tree_fingerprint(root / "stdlib"),
        "hooks_sha256": sha256(root / "tests/fixtures/pbai_caller_lifetime_hooks.sapp"),
        "runtime_probe_sha256": hashlib.sha256(json.dumps(RUNTIME_PROBES, sort_keys=True).encode()).hexdigest(),
    }
    temp = Path(tempfile.mkdtemp(prefix="pbai-direct-file-"))
    os.chmod(temp, 0o700)
    results = []
    runtime_contract = []
    accepted = False
    failure = None
    try:
        embedded = temp / "pblang"
        shutil.copytree(provider / "stdlib", embedded / "stdlib")
        (embedded / "pblang.toml").write_text('[package]\nname="direct-provider"\nversion="0.1.0"\nentry="main.sapp"\n[dependencies]\nstdlib={path="stdlib"}\n')
        (embedded / "main.sapp").write_text('print "direct provider";\n')
        project = temp / args.product
        if args.product == "pbai":
            shutil.copytree(root / "stdlib", project / "stdlib")
            (project / "pblang.toml").write_text('[package]\nname="direct-caller"\nversion="0.1.0"\nentry="main.sapp"\n[dependencies]\nstdlib={path="../pblang/stdlib"}\n')
            (project / "main.sapp").write_text('print "direct caller";\n')
        owner = embedded / "stdlib/pbai_neural_adapter.sapp"
        source = owner.read_text()
        hooks = (root / "tests/fixtures/pbai_caller_lifetime_hooks.sapp").read_text()
        signature = "fn student_load(artifact) {"
        assert source.count(signature) == 1
        source = source.replace(signature, "fn _direct_original_load(artifact) {", 1)
        hooks += '\nfn student_load(artifact){_caller_probe_acquire("loaded");return _caller_probe_wrap(_direct_original_load(artifact),"loaded");}\n'
        owner.write_text(source.replace("fn _number(", hooks + "\nfn _number(", 1))
        state = temp / "state"
        state.mkdir(mode=0o700)
        files = temp / "checkpoints"
        files.mkdir(mode=0o700)
        env = os.environ.copy()
        env.update(PBAI_STATE_ROOT=str(state), PBAI_CHECKPOINT_TEST_DIR=str(files))
        run([runtime, "build"], embedded, env, 0)
        if project != embedded:
            run([runtime, "build"], project, env, 0)

        def command(path):
            cmd = ["/usr/bin/bwrap", "--die-with-parent", "--unshare-net", "--ro-bind", "/", "/",
                   "--dev", "/dev", "--bind", str(temp), str(temp)]
            for hidden in sorted({str(root), str(provider)}):
                cmd += ["--tmpfs", hidden]
            flags = ["--allow-all", "--deny-net", "--deny-http", "--deny-spawn", "--deny-gfx", "--deny-sqlite"]
            return cmd + ["--chdir", str(project), runtime, "run", str(path), *flags]

        for name, (probe, expected_exit, expected_stdout, expected_error) in RUNTIME_PROBES.items():
            probe_file = project / ("runtime-" + name + ".sapp")
            probe_file.write_text(probe)
            outcome = run(command(probe_file), project, env)
            primary = outcome["stderr"].split("]: ", 1)[-1].splitlines()[0] if outcome["stderr"] else None
            passed = (outcome["exit"] == expected_exit and outcome["stdout"].strip() == expected_stdout
                      and primary == expected_error)
            runtime_contract.append({"probe": name, "passed": passed, "exit": outcome["exit"],
                                     "stdout": outcome["stdout"], "primary": primary})
        accepted = all(probe["passed"] for probe in runtime_contract)
        if not args.runtime_check_only and (accepted or args.phase == "baseline"):
            (project / "seed.sapp").write_text(example)
            env["PBAI_CHECKPOINT_TEST_PHASE"] = "write"
            run(command(project / "seed.sapp"), project, env, 0)
            saved = (files / "student.json").read_bytes()
            # Registered first, so it runs last: after the caller's own defers.
            setup = '''let TEST_N=import("stdlib:pbai_neural_adapter.sapp");
    let TEST_IO=import("stdlib:pbai_student_checkpoint_io.sapp");
    let independent=TEST_IO.load_file(path_join(env_get("PBAI_CHECKPOINT_TEST_DIR"),"student.json"));
    let independent_before=independent.checkpoint().digest;
    TEST_N.caller_probe_configure(env_get("PBAI_DIRECT_FAULT"),"checkpoint_file_guarded");
    fn test_observe(){
     let observed=TEST_N.caller_probe_observe();
     let unchanged=independent.checkpoint().digest==independent_before;
     independent.close();
     print json_stringify(map{format:"pbai.direct-file.observer.v1",rows:observed.rows,independent_unchanged:unchanged});
    }
    defer test_observe();
    '''
            (project / "workflow.sapp").write_text(setup + caller)
            for phase, fault_modes in MODES.items():
                for mode in fault_modes:
                    candidate = files / "candidate.json"
                    if candidate.exists():
                        candidate.unlink()
                    if mode != "candidate_missing":
                        candidate.write_bytes(b"synthetic-invalid-checkpoint" if mode == "candidate_corrupt" else saved)
                    env.update(PBAI_CHECKPOINT_TEST_PHASE=phase, PBAI_DIRECT_FAULT=mode)
                    result = run(command(project / "workflow.sapp"), project, env)
                    rows = [json.loads(line) for line in result["stdout"].splitlines() if line.strip().startswith("{")]
                    observations = [v for v in rows if v.get("format") == "pbai.direct-file.observer.v1"]
                    output = [v for v in rows if v.get("format") == "pbai.checkpoint-file.example.v1"]
                    assert len(observations) == 1, (phase, mode, result)
                    observed = observations[0]
                    assert observed["independent_unchanged"], observed
                    expected_handles = 0 if mode == "loaded_acquire_failure" else 1
                    if phase == "reload" and mode not in {"loaded_acquire_failure", "candidate_missing", "candidate_corrupt", "replacement_acquire_failure", "export_failure"}:
                        expected_handles = 2
                    assert len(observed["rows"]) == expected_handles, (phase, mode, observed)
                    primary = result["stderr"].split("]: ", 1)[-1].splitlines()[0] if result["stderr"] else None
                    if args.phase == "fixed":
                        success = mode in SUCCESS
                        assert (result["exit"] == 0) == success, (phase, mode, result)
                        assert bool(output) == success, (phase, mode, result)
                        if not success:
                            # Known primary errors survive cleanup. In reload the retired
                            # model's close precedes prediction, so it fails first.
                            expected_primary = PRIMARY[mode]
                            if phase == "reload" and mode == "loaded_use_close_before":
                                expected_primary = PRIMARY["close_before"]
                            assert primary == expected_primary, (phase, mode, primary)
                        unconfirmed = mode in {"close_before", "loaded_use_close_before"}
                        for row in observed["rows"]:
                            # Exactly one cleanup attempt per owned handle on failure paths;
                            # success paths keep the caller's explicit idempotence check.
                            if not success:
                                assert row["close_calls"] == 1, (phase, mode, row)
                            assert row["first_closes"] == (0 if unconfirmed else 1), (phase, mode, row)
                            assert row["predict_closed"] == (not unconfirmed), (phase, mode, row)
                            assert row["checkpoint_closed"] == row["predict_closed"]
                        if output and phase == "reload":
                            expected_loaded = mode not in {"candidate_missing", "candidate_corrupt", "replacement_acquire_failure"}
                            assert output[0]["loaded"] == expected_loaded
                            assert output[0]["previous_retained"] == (not expected_loaded)
                    assert (files / "student.json").read_bytes() == saved
                    results.append({"phase": phase, "mode": mode, "exit": result["exit"], "output": output,
                                    "observation": observed, "primary": primary})
    except Exception as error:
        # Do not persist exception text containing temporary/private host paths.
        failure = type(error).__name__
    finally:
        shutil.rmtree(temp)
    metrics = summarize(results)
    if results and failure is None:
        if args.phase == "baseline" and metrics["missing_cleanup_attempt_fault_cases"] == 0:
            failure = "HistoricalBaselineNotReproduced"
        elif args.phase == "fixed" and metrics["missing_cleanup_attempt_fault_cases"] != 0:
            failure = "MissingCleanupAttempt"
    status = ("failed" if failure else "runtime-rejected" if not accepted else
              "runtime-qualified" if args.runtime_check_only else
              "historical-baseline" if args.phase == "baseline" else "passed")
    report = {"format": "pbai.direct-file.acceptance.v2", "product": args.product, "phase": args.phase,
              "status": status, "failure_type": failure, "caller_scenarios_executed": bool(results),
              "cases": len(results), **metrics, "results": results, "provenance": provenance,
              "runtime_contract": runtime_contract, "runtime_accepted": accepted,
              "backend": "retained-native-source", "pbck_direct_supported": False,
              "source_hidden": True, "network_denied": True,
              "private_fixture_removed": not temp.exists(),
              "physical_memory_release_verified": False}
    emit(report, args.output)
    if failure:
        raise SystemExit("Direct caller check failed; inspect the durable report failure_type and partial results.")
    if not accepted and (args.phase == "fixed" or args.runtime_check_only):
        raise SystemExit("Runtime contract rejected: native defer/GC probes failed; caller scenarios were not accepted.")



if __name__ == "__main__":
    main()
