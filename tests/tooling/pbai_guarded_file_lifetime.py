"""Linux source-hidden qualification of the existing PBCK guarded file profile."""
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
        raise RuntimeError("guarded caller qualification timed out") from None
    if expected is not None and child.returncode != expected:
        raise RuntimeError(stderr[-2000:])
    return {"exit": child.returncode, "stdout": stdout, "stderr": stderr}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--product", choices=["pblang", "pbai"], required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    provider = root if args.product == "pblang" else root.parent / "pblang"
    runtime = str(Path(os.environ["PBLANG_BIN"]).resolve(strict=True))
    directory = "examples" if args.product == "pblang" else "demo"
    original = (root / directory / "pbai_student_checkpoint_file.sapp").read_text()
    guarded = (root / directory / "pbai_student_checkpoint_file_guarded.sapp").read_text()
    temp = Path(tempfile.mkdtemp(prefix="pbai-guarded-file-"))
    os.chmod(temp, 0o700)
    results = []
    try:
        embedded = temp / "pblang"
        shutil.copytree(provider / "stdlib", embedded / "stdlib")
        (embedded / "pblang.toml").write_text('[package]\nname="guard-provider"\nversion="0.1.0"\nentry="main.sapp"\n[dependencies]\nstdlib={path="stdlib"}\n')
        (embedded / "main.sapp").write_text('print "guard provider";\n')
        project = temp / args.product
        if args.product == "pbai":
            shutil.copytree(root / "stdlib", project / "stdlib")
            (project / "pblang.toml").write_text('[package]\nname="guard-caller"\nversion="0.1.0"\nentry="main.sapp"\n[dependencies]\nstdlib={path="../pblang/stdlib"}\n')
            (project / "main.sapp").write_text('print "guard caller";\n')
        owner = embedded / "stdlib/pbai_neural_adapter.sapp"
        source = owner.read_text()
        hooks = (root / "tests/fixtures/pbai_caller_lifetime_hooks.sapp").read_text()
        signature = "fn student_load(artifact) {"
        assert source.count(signature) == 1
        source = source.replace(signature, "fn _guard_original_load(artifact) {", 1)
        hooks += '\nfn student_load(artifact){_caller_probe_acquire("loaded");return _caller_probe_wrap(_guard_original_load(artifact),"loaded");}\n'
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
        def command(path, compile_only=False):
            # Runtime imports use only the private copied product/provider.
            cmd = ["/usr/bin/bwrap", "--die-with-parent", "--unshare-net", "--ro-bind", "/", "/",
                   "--dev", "/dev", "--bind", str(temp), str(temp)]
            for hidden in sorted({str(root), str(provider)}):
                cmd += ["--tmpfs", hidden]
            flags = ["--allow-all", "--deny-net", "--deny-http", "--deny-spawn", "--deny-gfx", "--deny-sqlite"]
            if compile_only:
                flags += ["--deny-exec", "--deny-ffi"]
            return cmd + ["--chdir", str(project), runtime, "run", str(path), *flags]
        (project / "seed.sapp").write_text(original)
        env["PBAI_CHECKPOINT_TEST_PHASE"] = "write"
        run(command(project / "seed.sapp"), project, env, 0)
        saved = (files / "student.json").read_bytes()
        setup = '''let TEST_N=import("stdlib:pbai_neural_adapter.sapp");
let TEST_IO=import("stdlib:pbai_student_checkpoint_io.sapp");
let independent=TEST_IO.load_file(path_join(env_get("PBAI_CHECKPOINT_TEST_DIR"),"student.json"));
let independent_before=independent.checkpoint().digest;
TEST_N.caller_probe_configure(env_get("PBAI_GUARD_FAULT"),"checkpoint_file_guarded");
fn test_observe(){
 let observed=TEST_N.caller_probe_observe();
 let unchanged=independent.checkpoint().digest==independent_before;
 independent.close();
 print json_stringify(map{format:"pbai.guarded-file.observer.v1",rows:observed.rows,independent_unchanged:unchanged});
}
'''
        modes = {
            "read": ["success", "loaded_use_failure", "loaded_acquire_failure", "close_before", "close_after", "close_invalid", "loaded_use_close_before", "success"],
            "reload": ["success", "candidate_missing", "candidate_corrupt", "loaded_acquire_failure", "export_failure", "loaded_use_failure", "replacement_acquire_failure", "close_before", "close_after", "close_invalid", "loaded_use_close_before", "success"],
        }
        for version in ["baseline", "fixed", "unmodified"]:
            if version == "baseline":
                text = setup + "\nfn checkpoint_work(){\n" + original + "\n}\nfn checkpoint_release(){test_observe();}\n"
            elif version == "fixed":
                marker = ' if failed {throw "checkpoint guarded cleanup failed";}'
                assert guarded.count(marker) == 1
                text = setup + guarded.replace(marker, " test_observe();\n" + marker, 1)
            else:
                text = guarded
            (project / "workflow.sapp").write_text(text)
            (project / "compile.sapp").write_text('let C=import("stdlib:compiler/native_pbck.sapp");fs_write_bytes("workflow.pbck",C.compile_source_with_cleanup(fs_read("workflow.sapp"),"checkpoint_work","checkpoint_release"));\n')
            run(command(project / "compile.sapp", True), project, env, 0)
            for phase, fault_modes in modes.items():
                if version == "unmodified":
                    fault_modes = ["success", "candidate_missing", "candidate_corrupt"] if phase == "reload" else ["success"]
                for mode in fault_modes:
                    candidate = files / "candidate.json"
                    if candidate.exists():
                        candidate.unlink()
                    if mode != "candidate_missing":
                        candidate.write_bytes(b"synthetic-invalid-checkpoint" if mode == "candidate_corrupt" else saved)
                    env.update(PBAI_CHECKPOINT_TEST_PHASE=phase, PBAI_GUARD_FAULT=mode)
                    result = run(command(project / "workflow.pbck"), project, env)
                    rows = [json.loads(line) for line in result["stdout"].splitlines() if line.strip()]
                    observations = [v for v in rows if v.get("format") == "pbai.guarded-file.observer.v1"]
                    output = [v for v in rows if v.get("format") == "pbai.checkpoint-file.example.v1"]
                    if version != "unmodified":
                        assert len(observations) == 1, result
                        observed = observations[0]
                        assert observed["independent_unchanged"], observed
                        expected_handles = 0 if mode == "loaded_acquire_failure" else 1
                        if phase == "reload" and mode not in {"loaded_acquire_failure", "candidate_missing", "candidate_corrupt", "replacement_acquire_failure", "export_failure"}:
                            expected_handles = 2
                        assert len(observed["rows"]) == expected_handles, (phase, mode, observed)
                        if output and phase == "reload":
                            expected_loaded = mode not in {"candidate_missing", "candidate_corrupt", "replacement_acquire_failure"}
                            assert output[0]["loaded"] == expected_loaded
                            assert output[0]["previous_retained"] == (not expected_loaded)
                        if version == "fixed":
                            for row in observed["rows"]:
                                assert row["close_calls"] == 1, (phase, mode, row)
                                unconfirmed = mode in {"close_before", "loaded_use_close_before"}
                                assert row["predict_closed"] == (not unconfirmed), (phase, mode, row)
                                assert row["checkpoint_closed"] == row["predict_closed"]
                                assert row["first_closes"] == (0 if unconfirmed else 1)
                            success = mode in {"success", "candidate_missing", "candidate_corrupt", "replacement_acquire_failure"}
                            assert (result["exit"] == 0) == success, (phase, mode, result)
                            assert bool(output) == success, (phase, mode, result)
                    else:
                        assert result["exit"] == 0 and len(output) == 1, result
                    assert (files / "student.json").read_bytes() == saved
                    results.append({"version": version, "phase": phase, "mode": mode, "exit": result["exit"], "output": output,
                                    "observation": observations[0] if observations else None,
                                    "primary": result["stderr"].split("]: ", 1)[-1].splitlines()[0] if result["stderr"] else None})
        # Genuine body failures retain the native message even if cleanup fails.
        for phase in ["read", "reload"]:
            old = next(x for x in results if x["version"] == "baseline" and x["phase"] == phase and x["mode"] == "loaded_use_failure")
            new = next(x for x in results if x["version"] == "fixed" and x["phase"] == phase and x["mode"] == "loaded_use_failure")
            assert old["primary"] == new["primary"] == "invalid student input", (old, new)
        for phase, mode in [("reload", "export_failure"), ("reload", "close_before"),
                            ("reload", "close_after"), ("read", "loaded_acquire_failure"),
                            ("reload", "loaded_acquire_failure")]:
            old = next(x for x in results if x["version"] == "baseline" and x["phase"] == phase and x["mode"] == mode)
            new = next(x for x in results if x["version"] == "fixed" and x["phase"] == phase and x["mode"] == mode)
            assert old["primary"] == new["primary"], (old, new)
        double = next(x for x in results if x["version"] == "fixed" and x["phase"] == "read" and x["mode"] == "loaded_use_close_before")
        assert double["primary"] == "invalid student input"
        for current in [x for x in results if x["version"] == "unmodified"]:
            before = next(x for x in results if x["version"] == "baseline" and x["phase"] == current["phase"] and x["mode"] == current["mode"])
            assert current["output"] == before["output"], (current, before)
    finally:
        shutil.rmtree(temp)
    print(json.dumps({"product": args.product, "results": results, "source_hidden": True,
                      "network_denied": True, "private_fixture_removed": not temp.exists(),
                      "physical_memory_release_verified": False}, sort_keys=True))


if __name__ == "__main__":
    main()
