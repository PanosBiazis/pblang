"""Bounded real batch prediction cleanup failure qualification on Linux."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import tempfile


def run(argv, cwd, env):
    p = subprocess.Popen(argv, cwd=cwd, env=env, text=True, stdout=subprocess.PIPE,
                         stderr=subprocess.PIPE, start_new_session=True)
    try:
        stdout, stderr = p.communicate(timeout=45)
    except (subprocess.TimeoutExpired, KeyboardInterrupt):
        os.killpg(p.pid, signal.SIGINT)
        try:
            p.communicate(timeout=2)
        except subprocess.TimeoutExpired:
            os.killpg(p.pid, signal.SIGKILL)
            p.communicate()
        raise RuntimeError("tool cleanup qualification timed out") from None
    return p.returncode, stdout, stderr


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--product", choices=["pblang", "pbai"], required=True)
    parser.add_argument("--phase", choices=["baseline", "fixed"], default="fixed")
    parser.add_argument("--baseline-owner", type=Path)
    parser.add_argument("--provider-stdlib", type=Path)
    parser.add_argument("--expected-owner-sha256")
    args = parser.parse_args()
    if os.name != "posix" or os.uname().sysname != "Linux":
        raise SystemExit("This source-hidden gate requires Linux and bubblewrap")
    if (args.phase == "baseline") != (args.baseline_owner is not None):
        parser.error("baseline requires an explicitly pinned original owner")
    root = Path(__file__).resolve().parents[2]
    embedded = root if args.product == "pblang" else root.parent / "pblang"
    provider_source = args.provider_stdlib or embedded / "stdlib"
    runtime = str(Path(os.environ["PBLANG_BIN"]).absolute())
    temporary = Path(tempfile.mkdtemp(prefix="pbai-tool-cleanup-"))
    temporary.chmod(0o700)
    try:
        provider = temporary / "pblang"
        provider.mkdir()
        shutil.copytree(provider_source, provider / "stdlib")
        project = temporary / args.product
        if project != provider:
            project.mkdir()
            shutil.copytree(root / "stdlib", project / "stdlib")
        for folder in {project, provider}:
            dep = "stdlib" if folder == provider else "../pblang/stdlib"
            (folder / "pblang.toml").write_text('[package]\nname="tool-cleanup-fixture"\nversion="0.1.0"\nentry="main.sapp"\n[dependencies]\nstdlib={path=' + json.dumps(dep) + '}\n')
            (folder / "main.sapp").write_text('print "fixture";\n')
        fixtures = root / "tests/fixtures"
        (project / "main.sapp").write_bytes((fixtures / "pbai_student_tool_cleanup.sapp").read_bytes())
        owner = provider / "stdlib/pbai_neural_adapter.sapp"
        if args.baseline_owner:
            owner.write_bytes(args.baseline_owner.read_bytes())
        original = owner.read_text()
        digest = hashlib.sha256(owner.read_bytes()).hexdigest()
        if args.expected_owner_sha256:
            assert digest == args.expected_owner_sha256, "selected owner pin mismatch"
        signature = "fn student_load(artifact) {"
        assert original.count(signature) == 1 and original.count("fn _number(") == 1
        hooks = (fixtures / "pbai_student_tool_cleanup_hooks.sapp").read_text()
        hooks += "\nfn student_load(artifact){return _cleanup_wrap(_cleanup_original_load(artifact),artifact);}\n"
        owner.write_text(original.replace(signature, "fn _cleanup_original_load(artifact) {", 1).replace("fn _number(", hooks + "\nfn _number(", 1))
        (temporary / "state").mkdir(mode=0o700)
        env = os.environ.copy()
        env.update(PBAI_STATE_ROOT=str(temporary / "state"), PBAI_TOOL_CLEANUP_PHASE=args.phase)
        build_notes = []
        initial_note = "pblang build: note: pblang-side manifest validator unavailable: run pblang manifest validator snippet failed: import 'stdlib:manifest_validate.sapp': declared dependency 'stdlib' has no pblang.lock entry; run `pblang install`\n"
        for folder in ([provider, project] if provider != project else [project]):
            code, out, err = run([runtime, "build"], folder, env)
            assert code == 0 and out == "build ok\n" and err in ("", initial_note), (code, out[-800:], err[-800:])
            if err:
                build_notes.append(err.strip())
        command = ["/usr/bin/bwrap", "--die-with-parent", "--unshare-net", "--ro-bind", "/", "/",
                   "--dev", "/dev", "--bind", str(temporary), str(temporary)]
        hidden = {str(root), str(embedded), os.environ.get("PBAI_SOURCE_HIDE_ROOT", "")} - {""}
        for path in sorted(hidden):
            command += ["--tmpfs", path]
        command += ["--chdir", str(project), runtime, "run", str(project / "main.sapp")]
        command += ["--deny-" + name for name in ("net", "http", "exec", "ffi", "spawn", "gfx", "sqlite")]
        code, out, err = run(command, project, env)
        assert code == 0 and not err, (code, out[-1000:], err[-2000:])
        observation = json.loads(out)
        assert observation["format"] == "pbai.student-tool.cleanup.v1" and len(observation["rows"]) == 39
        assert str(temporary) not in out and "synthetic private" not in out
        result = dict(product=args.product, phase=args.phase, owner_sha256=digest,
                      provider_source="explicit provider copy" if args.provider_stdlib else "repository provider copy",
                      observation=observation, source_hidden=True, network_denied=True,
                      ffi_exec_denied=True, test_only_overlay=True, build_notes=build_notes)
    finally:
        shutil.rmtree(temporary)
    assert not temporary.exists()
    result["private_fixture_removed"] = True
    print(json.dumps(result, sort_keys=True))


if __name__ == "__main__":
    main()
