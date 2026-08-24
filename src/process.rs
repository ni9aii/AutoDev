use anyhow::{Context, Result};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Resolve an executable name to an absolute path via `$PATH`, so callers
/// never hand a bare name to `Command::new` (which trusts whatever the
/// current `PATH` resolves to — a hijack risk if `PATH` is attacker-controlled).
pub fn resolve_exe(name: &str) -> Result<PathBuf> {
    if name.contains('/') {
        return std::fs::canonicalize(name)
            .with_context(|| format!("Executable not found: {}", name));
    }
    let path_var = std::env::var_os("PATH").context("PATH environment variable not set")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            // Canonicalize the directory only, not the file itself: some
            // toolchains (e.g. rustup) ship `cargo`/`rustc` as symlinks to
            // a single multiplexer binary that dispatches on argv[0], so
            // fully resolving the symlink would rename the program and
            // break that dispatch.
            let canonical_dir = std::fs::canonicalize(&dir)
                .with_context(|| format!("Failed to canonicalize {}", dir.display()))?;
            return Ok(canonical_dir.join(name));
        }
    }
    anyhow::bail!("Executable '{}' not found on PATH", name);
}

/// Abstraction over spawning a subprocess, so pipeline phase logic can be
/// unit-tested with `MockRunner` instead of spawning real processes.
pub trait ProcessRunner {
    fn run(&self, program: &str, args: &[&str], cwd: Option<&Path>) -> Result<Output>;
}

/// Default `ProcessRunner` — resolves `program` via `resolve_exe` and spawns it.
pub struct SystemRunner;

impl ProcessRunner for SystemRunner {
    fn run(&self, program: &str, args: &[&str], cwd: Option<&Path>) -> Result<Output> {
        let resolved = resolve_exe(program)?;
        let mut cmd = Command::new(&resolved);
        cmd.args(args);
        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }
        cmd.output()
            .with_context(|| format!("Failed to run '{}'", program))
    }
}

/// A single recorded invocation, captured by `MockRunner` for assertions.
#[derive(Debug, Clone)]
pub struct RecordedCall {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
}

/// Build a canned `std::process::Output` for use with `MockRunner`.
/// Build a fake `Output` with the given success flag and captured streams.
/// Cross-platform: uses the OS-specific `ExitStatusExt::from_raw` so the
/// same helper compiles and behaves identically on Unix and Windows.
pub fn mock_output(success: bool, stdout: &str, stderr: &str) -> Output {
    let code = if success { 0 } else { 1 };
    #[cfg(unix)]
    let status = std::os::unix::process::ExitStatusExt::from_raw(code);
    #[cfg(windows)]
    let status = std::os::windows::process::ExitStatusExt::from_raw(code);
    #[cfg(not(any(unix, windows)))]
    let status = std::process::Command::new(if success { "true" } else { "false" })
        .status()
        .unwrap();
    Output {
        status,
        stdout: stdout.as_bytes().to_vec(),
        stderr: stderr.as_bytes().to_vec(),
    }
}

/// Test double for `ProcessRunner`: records every call and replays queued
/// responses in FIFO order, so phase logic can be exercised without
/// spawning real processes.
#[derive(Default)]
pub struct MockRunner {
    pub calls: RefCell<Vec<RecordedCall>>,
    responses: RefCell<VecDeque<std::result::Result<Output, String>>>,
}

impl MockRunner {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_response(&self, output: Output) {
        self.responses.borrow_mut().push_back(Ok(output));
    }

    pub fn push_error(&self, msg: &str) {
        self.responses.borrow_mut().push_back(Err(msg.to_string()));
    }
}

impl ProcessRunner for MockRunner {
    fn run(&self, program: &str, args: &[&str], cwd: Option<&Path>) -> Result<Output> {
        self.calls.borrow_mut().push(RecordedCall {
            program: program.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
            cwd: cwd.map(|p| p.to_path_buf()),
        });
        match self.responses.borrow_mut().pop_front() {
            Some(Ok(out)) => Ok(out),
            Some(Err(msg)) => anyhow::bail!(msg),
            None => anyhow::bail!("MockRunner: no response queued for '{}'", program),
        }
    }
}
