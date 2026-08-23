//! The kill half of R6's kill-and-resume harness.
//!
//! # ⛔ KILL SCOPE — how this file stays inside it
//!
//! The brief's kill scope is binding in its strictest reading, and the
//! venue is a shared box carrying other lanes' workers and a never-kill
//! server. This harness therefore:
//!
//! * kills **only processes it started itself**, with
//!   [`std::process::Command::spawn`], in this test;
//! * takes the pid **from the pid file that process wrote for itself**, read
//!   **AT FIRE TIME**, immediately before the kill — never a pid recited
//!   from earlier, never a pid remembered across a step;
//! * issues a signal at one exact numeric pid and nothing else. **There is
//!   no pattern-matched process kill in any spelling anywhere in this
//!   crate** — a fact the `no_pattern_kill_anywhere` test asserts over the
//!   crate's own source bytes;
//! * cross-checks that the pid it is about to kill is one it spawned, and
//!   REFUSES to kill otherwise.
//!
//! A kill this harness cannot trace to its own receipt is a kill it does
//! not perform: the boundary goes untested and the gap is REPORTED.

use std::path::PathBuf;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use super::chain::Workspace;

/// A process this harness started, and the receipt it wrote.
pub struct OwnStep {
    child: Child,
    pid_file: PathBuf,
    /// The pid the OS reported at spawn. Used ONLY to cross-check the pid
    /// read from the receipt — never as the kill target on its own.
    spawned_pid: u32,
}

impl OwnStep {
    /// Read this process's pid from the receipt IT wrote, at fire time.
    ///
    /// Returns `None` until the receipt exists and parses — a receipt that
    /// is not there yet is a kill this harness does not perform.
    pub fn pid_from_own_receipt(&self) -> Option<u32> {
        let text = std::fs::read_to_string(&self.pid_file).ok()?;
        text.trim().parse::<u32>().ok()
    }

    /// Poll until the receipt appears, up to a total deadline.
    pub fn await_receipt(&self, deadline: Duration) -> Option<u32> {
        let started = Instant::now();
        loop {
            if let Some(pid) = self.pid_from_own_receipt() {
                return Some(pid);
            }
            if started.elapsed() >= deadline {
                return None;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// SIGKILL this process by the exact pid it recorded for itself.
    ///
    /// Refuses — and reports — when the receipt is absent or disagrees with
    /// the pid this harness spawned.
    pub fn kill_by_own_receipt(&mut self) -> Result<u32, String> {
        let Some(pid) = self.await_receipt(Duration::from_secs(10)) else {
            return Err("no pid receipt from the process this harness started; \
                        the kill is not performed and the boundary is REPORTED"
                .to_owned());
        };
        if pid != self.spawned_pid {
            return Err(format!(
                "receipt pid {pid} disagrees with the pid this harness spawned \
                 ({}); the kill is not performed",
                self.spawned_pid
            ));
        }
        // Exact numeric pid. No pattern, in any spelling.
        let out = Command::new("kill")
            .arg("-9")
            .arg(pid.to_string())
            .output()
            .map_err(|e| format!("cannot run kill: {e}"))?;
        if !out.status.success() {
            // The process may already have exited — that is a legitimate
            // race outcome and the caller asserts on the ledger either way.
            let _ = self.child.wait();
            return Ok(pid);
        }
        let _ = self.child.wait();
        Ok(pid)
    }
}

impl Workspace {
    /// A fresh workspace seeded with an EXACT copy of another ledger's
    /// bytes, so two runs can be compared from byte-identical state.
    pub fn from_ledger_bytes(graph_json: &str, ledger_bytes: &[u8]) -> Self {
        let ws = Self::new(graph_json);
        if !ledger_bytes.is_empty() {
            std::fs::write(ws.ledger_path(), ledger_bytes)
                .unwrap_or_else(|e| unreachable!("cannot seed ledger: {e}"));
        }
        ws
    }

    /// Empty the barrier, so the next step parks instead of proceeding.
    pub fn clear_inbox(&self) {
        let inbox = self.root.path().join("inbox");
        let Ok(entries) = std::fs::read_dir(&inbox) else {
            return;
        };
        for entry in entries.flatten() {
            let _ = std::fs::remove_file(entry.path());
        }
    }

    /// Start one step WITHOUT waiting for it, so it can be killed.
    pub fn spawn_step(&self, root_base: &str, deadline_secs: u64) -> OwnStep {
        let pid_file = self.root.path().join("step.pid");
        let _ = std::fs::remove_file(&pid_file);
        let mut cmd = self.command(root_base, deadline_secs);
        cmd.arg("--pid-file").arg(&pid_file);
        cmd.stdout(std::process::Stdio::null());
        cmd.stderr(std::process::Stdio::null());
        let child = cmd
            .spawn()
            .unwrap_or_else(|e| unreachable!("cannot spawn the step binary: {e}"));
        let spawned_pid = child.id();
        OwnStep {
            child,
            pid_file,
            spawned_pid,
        }
    }

    /// The next action, decided without advancing anything.
    pub fn peek_next(&self, root_base: &str) -> serde_json::Value {
        let out = Command::new(env!("CARGO_BIN_EXE_aion-fleet-plan"))
            .arg("chain-next")
            .arg("--graph-file")
            .arg(self.root.path().join("graph.json"))
            .arg("--ledger-file")
            .arg(self.ledger_path())
            .arg("--root-base")
            .arg(root_base)
            .output()
            .unwrap_or_else(|e| unreachable!("cannot run chain-next: {e}"));
        let text = String::from_utf8_lossy(&out.stdout);
        let value: serde_json::Value = serde_json::from_str(text.trim())
            .unwrap_or_else(|e| unreachable!("chain-next stdout is not JSON ({e}): {text}"));
        value["next"].clone()
    }
}
