#!/usr/bin/env bash
# Comprehensive pblang phase/resource/backend benchmark. Produces machine-readable
# evidence while failing closed on any correctness or tool execution error.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PB="${PB:-$ROOT/target/release/pblang}"
STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
OUT="${1:-$ROOT/bench/results/phase-$STAMP}"
mkdir -p "$OUT"

command -v /usr/bin/time >/dev/null
[ -x "$PB" ]

{
    echo "utc=$STAMP"
    echo "kernel=$(uname -srmo)"
    echo "cpu=$(lscpu | sed -n 's/^Model name:[[:space:]]*//p' | head -1)"
    echo "logical_cpus=$(nproc)"
    echo "memory=$(free -h | awk '/^Mem:/{print $2}')"
    "$PB" --version 2>&1 | sed 's/^/pblang=/'
} > "$OUT/environment.txt"

/usr/bin/time -f 'phase_process_elapsed_s=%e\nphase_process_peak_rss_kb=%M\nphase_process_user_s=%U\nphase_process_sys_s=%S' \
    -o "$OUT/phase_resources.txt" \
    "$PB" run "$ROOT/bench/pblang_phase_stages.sapp" --allow-all \
    | tee "$OUT/phases.txt"

grep -q '^PHASE_SUITE|ok$' "$OUT/phases.txt"

/usr/bin/time -f 'selfhost_c99_process_elapsed_s=%e\nselfhost_c99_process_peak_rss_kb=%M\nselfhost_c99_process_user_s=%U\nselfhost_c99_process_sys_s=%S' \
    -o "$OUT/selfhost_c99_resources.txt" \
    "$PB" run "$ROOT/bench/selfhost_c99_stages.sapp" --allow-exec \
    | tee "$OUT/selfhost_c99.txt"
grep -q '^C99_STAGE_SUITE|ok|' "$OUT/selfhost_c99.txt"

run_tree_fixture() {
    local name="$1" script="$2" marker="$3"
    /usr/bin/time -f "${name}_process_elapsed_s=%e\n${name}_process_peak_rss_kb=%M\n${name}_process_user_s=%U\n${name}_process_sys_s=%S" \
        -o "$OUT/${name}_resources.txt" \
        "$PB" run "$ROOT/bench/$script" --allow-all \
        | tee "$OUT/${name}.txt"
    grep -Fqx "$marker" "$OUT/${name}.txt"
}

run_tree_fixture tree_interpreter_closure tree_interpreter_closure_stages.sapp 'CLOSURE_SUITE|ok'
run_tree_fixture tree_interpreter_lexical tree_interpreter_lexical_cache_stages.sapp 'LEXICAL_CACHE_SUITE|ok'
run_tree_fixture tree_interpreter_control tree_interpreter_control_stages.sapp 'CONTROL_SUITE|ok'
run_tree_fixture tree_interpreter_layout tree_interpreter_closure_layout_stages.sapp 'CLOSURE_LAYOUT_SUITE|ok'
run_tree_fixture app_facade app_facade_stages.sapp 'APP_PHASE_SUITE|ok'
run_tree_fixture polyglot_control_plane polyglot_control_plane_stages.sapp 'POLYGLOT_CONTROL_PLANE_SUITE|ok'
run_tree_fixture ecosystem_vertical ecosystem_vertical_stages.sapp 'ECOSYSTEM_VERTICAL_SUITE|ok'
run_tree_fixture stdlib_hotpaths stdlib_hotpaths_stages.sapp 'STDLIB_HOTPATH_SUITE|ok'
run_tree_fixture stdlib_pipeline_hotpaths stdlib_pipeline_hotpaths_stages.sapp 'STDLIB_PIPELINE_HOTPATH_SUITE|ok'
run_tree_fixture stdlib_data_pipeline_hotpaths stdlib_data_pipeline_hotpaths_stages.sapp 'DATA_PIPELINE_HOTPATH_SUITE|ok'
run_tree_fixture stdlib_safety_resource stdlib_safety_resource_stages.sapp 'SAFETY_RESOURCE_SUITE|ok'
run_tree_fixture product_runtime product_runtime_stages.sapp 'PRODUCT_RUNTIME_SUITE|ok'
run_tree_fixture cache_render cache_render_stages.sapp 'CACHE_RENDER_SUITE|ok'
run_tree_fixture platform_resource platform_resource_stages.sapp 'PLATFORM_RESOURCE_SUITE|ok'

