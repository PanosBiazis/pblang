"""Linux-only real-caller ownership gate with private source copies and no network."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import tempfile


def run(args, cwd, env):
    process = subprocess.Popen(args, cwd=cwd, env=env, stdout=subprocess.PIPE,
                               stderr=subprocess.PIPE, text=True, start_new_session=True)
    try:
        stdout, stderr = process.communicate(timeout=20)
    except (subprocess.TimeoutExpired, KeyboardInterrupt):
        os.killpg(process.pid, signal.SIGINT)
        try:
            process.communicate(timeout=2)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            process.communicate()
        raise RuntimeError("bounded caller gate timed out") from None
    if process.returncode:
        raise RuntimeError("caller gate command failed: " + stderr[-1500:])
    return stdout


def quality(value):
    # Omit only explicitly reported wall-time measurements, never quality data.
    if isinstance(value, dict):
        return {k: quality(v) for k, v in value.items()
                if k not in {"samples_ms", "median_ms", "p95_sample_ms"}}
    if isinstance(value, list):
        return [quality(v) for v in value]
    return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--product", choices=["pblang", "pbai"], required=True)
    parser.add_argument("--target", choices=["checkpoint", "continuation", "review", "review_close", "checkpoint_reload_reviewed", "tools", "distillation", "supervised"], required=True)
    parser.add_argument("--kind", choices=["example", "benchmark"], default="example")
    parser.add_argument("--phase", choices=["baseline", "fixed"], default="fixed")
    args = parser.parse_args()
    if os.name != "posix" or os.uname().sysname != "Linux":
        raise SystemExit("This source-hidden observer requires Linux and bubblewrap")
    root = Path(__file__).resolve().parents[2]
    embedded = root if args.product == "pblang" else root.parent / "pblang"
    runtime = Path(os.environ["PBLANG_BIN"]).resolve(strict=True)
    directory = "bench" if args.kind == "benchmark" else ("examples" if args.product == "pblang" else "demo")
    pure_targets = {"tools": "pbai_student_tools.sapp", "distillation": "pbai_neural_distillation.sapp", "supervised": "pbai_specialist_supervised_distillation.sapp"}
    caller = root / directory / pure_targets.get(args.target, "pbai_student_" + args.target + ".sapp")
    if args.kind == "benchmark" and (args.target in {"review_close", "checkpoint_reload_reviewed"} or args.target in pure_targets):
        parser.error("remaining-caller targets are examples only")
    source = caller.read_text()
    fixtures = root / "tests/fixtures"
    temporary = Path(tempfile.mkdtemp(prefix="pbai-caller-owned-"))
    os.chmod(temporary, 0o700)
    try:
        private_embedded = temporary / "pblang"
        private_embedded.mkdir()
        shutil.copytree(embedded / "stdlib", private_embedded / "stdlib")
        project = temporary / args.product
        if args.product == "pbai":
            # stdlib: imports resolve at their owning package, not the caller.
            (private_embedded / "pblang.toml").write_text('[package]\nname="caller-provider-fixture"\nversion="0.1.0"\nentry="main.sapp"\n[dependencies]\nstdlib={path="stdlib"}\n')
            (private_embedded / "main.sapp").write_text('print "provider fixture";\n')
            project.mkdir()
            shutil.copytree(root / "stdlib", project / "stdlib")
        dependency = "stdlib" if args.product == "pblang" else "../pblang/stdlib"
        (project / "pblang.toml").write_text('[package]\nname="caller-lifetime-fixture"\nversion="0.1.0"\nentry="main.sapp"\n[dependencies]\nstdlib={path=' + json.dumps(dependency) + '}\n')
        owner = private_embedded / "stdlib/pbai_neural_adapter.sapp"
        provider = owner.read_text()
        hooks = (fixtures / "pbai_caller_lifetime_hooks.sapp").read_text()
        signatures = [
            ("_distill", "teacher_artifact, training, validation, options, supervised", "teacher_artifact,training,validation,options,supervised", "distill"),
            ("student_load", "artifact", "artifact", "loaded"),
            ("student_continue", "artifact, training, validation, options=nil", "artifact,training,validation,options", "continuation"),
            ("student_continue_reviewed", "artifact, history, training, validation, options=nil", "artifact,history,training,validation,options", "review"),
        ]
        for name, parameters, arguments, kind in signatures:
            signature = "fn " + name + "(" + parameters + ") {"
            assert provider.count(signature) == 1, name
            original = "_caller_original" + name
            provider = provider.replace(signature, "fn " + original + "(" + parameters + ") {", 1)
            hooks += "\nfn " + name + "(" + parameters + "){_caller_probe_acquire(" + json.dumps(kind) + ");return _caller_probe_wrap(" + original + "(" + arguments + ")," + json.dumps(kind) + ");}\n"
        assert provider.count("fn _number(") == 1
        owner.write_text(provider.replace("fn _number(", hooks + "\nfn _number(", 1))
        scenario_name = "pbai_remaining_caller_scenario.sapp" if args.target in {"review_close", "checkpoint_reload_reviewed"} else "pbai_caller_lifetime_scenario.sapp"
        if args.target in pure_targets:
            scenario_name = "pbai_tool_distill_caller_scenario.sapp"
        scenario = (fixtures / scenario_name).read_text()
        placeholder = 'fn _caller_workflow(){throw "test harness must supply the real caller";}'
        assert scenario.count(placeholder) == 1
        (project / "main.sapp").write_text(scenario.replace(placeholder, "fn _caller_workflow(){\n" + source + "\n}", 1))
        env = os.environ.copy()
        env.update(PBAI_STATE_ROOT=str(temporary / "state"), PBAI_CALLER_TARGET=args.target,
                   PBAI_CALLER_BENCHMARK="1" if args.kind == "benchmark" else "0",
                   PBAI_CALLER_LIFETIME_PHASE=args.phase)
        (temporary / "state").mkdir(mode=0o700)
        if args.target == "checkpoint_reload_reviewed":
            (temporary / "checkpoints").mkdir(mode=0o700)
            env.update(PBAI_CHECKPOINT_TEST_DIR=str(temporary / "checkpoints"), PBAI_RELOAD_PHASE="prepare")
        if args.product == "pbai":
            run([str(runtime), "build"], private_embedded, env)
        run([str(runtime), "build"], project, env)
        command = ["/usr/bin/bwrap", "--die-with-parent", "--unshare-net", "--ro-bind", "/", "/",
                   "--dev", "/dev", "--bind", str(temporary), str(temporary)]
        for hidden in sorted({str(root), str(embedded), os.environ.get("PBAI_SOURCE_HIDE_ROOT", "")} - {""}):
            command += ["--tmpfs", hidden]
        capabilities = ["--allow-all", "--deny-net", "--deny-http", "--deny-sqlite"]
        if args.target in pure_targets:
            capabilities = ["--deny-" + name for name in ("net", "http", "exec", "ffi", "spawn", "gfx", "sqlite")]
        command += ["--chdir", str(project), str(runtime), "run", str(project / "main.sapp"), *capabilities]
        stdout = run(command, project, env)
        lines = [json.loads(line) for line in stdout.splitlines() if line.strip()]
        observation = lines.pop()
        assert observation["format"] == "pbai.caller-lifetime.v1"
        values = quality(lines)
        payload = json.dumps(values, sort_keys=True, separators=(",", ":"))
        result = {"product": args.product, "target": args.target, "kind": args.kind,
                  "phase": args.phase, "observer": observation, "quality_outputs": values,
                  "raw_outputs": lines,
                  "quality_digest": hashlib.sha256(payload.encode()).hexdigest(),
                  "caller_sha256": hashlib.sha256(source.encode()).hexdigest(),
                  "source_hidden": True, "network_denied": True}
    finally:
        shutil.rmtree(temporary)
    assert not temporary.exists()
    result["private_fixture_removed"] = True
    print(json.dumps(result, sort_keys=True))


if __name__ == "__main__":
    main()
