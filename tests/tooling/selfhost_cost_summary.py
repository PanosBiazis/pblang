"""Validate and summarize paired conversion-cost receipts, without rerunning them.

This describes observations only. It neither authenticates the measuring process
nor attributes a difference to a source revision. See SELFHOST_CONVERSION_COST.md.
"""
import argparse
import hashlib
import json
import math
from pathlib import Path
import statistics

PRIMARY_FIXTURE = "4d1ade5412d8b04539c409ddf8cebf14b259f00097143849368a32e61e208a83"
STAGE_FIXTURE = "5348b4beed7b8f6311e6c7af29f6af57f463fdead43d6b3a2e4b955b6095fde1"
METRICS = ("cpu_s", "wall_s", "maxrss_kib")


def require(condition, message):
    if not condition:
        raise ValueError(message)


def number(value):
    return type(value) in (float, int) and math.isfinite(value) and value >= 0


def summarize(data, stages=False):
    require(type(data) is dict, "receipt must be an object")
    require(data.get("format") == "cycle16.conversion-measurement.v1", "unsupported receipt format")
    require(data.get("group") == "escape", "unexpected workload")
    for key in ("passed", "source_hidden", "release_hashes_unchanged", "fixtures_removed"):
        require(data.get(key) is True, "unsuccessful or incomplete receipt: " + key)
    require(data.get("fixture_sha256") == (STAGE_FIXTURE if stages else PRIMARY_FIXTURE), "fixture changed")
    require(data.get("samples_per_variant") == 11 and data.get("warmups_per_variant") == 2, "sample contract changed")
    require(data.get("limits") == {"group_s": 50, "sample_s": 5}, "measurement bounds changed")
    rows = data.get("rows")
    require(type(rows) is list and len(rows) == 26, "expected all 26 invocations")
    samples = {"baseline": [], "candidate": []}
    for index, row in enumerate(rows):
        require(type(row) is dict, "invalid sample")
        pair = index // 2
        order = ("baseline", "candidate") if pair % 2 == 0 else ("candidate", "baseline")
        require(row.get("pair") == pair and row.get("variant") == order[index % 2], "pair sequence changed")
        require(row.get("warmup") is (pair < 2), "warmup classification changed")
        require(row.get("exit") == 0 and row.get("stdout") == "escape_benchmark_pass\n"
                and row.get("stderr") == "" and row.get("first_error") is None,
                "sample oracle failed")
        require(row.get("timed_out") is False and row.get("owned_group_remaining") is False,
                "sample timed out or left an owned process")
        for metric in (*METRICS, "cpu_user_s", "cpu_system_s"):
            require(number(row.get(metric)), "invalid metric: " + metric)
        require(all(row[key] > 0 for key in METRICS), "zero measurement")
        require(math.isclose(row["cpu_s"], row["cpu_user_s"] + row["cpu_system_s"], abs_tol=1e-6),
                "CPU accounting mismatch")
        if stages:
            phase = row.get("stage_ms")
            require(type(phase) is list and len(phase) == 3 and all(number(v) for v in phase),
                    "invalid stage durations")
            require(sum(phase) > 0 and sum(phase) <= row["wall_s"] * 1000 + 1,
                    "stage durations exceed whole invocation")
        if pair >= 2:
            samples[row["variant"]].append(row)
    summary = {}
    for metric in METRICS:
        baseline = [r[metric] for r in samples["baseline"]]
        candidate = [r[metric] for r in samples["candidate"]]
        summary[metric] = {
            "baseline_median": statistics.median(baseline),
            "candidate_median": statistics.median(candidate),
            "median_ratio_change_percent": 100 * (statistics.median(candidate) / statistics.median(baseline) - 1),
            "paired_median_change_percent": statistics.median(100 * (b / a - 1) for a, b in zip(baseline, candidate)),
            "baseline_range": [min(baseline), max(baseline)],
            "candidate_range": [min(candidate), max(candidate)],
        }
    result = {"summary": summary, "measured_invocations": 22, "warmup_invocations": 4,
              "loadavg_before": data.get("loadavg_before"), "loadavg_after": data.get("loadavg_after"),
              "baseline": data.get("baseline"), "candidate": data.get("candidate"),
              "causal_attribution": False, "speedup_established": False,
              "p95_note": "For 11 samples nearest-rank p95 is the maximum, not an independent tail estimate."}
    if stages:
        result["stages"] = {
            variant: {"median_wall_ms": dict(zip(("imports", "compile", "run_100"),
                          (statistics.median(r["stage_ms"][i] for r in values) for i in range(3)))),
                      "median_import_fraction_of_instrumented_interval": statistics.median(
                          r["stage_ms"][0] / sum(r["stage_ms"]) for r in values)}
            for variant, values in samples.items()
        }
    return result


def read_receipt(path):
    # Bound allocation even when the input grows between stat and read.
    with path.open("rb") as stream:
        raw = stream.read(1024 * 1024 + 1)
    require(len(raw) <= 1024 * 1024, "receipt exceeds 1 MiB")
    return json.loads(raw), hashlib.sha256(raw).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("receipt", type=Path)
    parser.add_argument("--stages", action="store_true", help="validate the separate instrumented fixture")
    args = parser.parse_args()
    try:
        data, digest = read_receipt(args.receipt)
        result = summarize(data, args.stages)
    except (OSError, ValueError, TypeError, KeyError) as exc:
        parser.error(str(exc))
    result["input_sha256"] = digest
    print(json.dumps(result, indent=2, allow_nan=False))


if __name__ == "__main__":
    main()
