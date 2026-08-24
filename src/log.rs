use colored::Colorize;
use std::sync::atomic::{AtomicBool, Ordering};

static NO_COLOR: AtomicBool = AtomicBool::new(false);

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