/usr/bin/time -f 'platform_emulator_process_elapsed_s=%e\nplatform_emulator_peak_rss_kb=%M\nplatform_emulator_user_s=%U\nplatform_emulator_sys_s=%S' \
    -o "$OUT/platform_emulator_resources.txt" \
    "$PB" run "$ROOT/bench/platform_emulator_matrix_stages.sapp" --allow-all \
    | tee "$OUT/platform_emulator.txt"
grep -q '^PLATFORM_EMULATOR_MATRIX_SUITE|ok$' "$OUT/platform_emulator.txt"

chmod +x /tmp/pblang_phase_x64
native_result="$(/tmp/pblang_phase_x64)"
[ "$native_result" = "6765" ]
/usr/bin/time -f 'direct_x64_elapsed_s=%e\ndirect_x64_peak_rss_kb=%M\ndirect_x64_user_s=%U\ndirect_x64_sys_s=%S' \
    -o "$OUT/direct_x64_resources.txt" /tmp/pblang_phase_x64 >/dev/null
stat -c 'direct_x64_artifact_bytes=%s' /tmp/pblang_phase_x64 >> "$OUT/direct_x64_resources.txt"

# A longer recursive workload separates generated-code throughput from the
# millisecond-scale process-startup floor. Keep every sample and correctness
# gate instead of reporting only the most favorable run.
chmod +x /tmp/pblang_phase_x64_stress
printf 'run,elapsed_ms,result\n' > "$OUT/direct_x64_fib36.csv"
for run in 1 2 3 4 5; do
    start_ns="$(date +%s%N)"
    stress_result="$(/tmp/pblang_phase_x64_stress)"
    end_ns="$(date +%s%N)"
    [ "$stress_result" = "14930352" ]
    printf '%s,%s,%s\n' "$run" "$(( (end_ns - start_ns) / 1000000 ))" "$stress_result" >> "$OUT/direct_x64_fib36.csv"
done
/usr/bin/time -f 'direct_x64_fib36_peak_rss_kb=%M\ndirect_x64_fib36_user_s=%U\ndirect_x64_fib36_sys_s=%S' \
    -o "$OUT/direct_x64_fib36_resources.txt" /tmp/pblang_phase_x64_stress >/dev/null
stat -c 'direct_x64_fib36_artifact_bytes=%s' /tmp/pblang_phase_x64_stress >> "$OUT/direct_x64_fib36_resources.txt"

start_ns="$(date +%s%N)"
"$PB" compile /tmp/pblang_phase_source.sapp --out /tmp/pblang_phase_c99 >/dev/null
end_ns="$(date +%s%N)"
c99_compile_ms="$(( (end_ns - start_ns) / 1000000 ))"
c99_result="$(/tmp/pblang_phase_c99)"
[ "$c99_result" = "6765" ]
{
    echo "c99_compile_ms=$c99_compile_ms"
    stat -c 'c99_artifact_bytes=%s' /tmp/pblang_phase_c99
    stat -c 'selfhost_c_source_bytes=%s' /tmp/pblang_phase_c99.c
} > "$OUT/c99_resources.txt"
/usr/bin/time -f 'c99_elapsed_s=%e\nc99_peak_rss_kb=%M\nc99_user_s=%U\nc99_sys_s=%S' \
    -a -o "$OUT/c99_resources.txt" /tmp/pblang_phase_c99 >/dev/null

emu_tests=(emu_6502 emu_avr emu_z80 emu_mips emu_msp430 emu_powerpc emu_riscv emu_sparc emu_wasm emu_quantum platform_emulator)
printf 'emulator,status,elapsed_s,peak_rss_kb\n' > "$OUT/emulators.csv"
for name in "${emu_tests[@]}"; do
    metric="$(mktemp)"
    if /usr/bin/time -f '%e,%M' -o "$metric" "$PB" run "$ROOT/tests/semantics/${name}_test.sapp" --allow-all >/dev/null; then
        printf '%s,ok,%s\n' "$name" "$(cat "$metric")" >> "$OUT/emulators.csv"
    else
        printf '%s,fail,-,-\n' "$name" >> "$OUT/emulators.csv"
        rm -f "$metric"
        exit 1
    fi
    rm -f "$metric"
done

rm -f /tmp/pblang_phase_x64 /tmp/pblang_phase_x64_stress /tmp/pblang_phase_c99 /tmp/pblang_phase_c99.c /tmp/pblang_phase_source.sapp
printf 'Phase benchmark evidence written to %s\n' "$OUT"
