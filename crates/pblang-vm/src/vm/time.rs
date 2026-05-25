//! Time / sleep builtin implementations.
//!
//! Free functions backing the time-related builtins registered in
//! `VM::builtins()` (vm.rs):
//!
//!   - `clock()` — Unix seconds, lossless `f64` from `SystemTime`.
//!   - `now_ms()` — milliseconds since the epoch. Uses `secs_f64 * 1000`
//!     rather than `as_millis()` to avoid precision loss past 2^53 ms.
//!   - `now_iso()` — ISO-8601 timestamp string (`YYYY-MM-DDTHH:MM:SSZ`).
//!     The (y, m, d) decomposition uses the O(1) Howard Hinnant
//!     `civil_from_days` formula instead of a year-by-year loop, so far-
//!     future timestamps stay fast.
//!   - `sleep_ms(n)` — `std::thread::sleep` for `n` ms. Gated on
//!     `host.allow_exec()` (sleeping changes process scheduling, same
//!     spirit as a tiny exec).
//!
//! Pure or quasi-pure — `clock`/`now_ms`/`now_iso` need no host caps;
//! `sleep_ms` gates on exec.

use pblang_ir::{alloc_string, RuntimeError, Value, VmHost};

pub(super) fn clock(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if !args.is_empty() {
        return Err(host.err("clock() takes no arguments".into()));
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| host.err("clock(): system time before UNIX_EPOCH".into()))?;
    Ok(Value::Number(now.as_secs_f64()))
}

pub(super) fn now_ms(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if !args.is_empty() {
        return Err(host.err("now_ms() takes no arguments".into()));
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| host.err("now_ms(): system time before UNIX_EPOCH".into()))?;
    // BUG FIX #16: as_millis() returns u128; casting to f64 loses precision
    // beyond 2^53 ms (~year 2255). Use secs_f64 * 1000 for lossless conversion.
    Ok(Value::Number(now.as_secs_f64() * 1000.0))
}

pub(super) fn now_iso(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if !args.is_empty() {
        return Err(host.err("now_iso() takes no arguments".into()));
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| host.err("now_iso(): system time before UNIX_EPOCH".into()))?;
    let secs = now.as_secs();
    // FIX: Replaced O(years) year-by-year loop with O(1) Gregorian formula.
    // The old implementation would be slow for timestamps far in the future.
    let s_in_day = secs % 86400;
    let hours = s_in_day / 3600;
    let minutes = (s_in_day % 3600) / 60;
    let seconds = s_in_day % 60;

    // Algorithm: civil_from_days (Howard Hinnant, public domain).
    // Computes (year, month, day) from a Unix day count in O(1).
    let z: i64 = (secs / 86400) as i64 + 719468;
    let era: i64 = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64; // day of era [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // year of era [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // day of year [0, 365]
    let mp = (5 * doy + 2) / 153; // month of year [0, 11]
    let day = doy - (153 * mp + 2) / 5 + 1; // day [1, 31]
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if month <= 2 { y + 1 } else { y };
    let (y, month, day) = (y as u64, month, day);
    let iso = format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y, month, day, hours, minutes, seconds
    );
    Ok(alloc_string(iso))
}

pub(super) fn sleep_ms(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if !host.allow_exec() {
        return Err(host.err("exec access denied".into()));
    }
    if args.len() != 1 {
        return Err(host.err(format!("sleep_ms() expects 1 argument, got {}", args.len())));
    }
    let ms = match args[0] {
        Value::Number(n) if n.is_finite() && n >= 0.0 => n as u64,
        _ => return Err(host.err("sleep_ms(): expected non-negative number".into())),
    };
    std::thread::sleep(std::time::Duration::from_millis(ms));
    Ok(Value::Nil)
}
