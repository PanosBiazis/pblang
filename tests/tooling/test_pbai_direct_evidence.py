"""Regressions for acceptance evidence semantics; no runtime or user state."""
import contextlib
import importlib.util
import io
import json
from pathlib import Path
import stat
import tempfile
import unittest


spec = importlib.util.spec_from_file_location(
    "direct_gate", Path(__file__).with_name("pbai_checkpoint_file_direct_lifetime.py"))
gate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gate)


class EvidenceTests(unittest.TestCase):
    def test_attempt_is_not_release_and_recovery_is_not_a_fault(self):
        def case(mode, attempts, releases):
            return {"phase": "read", "mode": mode,
                    "observation": {"rows": [{"close_calls": attempts, "first_closes": releases}]}}
        results = [case("close_before", 1, 0), case("loaded_use_failure", 0, 0),
                   case("success", 2, 1), case("success", 2, 1)]
        metrics = gate.summarize(results)
        self.assertEqual(metrics["invocations"], 4)
        self.assertEqual(metrics["distinct_phase_modes"], 3)
        self.assertEqual(metrics["fault_invocations"], 2)
        self.assertEqual(metrics["missing_cleanup_attempt_fault_cases"], 1)
        self.assertEqual(metrics["unclosed_fault_cases"], 1)
        self.assertEqual(metrics["unconfirmed_release_cases"], 2)
        self.assertEqual(metrics["unconfirmed_release_handles"], 2)

    def test_durable_report_does_not_overwrite_failure_or_follow_symlink(self):
        with tempfile.TemporaryDirectory() as directory, contextlib.redirect_stdout(io.StringIO()):
            path = Path(directory) / "failure.json"
            gate.emit({"status": "failed"}, path)
            self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o600)
            before = path.read_bytes()
            with self.assertRaises(FileExistsError):
                gate.emit({"status": "passed"}, path)
            self.assertEqual(path.read_bytes(), before)
            link = Path(directory) / "alias.json"
            link.symlink_to(path)
            with self.assertRaises(FileExistsError):
                gate.emit({"status": "passed"}, link)
            self.assertEqual(json.loads(path.read_text())["status"], "failed")

    def test_provider_fingerprint_is_content_sensitive_and_path_independent(self):
        with tempfile.TemporaryDirectory() as directory:
            one, two = Path(directory) / "one", Path(directory) / "two"
            one.mkdir(); two.mkdir()
            (one / "module.sapp").write_text("let a=1;")
            (two / "module.sapp").write_text("let a=1;")
            self.assertEqual(gate.tree_fingerprint(one), gate.tree_fingerprint(two))
            (two / "module.sapp").write_text("let a=2;")
            self.assertNotEqual(gate.tree_fingerprint(one), gate.tree_fingerprint(two))


if __name__ == "__main__":
    unittest.main()
