//! The embedded assistant document is launchable by the verb the server
//! recommends — proved through the real binary (#209).
//!
//! # The defect this pins
//!
//! 0.14.0 shipped a QUEUE MOVE warn telling the operator to run
//! `aion worker agent assistant.awl …` — and the embedded document carried no
//! `harness` section, so that exact invocation refused with
//! "declares no `harness` section". The server recommended a verb its own
//! document could not satisfy. The fix put the launch declaration INTO the
//! embedded document; this test holds the two surfaces together from now on.
//!
//! # The arms
//!
//! * The embedded document — the very bytes `include_str!` compiles into the
//!   server — reports `norn` on queue `assistant` under `--check`, advertising
//!   the `assistant` action.
//! * The SAME document with its `harness` section stripped reports
//!   `harness: null` under `--check` and is REFUSED by the serve verb, naming
//!   the missing section — 0.14.0 reconstructed.
//! * THE PIN: the recommended serve invocation on the shipped document clears
//!   the whole serve path and fails only at the connection. `--check` alone
//!   cannot carry the pin (it is a report that returns before the refusals),
//!   so this arm is the one that would have caught 0.14.0.
//! * The spike (`examples/assistant/awl/assistant_spike.awl`) carries the
//!   embedded document's prompt constants VERBATIM, except the one named
//!   divergence — its header comment claims this, and the claim drifted once
//!   already with nothing measuring it.

use std::path::{Path, PathBuf};
use std::process::Output;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Removes a fixture file when the test ends, PASS OR FAIL — an assertion
/// firing mid-test must not leak the fixture into the temp directory.
struct RemoveOnDrop(PathBuf);

impl Drop for RemoveOnDrop {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// The embedded document's one home: the file `aion-server` compiles in via
/// `include_str!`. Read from the tree so this test exercises the same bytes a
/// release would embed, with no copy to drift.
fn embedded_document_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../aion-server/assistant-embed/assistant.awl")
}

/// A test-owned aion home, thrown away with the test.
///
/// The shipped document's `harness` section sets `cwd "{workspace_root}"`, and
/// resolving that placeholder asks the launching box for its aion home
/// (`aion_server::config::aion_home` → `<home>/clones`, created at expansion).
/// Reading that home from an ambient `HOME` makes the verdict a property of
/// the BOX rather than of the document: green on a desk, red under any runner
/// whose child environment carries no `HOME` — and green only by creating
/// `clones/` inside the operator's own `~/.aion` as a side effect. The home
/// this test means is therefore stated, exactly as every other launching e2e
/// in this crate states its own.
fn test_home() -> Result<tempfile::TempDir, Box<dyn std::error::Error>> {
    Ok(tempfile::Builder::new()
        .prefix("aion-209-home-")
        .tempdir()?)
}

/// Runs `aion worker agent <document> --check` through the real binary.
///
/// `--check` connects to nothing: the surface is derived and the harness
/// resolved exactly as a serve would, then the verb stops before dialling —
/// which is what makes this safe to run anywhere.
fn run_check(document: &Path, home: &Path) -> Result<Output, Box<dyn std::error::Error>> {
    Ok(std::process::Command::new(env!("CARGO_BIN_EXE_aion"))
        .args(["worker", "agent"])
        .arg(document)
        .arg("--check")
        .env("AION_HOME", home)
        .output()?)
}

#[test]
fn the_embedded_document_is_launchable_by_the_recommended_verb() -> TestResult {
    let home = test_home()?;
    let output = run_check(&embedded_document_path(), home.path())?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "the server's QUEUE MOVE warn tells the operator to run `aion worker agent` on this \
         document; the document must satisfy that verb as shipped (#209). stderr was:\n{stderr}"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let report: serde_json::Value = serde_json::from_str(&stdout)?;
    assert_eq!(
        report["harness"],
        serde_json::json!("norn"),
        "the embedded document must declare a harness this build resolves; report was:\n{report}"
    );
    assert_eq!(
        report["task_queue"],
        serde_json::json!("assistant"),
        "the worker must come up on the assistant's private queue (#200); report was:\n{report}"
    );
    let advertises_assistant = report["actions"]
        .as_array()
        .is_some_and(|actions| actions.iter().any(|action| action["name"] == "assistant"));
    assert!(
        advertises_assistant,
        "the worker must advertise the `assistant` agent action, or admission on the private \
         queue refuses it; report was:\n{report}"
    );
    Ok(())
}

