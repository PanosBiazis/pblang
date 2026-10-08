"""Contract tests: invalid measurements must not become success summaries."""
import copy
import unittest
from selfhost_cost_summary import PRIMARY_FIXTURE, STAGE_FIXTURE, summarize


def receipt():
    rows = []
    for pair in range(13):
        order = ("baseline", "candidate") if pair % 2 == 0 else ("candidate", "baseline")
        for variant in order:
            value = 1 if variant == "baseline" else 2
            rows.append(dict(pair=pair, variant=variant, warmup=pair < 2, exit=0,
                             stdout="escape_benchmark_pass\n", stderr="", first_error=None,
                             timed_out=False, owned_group_remaining=False, cpu_s=value,
                             cpu_user_s=value * .75, cpu_system_s=value * .25,
                             wall_s=value, maxrss_kib=1000 * value))
    return dict(format="cycle16.conversion-measurement.v1", group="escape", passed=True,
                source_hidden=True, release_hashes_unchanged=True, fixtures_removed=True,
                fixture_sha256=PRIMARY_FIXTURE, samples_per_variant=11,
                warmups_per_variant=2, limits=dict(group_s=50, sample_s=5), rows=rows)


class SummaryTests(unittest.TestCase):
    def test_ratios_and_no_causal_claim(self):
        result = summarize(receipt())
        self.assertEqual(result["summary"]["cpu_s"]["paired_median_change_percent"], 100)
        self.assertFalse(result["causal_attribution"])
        self.assertFalse(result["speedup_established"])

    def test_warmups_excluded_but_validated(self):
        data = receipt()
        for row in data["rows"][:4]:
            row.update(cpu_s=4, cpu_user_s=3, cpu_system_s=1, wall_s=4)
        self.assertEqual(summarize(data)["summary"]["cpu_s"]["baseline_median"], 1)
        data["rows"][0]["exit"] = 1
        with self.assertRaises(ValueError):
            summarize(data)

    def test_incomplete_reordered_or_failed_receipts_rejected(self):
        changes = [lambda d: d["rows"].pop(), lambda d: d["rows"].reverse(),
                   lambda d: d.update(passed=False), lambda d: d.update(limits=dict(group_s=60, sample_s=5)),
                   lambda d: d.update(fixture_sha256="0" * 64),
                   lambda d: d["rows"][5].update(warmup=True),
                   lambda d: d["rows"][5].update(timed_out=True),
                   lambda d: d["rows"][5].update(owned_group_remaining=True),
                   lambda d: d["rows"][5].update(cpu_s=float("nan")),
                   lambda d: d["rows"][5].update(cpu_user_s=90),
                   lambda d: d["rows"][5].update(maxrss_kib=True)]
        for change in changes:
            with self.subTest(change=change):
                data = receipt()
                change(data)
                with self.assertRaises(ValueError):
                    summarize(data)

    def test_stage_fixture_is_separate_and_finite(self):
        data = receipt()
        data["fixture_sha256"] = STAGE_FIXTURE
        for row in data["rows"]:
            row["stage_ms"] = [800, 10, 20]
        result = summarize(data, stages=True)
        self.assertEqual(result["stages"]["baseline"]["median_wall_ms"]["imports"], 800)
        with self.assertRaises(ValueError):
            summarize(data)
        for values in ([1, 2], [1, -1, 2], [float("inf"), 1, 2], [2000, 1, 2]):
            broken = copy.deepcopy(data)
            broken["rows"][0]["stage_ms"] = values
            with self.assertRaises(ValueError):
                summarize(broken, stages=True)


if __name__ == "__main__":
    unittest.main()
