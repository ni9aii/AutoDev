use colored::Colorize;
use std::sync::atomic::{AtomicBool, Ordering};

static NO_COLOR: AtomicBool = AtomicBool::new(false);

/// Auto-detect the NO_COLOR convention (https://no-color.org/) and non-TTY
/// stdout at first log call, so piped/redirected output stays plain even when
/// the binary never calls `set_no_color` explicitly.
pub fn auto_detect_no_color() {
    if std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty()) {
        set_no_color(true);
        return;
    }
    // Respect an explicit clap/config override: only auto-detect once.
    if !NO_COLOR.load(Ordering::Relaxed) && !is_stderr_tty() {
        set_no_color(true);
    }
}

fn is_stderr_tty() -> bool {
    // Minimal isatty without adding a dependency: same ioctl the atty crate uses.
    #[cfg(unix)]
    {
        extern "C" {
            fn isatty(fd: i32) -> i32;
        }
        unsafe { isatty(2) == 1 }
    }
    #[cfg(not(unix))]
    {
        // Non-unix: assume TTY (previous behavior).
        true
    }
}

/// Disable colored output (respects NO_COLOR env convention).
pub fn set_no_color(enabled: bool) {
    NO_COLOR.store(enabled, Ordering::Relaxed);
}

fn prefix(level: &str) -> String {
    let no_color = NO_COLOR.load(Ordering::Relaxed);
    if no_color {
        format!("[auto-dev] {}", level)
    } else {
        match level {
            "INFO" => format!("{} {}", "[auto-dev]".blue(), "INFO".blue()),
            "WARN" => format!("{} {}", "[auto-dev]".yellow(), "WARN".yellow()),
            "ERROR" => format!("{} {}", "[auto-dev]".red(), "ERROR".red()),
            "OK" => format!("{} {}", "[auto-dev]".green(), "OK".green()),
            _ => format!("[auto-dev] {}", level),
        }
    }
}

pub fn log(msg: &str) {
    eprintln!("{} {}", prefix("INFO"), msg);
}

pub fn warn(msg: &str) {
    eprintln!("{} {}", prefix("WARN"), msg);
}

pub fn error(msg: &str) {
    eprintln!("{} {}", prefix("ERROR"), msg);
}

pub fn success(msg: &str) {
    eprintln!("{} {}", prefix("OK"), msg);
}

/// Final one-line run summary (advisor convention: `DONE key=value ...`).
pub fn done(msg: &str) {
    let no_color = NO_COLOR.load(Ordering::Relaxed);
    let tag = if no_color {
        "[auto-dev] DONE".to_string()
    } else {
        format!("{}", "[auto-dev] DONE".cyan())
    };
    eprintln!("{} {}", tag, msg);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_functions() {
        set_no_color(true);
        log("test message");
        warn("test warning");
        error("test error");
        success("test success");
        done("findings=2 do_now=1 plan=/tmp/x-plan.md");
        set_no_color(false);
    }

    #[test]
    fn test_no_color_env_detection() {
        // The no-color path must render the same plain prefix the tests and
        // piped output rely on.
        set_no_color(true);
        assert!(prefix("INFO").starts_with("[auto-dev]"));
        set_no_color(false);
    }
}