/// 0.14.0, RECONSTRUCTED. The same document with the `harness` section removed
/// reports `harness: null` under `--check` — so the `"norn"` above is the
/// section's own contribution, not a build default — and the SERVE invocation
/// the warn recommends is refused outright, naming the missing section.
///
/// The serve arm dials nothing: `prepare` resolves the harness before any
/// connection exists, and the refusal it returns is exactly what a 0.14.0
/// operator saw when they followed the server's own instructions.
#[test]
fn the_same_document_without_its_harness_section_is_refused() -> TestResult {
    let document = std::fs::read_to_string(embedded_document_path())?;
    let mut stripped = Vec::new();
    let mut lines = document.lines().peekable();
    while let Some(line) = lines.next() {
        if line == "  harness" {
            // Drop the section body: every following line at setting depth.
            while lines.peek().is_some_and(|next| next.starts_with("    ")) {
                lines.next();
            }
            continue;
        }
        stripped.push(line);
    }
    let stripped = format!("{}\n", stripped.join("\n"));
    assert_ne!(
        stripped, document,
        "the control must actually remove a harness section — if there was none to remove, \
         the passing arm above proved nothing"
    );

    let path = std::env::temp_dir().join(format!(
        "aion-209-assistant-without-harness-{}.awl",
        std::process::id()
    ));
    std::fs::write(&path, &stripped)?;
    let _fixture = RemoveOnDrop(path.clone());

    // `--check` is a report, not a gate: it succeeds and says `harness: null`.
    // This is what discriminates the passing arm's `"norn"` from a default.
    let home = test_home()?;
    let check = run_check(&path, home.path())?;
    assert!(
        check.status.success(),
        "`--check` reports rather than refuses; stderr was:\n{}",
        String::from_utf8_lossy(&check.stderr)
    );
    let report: serde_json::Value = serde_json::from_str(&String::from_utf8_lossy(&check.stdout))?;
    assert!(
        report["harness"].is_null(),
        "with the section stripped, no harness may resolve — a non-null value here means \
         the passing arm's `norn` came from somewhere other than the document; report \
         was:\n{report}"
    );

    // The invocation the QUEUE MOVE warn recommends, minus nothing. `prepare`
    // refuses before any connection is attempted, so the address is never dialled.
    let serve = std::process::Command::new(env!("CARGO_BIN_EXE_aion"))
        .args(["worker", "agent"])
        .arg(&path)
        .args([
            "--liminal-address",
            "127.0.0.1:1",
            "--identity",
            "refusal-control",
        ])
        .env("AION_HOME", home.path())
        .output()?;

    let stderr = String::from_utf8_lossy(&serve.stderr);
    assert!(
        !serve.status.success(),
        "a document with no `harness` section must be refused by the launch verb — this arm \
         is the 0.14.0 defect, and it must stay red. stdout was:\n{}",
        String::from_utf8_lossy(&serve.stdout)
    );
    assert!(
        stderr.contains("harness"),
        "the refusal must name the missing section so an operator can act on it; stderr \
         was:\n{stderr}"
    );
    Ok(())
}

