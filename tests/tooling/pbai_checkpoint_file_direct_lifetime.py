"""Linux source-hidden check of the *direct* checkpoint-file example read/reload.

Unlike pbai_guarded_file_lifetime.py (explicit PBCK profile), this runs the
ordinary source example. A test-only observer is registered as the first
script-level defer, so it runs last: after the example's own cleanup, before
diagnostic release. --phase baseline only characterizes a historical caller
and requires that at least one fault leaves an acquired handle unclosed.
"""
import argparse
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


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--product", choices=["pblang", "pbai"], required=True)
    parser.add_argument("--phase", choices=["baseline", "fixed"], default="fixed")
    parser.add_argument("--caller", type=Path,
                        help="historical caller source (baseline; default: frozen pre-defer fixture)")
    args = parser.parse_args()
    if args.phase == "fixed" and args.caller is not None:
        parser.error("--caller is only valid with --phase baseline")
    root = Path(__file__).resolve().parents[2]
    provider = root if args.product == "pblang" else root.parent / "pblang"
    runtime = str(Path(os.environ["PBLANG_BIN"]).resolve(strict=True))
    directory = "examples" if args.product == "pblang" else "demo"
    example = (root / directory / "pbai_student_checkpoint_file.sapp").read_text()
    if args.phase == "baseline" and args.caller is None:
        args.caller = root / "tests/fixtures/pbai_student_checkpoint_file_baseline.sapp"
    caller = args.caller.read_text() if args.caller else example
    temp = Path(tempfile.mkdtemp(prefix="pbai-direct-file-"))
    os.chmod(temp, 0o700)
    results = []
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
        unclosed = [r for r in results if r["mode"] not in SUCCESS
                    and any(row["close_calls"] == 0 for row in r["observation"]["rows"])]
        if args.phase == "baseline":
            assert unclosed, "baseline did not reproduce an unclosed acquired handle"
        else:
            assert not unclosed, unclosed
    finally:
        shutil.rmtree(temp)
    print(json.dumps({"product": args.product, "phase": args.phase, "cases": len(results),
                      "unclosed_fault_cases": len(unclosed), "results": results,
                      "source_hidden": True, "network_denied": True,
                      "private_fixture_removed": not temp.exists(),
                      "physical_memory_release_verified": False}, sort_keys=True))


if __name__ == "__main__":
    main()
