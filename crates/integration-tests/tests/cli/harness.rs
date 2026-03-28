use std::path::PathBuf;
use std::process::{Command, Output};

pub struct Ctx {
    db_path: PathBuf,
    bin: PathBuf,
    _dir: tempfile::TempDir,
}

pub struct CmdResult {
    pub stdout: String,
    pub stderr: String,
    pub status: i32,
}

impl Ctx {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let db_path = dir.path().join("test.db");
        let bin = Self::find_binary();
        Self { db_path, bin, _dir: dir }
    }

    fn find_binary() -> PathBuf {
        let test_exe = std::env::current_exe().expect("no current exe");
        let deps_dir = test_exe.parent().expect("no parent");
        let target_dir = deps_dir.parent().expect("no target dir");
        let bin = target_dir.join("ctx");
        assert!(bin.exists(), "ctx binary not found at {bin:?}. Run `cargo build` first.");
        bin
    }

    pub fn run(&self, args: &[&str]) -> CmdResult {
        let output: Output = Command::new(&self.bin)
            .arg("--db")
            .arg(&self.db_path)
            .args(args)
            .output()
            .expect("failed to execute ctx");

        CmdResult {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            status: output.status.code().unwrap_or(-1),
        }
    }
}

impl CmdResult {
    // -- status assertions --------------------------------------------------

    pub fn success(&self) -> &Self {
        assert_eq!(self.status, 0, "expected success, got exit {}\nstderr: {}", self.status, self.stderr);
        self
    }

    pub fn failure(&self) -> &Self {
        assert_ne!(self.status, 0, "expected failure but succeeded\nstdout: {}", self.stdout);
        self
    }

    // -- string assertions --------------------------------------------------

    pub fn stdout_contains(&self, needle: &str) -> &Self {
        assert!(self.stdout.contains(needle), "stdout missing '{needle}'\n---\n{}", self.stdout);
        self
    }

    pub fn stdout_not_contains(&self, needle: &str) -> &Self {
        assert!(!self.stdout.contains(needle), "stdout should not contain '{needle}'\n---\n{}", self.stdout);
        self
    }

    pub fn stderr_contains(&self, needle: &str) -> &Self {
        assert!(self.stderr.contains(needle), "stderr missing '{needle}'\n---\n{}", self.stderr);
        self
    }

    pub fn stdout_eq(&self, expected: &str) -> &Self {
        assert_eq!(self.stdout.trim(), expected.trim(), "stdout mismatch");
        self
    }

    // -- line assertions ----------------------------------------------------

    pub fn stdout_line_count(&self, expected: usize) -> &Self {
        let count = self.lines().len();
        assert_eq!(count, expected, "expected {expected} lines, got {count}\n---\n{}", self.stdout);
        self
    }

    /// Assert that at least one line exactly matches (after trimming).
    pub fn stdout_has_line(&self, expected: &str) -> &Self {
        let expected = expected.trim();
        assert!(
            self.lines().iter().any(|l| l.trim() == expected),
            "no line matches '{expected}'\n---\n{}", self.stdout
        );
        self
    }

    // -- predicate assertions -----------------------------------------------

    /// Assert that at least one line satisfies the predicate.
    pub fn stdout_any_line<F: Fn(&str) -> bool>(&self, desc: &str, pred: F) -> &Self {
        assert!(
            self.lines().iter().any(|l| pred(l)),
            "no line matches predicate: {desc}\n---\n{}", self.stdout
        );
        self
    }

    /// Assert that every line satisfies the predicate.
    pub fn stdout_all_lines<F: Fn(&str) -> bool>(&self, desc: &str, pred: F) -> &Self {
        for (i, line) in self.lines().iter().enumerate() {
            assert!(pred(line), "line {i} failed predicate: {desc}\nline: {line}\n---\n{}", self.stdout);
        }
        self
    }

    /// Assert that the Nth line (0-indexed) satisfies the predicate.
    pub fn stdout_line_at<F: Fn(&str) -> bool>(&self, n: usize, desc: &str, pred: F) -> &Self {
        let lines = self.lines();
        assert!(n < lines.len(), "line {n} out of range (have {} lines)", lines.len());
        assert!(pred(lines[n]), "line {n} failed predicate: {desc}\nline: {}\n---\n{}", lines[n], self.stdout);
        self
    }

    /// Parse tabular output where each line is `label  value` and assert a specific key-value.
    pub fn stdout_has_kv(&self, key: &str, value: &str) -> &Self {
        let found = self.lines().iter().any(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            parts.len() >= 2 && parts[0] == key && parts[parts.len() - 1] == value
        });
        assert!(found, "no line has key='{key}' value='{value}'\n---\n{}", self.stdout);
        self
    }

    // -- extraction ---------------------------------------------------------

    /// Extract the ID from output like "Session:42".
    pub fn ref_id(&self) -> i64 {
        let line = self.stdout.trim();
        line.split(':')
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(|| panic!("could not parse ref ID from '{line}'"))
    }

    // -- helpers ------------------------------------------------------------

    fn lines(&self) -> Vec<&str> {
        self.stdout.lines().filter(|l| !l.is_empty()).collect()
    }
}
