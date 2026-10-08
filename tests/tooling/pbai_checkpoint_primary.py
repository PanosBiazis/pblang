"""Bounded Linux real-file known primary-error and cleanup regression gate."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import tempfile


BUILD_NOTES = []

def run(argv, cwd, env):
    p = subprocess.Popen(argv, cwd=cwd, env=env, stdout=subprocess.PIPE,
                         stderr=subprocess.PIPE, text=True, start_new_session=True)
    try:
        stdout, stderr = p.communicate(timeout=30)
    except (subprocess.TimeoutExpired, KeyboardInterrupt):
        os.killpg(p.pid, signal.SIGINT)
        try:
            p.communicate(timeout=2)
        except subprocess.TimeoutExpired:
            os.killpg(p.pid, signal.SIGKILL)
            p.communicate()
        raise RuntimeError("bounded acquisition gate timed out") from None
    known_initial_note = "pblang build: note: pblang-side manifest validator unavailable: run pblang manifest validator snippet failed: import 'stdlib:manifest_validate.sapp': declared dependency 'stdlib' has no pblang.lock entry; run `pblang install`\n"
    if argv[1:] == ["build"] and p.returncode == 0 and stdout == "build ok\n" and stderr == known_initial_note:
        BUILD_NOTES.append(stderr.strip())
        stderr = ""
    if p.returncode or stderr:
        raise RuntimeError("acquisition gate failed: " + stdout[-800:] + stderr[-2400:])
    return stdout


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--product", choices=["pblang", "pbai"], required=True)
    parser.add_argument("--phase", choices=["baseline", "fixed"], default="fixed")
    parser.add_argument("--baseline-owner", type=Path)
    parser.add_argument("--provider-stdlib", type=Path)
    parser.add_argument("--expected-owner-sha256")
    args = parser.parse_args()
    if (args.phase == "baseline") != (args.baseline_owner is not None):
        parser.error("baseline requires explicit original scope source")
    if os.name != "posix" or os.uname().sysname != "Linux":
        raise SystemExit("This real-file source-hidden test requires Linux and bubblewrap")
    root = Path(__file__).resolve().parents[2]
    embedded = root if args.product == "pblang" else root.parent / "pblang"
    runtime = str(Path(os.environ["PBLANG_BIN"]).absolute())
    temporary = Path(tempfile.mkdtemp(prefix="pbai-primary-error-"))
    temporary.chmod(0o700)
    try:
        provider = temporary / "pblang"
        provider.mkdir()
        shutil.copytree(args.provider_stdlib or embedded / "stdlib", provider / "stdlib")
        project = temporary / args.product
        if args.product == "pbai":
            project.mkdir()
            shutil.copytree(root / "stdlib", project / "stdlib")
        for folder in {provider, project}:
            dep = "stdlib" if folder == provider else "../pblang/stdlib"
            (folder / "pblang.toml").write_text('[package]\nname="acquisition-fixture"\nversion="0.1.0"\nentry="main.sapp"\n[dependencies]\nstdlib={path=' + json.dumps(dep) + '}\n')
            (folder / "main.sapp").write_text('print "provider fixture";\n')
        fixtures = root / "tests/fixtures"
        (project / "main.sapp").write_text((fixtures / "pbai_checkpoint_primary.sapp").read_text())
        scope = provider / "stdlib/pbai_checkpoint_scope.sapp"
        if args.baseline_owner:
            scope.write_bytes(args.baseline_owner.read_bytes())
        scope_digest = hashlib.sha256(scope.read_bytes()).hexdigest()
        if args.expected_owner_sha256:
            assert scope_digest == args.expected_owner_sha256, "scope pin mismatch"
        owner = provider / "stdlib/pbai_neural_adapter.sapp"
        original = owner.read_text()
        signature = "fn student_load(artifact) {"
        assert original.count(signature) == 1
        hooks = (fixtures / "pbai_checkpoint_primary_hooks.sapp").read_text()
        hooks += "\nfn student_load(artifact){return _primary_wrap(_diagnostic_original_load(artifact));}\n"
        assert original.count("fn _number(") == 1
        owner.write_text(original.replace(signature, "fn _diagnostic_original_load(artifact) {", 1)
                         .replace("fn _number(", hooks + "\nfn _number(", 1))
        private = temporary / "private"
        private.mkdir(mode=0o700)
        for name, text in {"encoding": "{", "checkpoint": "{}", "duplicate": '{"format":1,"format":2}'}.items():
            path = private / (name + ".json")
            path.write_text(text)
            path.chmod(0o600)
        (temporary / "state").mkdir(mode=0o700)
        env = os.environ.copy()
        env.update(PBAI_STATE_ROOT=str(temporary / "state"),
                   PBAI_CHECKPOINT_TEST_DIR=str(private), PBAI_PRIMARY_PHASE=args.phase)
        if project != provider:
            run([runtime, "build"], provider, env)
        run([runtime, "build"], project, env)
        flags = ["--deny-net", "--deny-http", "--deny-spawn", "--deny-gfx", "--deny-sqlite",
                 "--allow-exec", "--ffi-lib=/usr/lib/x86_64-linux-gnu/libc.so.6"]
        flags += ["--ffi-symbol=" + name for name in
                  ("free", "close", "strdup", "openat", "getpid", "getuid", "dup", "unlinkat", "memfd_create", "sendfile", "lseek")]
        command = ["/usr/bin/bwrap", "--die-with-parent", "--unshare-net", "--ro-bind", "/", "/",
                   "--dev", "/dev", "--bind", str(temporary), str(temporary)]
        for hidden in sorted({str(root), str(embedded), os.environ.get("PBAI_SOURCE_HIDE_ROOT", "")} - {""}):
            command += ["--tmpfs", hidden]
        command += ["--chdir", str(project), runtime, "run", str(project / "main.sapp"), *flags]
        observation = json.loads(run(command, project, env))
        assert observation["format"] == "pbai.checkpoint-primary.v1"
        assert len(observation["rows"]) == 32
        serialized = json.dumps(observation, sort_keys=True)
        assert "synthetic explicit primary" in serialized and str(private) not in serialized
        file_digests = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in private.iterdir()}
        result = dict(product=args.product, phase=args.phase, observation=observation,
                      owner_sha256=scope_digest, file_digests=file_digests, build_notes=BUILD_NOTES, source_hidden=True, network_denied=True,
                      profile="existing Linux private-file exec and exact libc/11-symbol qualification")
    finally:
        shutil.rmtree(temporary)
    assert not temporary.exists()
    result["private_fixture_removed"] = True
    print(json.dumps(result, sort_keys=True))


if __name__ == "__main__":
    main()