/// THE PIN (#209). The exact invocation the QUEUE MOVE warn recommends, on the
/// embedded document as shipped, clears the WHOLE serve path — surface
/// derivation, harness resolution, compose, capability admission, worker
/// config — and fails only at the connection, because the address is dead.
///
/// `--check` cannot pin this: it is a report that returns before the
/// no-harness refusal and before `compose`, so a document the serve path
/// refuses still checks green (the arm above proves exactly that). This arm
/// runs the serve verb itself against `127.0.0.1:1` and reads the launch
/// outcome from the worker's own JSON log on STDOUT — the product subscriber
/// (`aion_server::observability::tracing::init`) writes events there, while
/// stderr carries only the pre-subscriber refusal reports. The worker's
/// reconnect loop retries a dead address within its attempt budget, so the
/// test waits for the dial-failure EVENT and then ends the child — a
/// serve-path refusal exits on its own, well before the deadline, and is the
/// red this test exists to raise.
#[test]
fn the_recommended_serve_invocation_reaches_the_dial_not_a_refusal() -> TestResult {
    use std::io::BufRead;
    use std::process::Stdio;
    use std::sync::{Arc, Mutex};

    let home = test_home()?;
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_aion"))
        .args(["worker", "agent"])
        .arg(embedded_document_path())
        .args([
            "--liminal-address",
            "127.0.0.1:1",
            "--identity",
            "assistant-worker",
        ])
        .env("AION_HOME", home.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    // Capture stdout as it streams: the child never exits on its own on the
    // happy path, so the log must be read while it runs, not after.
    let stdout_pipe = child
        .stdout
        .take()
        .ok_or("the child's stdout was not piped")?;
    let captured: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
    let sink = Arc::clone(&captured);
    let reader = std::thread::spawn(move || {
        for line in std::io::BufReader::new(stdout_pipe).lines() {
            let Ok(line) = line else { break };
            let mut buffer = match sink.lock() {
                Ok(buffer) => buffer,
                Err(poisoned) => poisoned.into_inner(),
            };
            buffer.push_str(&line);
            buffer.push('\n');
        }
    });
    let captured_text = || match captured.lock() {
        Ok(buffer) => buffer.clone(),
        Err(poisoned) => poisoned.into_inner().clone(),
    };

    // Wait for the dial-failure EVENT rather than a fixed interval: a refusal
    // exits within milliseconds, the first failed dial is logged in under a
    // second, and the deadline only bites when neither happens on a loaded box.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let mut exited_early = None;
    let mut dial_failed = false;
    loop {
        if let Some(status) = child.try_wait()? {
            exited_early = Some(status);
            break;
        }
        if captured_text().contains("dial failed") {
            dial_failed = true;
            break;
        }
        if std::time::Instant::now() >= deadline {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    if exited_early.is_none() {
        child.kill()?;
    }
    let output = child.wait_with_output()?;
    reader
        .join()
        .map_err(|_| "the stdout reader thread panicked")?;
    let log = captured_text();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        exited_early.is_none(),
        "the recommended invocation must survive to the dial — an early exit is a \
         serve-path refusal, which is the 0.14.0 defect; stderr was:\n{stderr}\nlog was:\n{log}"
    );
    assert!(
        dial_failed,
        "the only failure on a dead address is the connection itself, and it must be \
         logged before the deadline; log was:\n{log}"
    );
    assert!(
        log.contains("agent worker dialing liminal candidates"),
        "the launch must reach the dial; log was:\n{log}"
    );
    assert!(
        log.contains("\"task_queue\":\"assistant\"") && log.contains("\"harness\":\"norn\""),
        "the dial must be for the document's own queue under its own harness; log \
         was:\n{log}"
    );
    assert!(
        !log.contains("declares no `harness` section")
            && !log.contains("would advertise")
            && !stderr.contains("declares no `harness` section")
            && !stderr.contains("would advertise"),
        "neither the no-harness refusal nor a compose refusal may appear on the shipped \
         document, on either stream; stderr was:\n{stderr}\nlog was:\n{log}"
    );
    Ok(())
}

/// Extracts every `const NAME = """…"""` from an AWL document as name → value.
///
/// Handles both shapes the assistant documents use: a constant opened and
/// closed on one line, and a multi-line body whose closing `"""` ends either a
/// content line or a line of its own. Both documents are parsed by this same
/// function, so any systematic parsing quirk cancels out of the comparison.
fn document_constants(source: &str) -> std::collections::BTreeMap<String, String> {
    let mut constants = std::collections::BTreeMap::new();
    let mut lines = source.lines();
    while let Some(line) = lines.next() {
        let trimmed = line.trim_start();
        let Some(rest) = trimmed.strip_prefix("const ") else {
            continue;
        };
        let Some((name, opening)) = rest.split_once("= \"\"\"") else {
            continue;
        };
        let name = name.trim().to_owned();
        if let Some(inline) = opening.strip_suffix("\"\"\"") {
            constants.insert(name, inline.to_owned());
            continue;
        }
        let mut body = String::from(opening);
        for content in lines.by_ref() {
            if let Some(last) = content.strip_suffix("\"\"\"") {
                body.push('\n');
                body.push_str(last);
                break;
            }
            body.push('\n');
            body.push_str(content);
        }
        constants.insert(name, body);
    }
    constants
}

/// One prompt, two documents. The spike's header comment claims its const
/// block is IDENTICAL to the embedded document's — and inside #209's own lane
/// that claim drifted (three constants diverged) with nothing measuring it.
/// The two documents share queue `assistant`, so one worker can serve both
/// and open sessions under contradictory contracts; this pins the identity.
///
/// `contract_tail` is the ONE deliberate divergence (the embedded copy alone
/// teaches the aion MCP authoring tools, #184) and it must actually differ —
/// if it ever becomes equal, the exemption is dead and gets removed.
#[test]
fn the_spike_carries_the_embedded_prompt_constants_verbatim() -> TestResult {
    let embedded = document_constants(&std::fs::read_to_string(embedded_document_path())?);
    let spike_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/assistant/awl/assistant_spike.awl");
    let spike = document_constants(&std::fs::read_to_string(spike_path)?);

    let shared: Vec<&String> = embedded
        .keys()
        .filter(|name| spike.contains_key(name.as_str()))
        .collect();
    for required in [
        "contract_head",
        "workspace_confinement",
        "scratch_ground_truth",
        "contract_tail",
    ] {
        assert!(
            shared.iter().any(|name| name.as_str() == required),
            "const `{required}` must be shared by both documents — its absence means the \
             parser stopped matching or a document lost its prompt, and every comparison \
             below would be vacuous; shared set was: {shared:?}"
        );
    }

    for name in &shared {
        if name.as_str() == "contract_tail" {
            continue;
        }
        let embedded_value = embedded
            .get(name.as_str())
            .ok_or("embedded constant vanished between collection and comparison")?;
        let spike_value = spike
            .get(name.as_str())
            .ok_or("spike constant vanished between collection and comparison")?;
        assert_eq!(
            embedded_value, spike_value,
            "shared const `{name}` diverged between the embedded document and the spike — \
             one worker can serve both documents on queue `assistant`, so a divergence \
             opens sessions under contradictory contracts"
        );
    }

    let embedded_tail = embedded
        .get("contract_tail")
        .ok_or("the embedded document lost contract_tail")?;
    let spike_tail = spike
        .get("contract_tail")
        .ok_or("the spike lost contract_tail")?;
    assert_ne!(
        embedded_tail, spike_tail,
        "contract_tail is the one deliberate divergence (#184: the embedded copy alone \
         teaches the MCP tools) — equal tails mean the exemption above is dead and this \
         test should compare them too"
    );
    Ok(())
}
