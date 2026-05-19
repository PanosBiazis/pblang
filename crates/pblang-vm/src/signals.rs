//! Process-wide OS signal trapping for pblang programs.
//!
//! Signal handlers run in async-signal context, so they cannot safely call back
//! into the VM. Instead this module installs a tiny C-compatible handler that
//! atomically increments a per-signal pending counter. pblang programs poll
//! the counters via the `os_signal_poll()` / `os_signal_pending()` builtins
//! and dispatch their own registered handler closures from regular bytecode
//! context (see `stdlib/signal.sapp`).
//!
//! Supported signal names (cross-platform subset):
//!
//! | name      | unix       | windows                          |
//! |-----------|------------|----------------------------------|
//! | SIGINT    | SIGINT (2) | CTRL_C_EVENT, CTRL_BREAK_EVENT   |
//! | SIGTERM   | SIGTERM(15)| CTRL_CLOSE_EVENT                 |
//! | SIGHUP    | SIGHUP (1) | (unsupported, returns error)     |
//! | SIGUSR1   | SIGUSR1    | (unsupported, returns error)     |
//! | SIGUSR2   | SIGUSR2    | (unsupported, returns error)     |
//! | SIGPIPE   | SIGPIPE    | (unsupported, returns error)     |
//! | SIGQUIT   | SIGQUIT    | (unsupported, returns error)     |
//!
//! Signal counters are process-global by necessity; embedding multiple VMs in
//! one process and trapping the same signal from each will share counters.

use std::sync::atomic::{AtomicU32, Ordering};

const MAX_SIGNALS: usize = 32;

static SIGNAL_PENDING: [AtomicU32; MAX_SIGNALS] = {
    // initializer pattern compatible with stable Rust 1.83+
    [
        AtomicU32::new(0),
        AtomicU32::new(0),
        AtomicU32::new(0),
        AtomicU32::new(0),
        AtomicU32::new(0),
        AtomicU32::new(0),
        AtomicU32::new(0),
        AtomicU32::new(0),
        AtomicU32::new(0),
        AtomicU32::new(0),
        AtomicU32::new(0),
        AtomicU32::new(0),
        AtomicU32::new(0),
        AtomicU32::new(0),
        AtomicU32::new(0),
        AtomicU32::new(0),
        AtomicU32::new(0),
        AtomicU32::new(0),
        AtomicU32::new(0),
        AtomicU32::new(0),
        AtomicU32::new(0),
        AtomicU32::new(0),
        AtomicU32::new(0),
        AtomicU32::new(0),
        AtomicU32::new(0),
        AtomicU32::new(0),
        AtomicU32::new(0),
        AtomicU32::new(0),
        AtomicU32::new(0),
        AtomicU32::new(0),
        AtomicU32::new(0),
        AtomicU32::new(0),
    ]
};

/// Translate a textual signal name (case-insensitive, accepts with or without
/// the `SIG` prefix) into a platform-appropriate numeric code.
///
/// On unsupported platforms this returns `Err` so callers can surface a clean
/// error to the pblang program.
pub fn signal_number(name: &str) -> Result<i32, String> {
    let upper = name.trim().to_ascii_uppercase();
    let stripped = upper.strip_prefix("SIG").unwrap_or(&upper);
    #[cfg(unix)]
    {
        Ok(match stripped {
            "INT" => libc::SIGINT,
            "TERM" => libc::SIGTERM,
            "HUP" => libc::SIGHUP,
            "USR1" => libc::SIGUSR1,
            "USR2" => libc::SIGUSR2,
            "PIPE" => libc::SIGPIPE,
            "QUIT" => libc::SIGQUIT,
            "ALRM" => libc::SIGALRM,
            "CHLD" => libc::SIGCHLD,
            "WINCH" => libc::SIGWINCH,
            _ => return Err(format!("unknown signal: {name}")),
        })
    }
    #[cfg(windows)]
    {
        Ok(match stripped {
            "INT" => 2,
            "TERM" => 15,
            // Treat BREAK as SIGINT-ish on windows so users can ctrl-break.
            "BREAK" => 21,
            _ => return Err(format!("signal {name} not supported on this platform")),
        })
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = stripped;
        Err(format!("signal {name} not supported on this platform"))
    }
}

/// Increment the pending-signal counter for `sig`. Safe to call from an
/// async-signal-handler context; only uses relaxed atomic adds.
fn record(sig: i32) {
    let idx = sig as usize;
    if idx < MAX_SIGNALS {
        SIGNAL_PENDING[idx].fetch_add(1, Ordering::Relaxed);
    }
}

/// Install a real OS signal handler for the given signal. Returns Ok(()) on
/// success or an Err with a human-readable message.
pub fn install(sig: i32) -> Result<(), String> {
    #[cfg(unix)]
    {
        // libc::signal returns SIG_ERR ((-1) cast) on failure.
        // Use a ptr-cast through `*const ()` to avoid the
        // `function_casts_as_integer` lint when going to sighandler_t (usize).
        let f = handler_unix as extern "C" fn(libc::c_int) as *const ();
        let h = f as libc::sighandler_t;
        unsafe {
            let prev = libc::signal(sig, h);
            if prev == libc::SIG_ERR {
                return Err(format!("libc::signal failed for {sig}"));
            }
        }
        Ok(())
    }
    #[cfg(windows)]
    {
        // The only cross-platform signal we can reasonably hook on Windows is
        // ctrl-c (and break). For other "signals" we just record the
        // installation and rely on polling never seeing pending events.
        install_console_ctrl_handler();
        let _ = sig;
        Ok(())
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = sig;
        Err("OS signals are not supported on this platform".to_string())
    }
}

#[cfg(unix)]
extern "C" fn handler_unix(sig: libc::c_int) {
    record(sig);
}

#[cfg(windows)]
fn install_console_ctrl_handler() {
    use std::sync::Once;
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        extern "system" fn ctrl_handler(ctrl_type: u32) -> i32 {
            // Map CTRL_C_EVENT (0) and CTRL_BREAK_EVENT (1) to SIGINT (2).
            // Map CTRL_CLOSE_EVENT (2) to SIGTERM (15).
            match ctrl_type {
                0 | 1 => record(2),
                2 => record(15),
                _ => return 0,
            }
            1
        }
        // SAFETY: SetConsoleCtrlHandler is FFI to the Windows API. Calling
        // it with a non-null handler installs `ctrl_handler` as a listener.
        unsafe {
            extern "system" {
                fn SetConsoleCtrlHandler(handler: extern "system" fn(u32) -> i32, add: i32) -> i32;
            }
            SetConsoleCtrlHandler(ctrl_handler, 1);
        }
    });
}

/// Reset (and read) the pending count for `sig`. Returns 0 if `sig` is out of
/// range for the internal counter table.
pub fn take_pending(sig: i32) -> u32 {
    let idx = sig as usize;
    if idx >= MAX_SIGNALS {
        return 0;
    }
    SIGNAL_PENDING[idx].swap(0, Ordering::Relaxed)
}

/// Peek (without resetting) the pending count for `sig`.
pub fn peek_pending(sig: i32) -> u32 {
    let idx = sig as usize;
    if idx >= MAX_SIGNALS {
        return 0;
    }
    SIGNAL_PENDING[idx].load(Ordering::Relaxed)
}
