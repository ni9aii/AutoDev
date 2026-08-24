use std::fmt;
use std::str::FromStr;

/// Finding severity, ordered most-severe first (`Critical` < `Important` < `Minor`)
/// so that `sort()` places critical findings at the top.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    Critical,
    Important,
    Minor,
}

impl Severity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Critical => "CRITICAL",
            Severity::Important => "IMPORTANT",
            Severity::Minor => "MINOR",
        }
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Severity {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_uppercase().as_str() {
            "CRITICAL" => Ok(Severity::Critical),
            "IMPORTANT" => Ok(Severity::Important),
            "MINOR" => Ok(Severity::Minor),
            other => Err(format!("Unknown severity: {}", other)),
        }
    }
}
