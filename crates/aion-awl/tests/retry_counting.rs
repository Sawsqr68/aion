//! `retry N` declares N FURTHER attempts after the first — N+1 total
//! executions.
//!
//! The wording is ratified in
//! `docs/design/aion-authoring/awl/AWL-COMPLETION-COLD-READ.md` §2.3 and is
//! already the documented meaning of `RetryContract::{Every,Backoff}::count`
//! in `aion-package`. The engine's dispatch loop stops at
//! `attempt < max_attempts`, so the ONE place the convention lives is where a
//! declared count is translated into the SDK's total-attempt budget. Both
//! lowerings own such a site:
//!
//! - reference: `emitter::stmts::retry_policy` → `activity.RetryPolicy(...)`;
//! - direct: `mir::lower::activity::build_retry_policy` →
//!   `record(retry_policy, [total, backoff])`.
//!
//! These pins hold both sites to `max_attempts = count + 1`, and hold the
//! declaration's `.v4` byte identity FIXED across the translation move: the
//! number the author wrote is what identity commits to, and it does not
//! change.

use std::path::Path;

use aion_awl::mir::{lower, print_mir};
use aion_awl::{compile, emit_artifact, parse};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// `retry 3 every 1s` on the action declaration: three FURTHER attempts, so
/// four total executions.
const EVERY_THREE: &str = r"//! Retry-counting pin: constant-interval dialect.
workflow retry_counting
  input name: String
  outcome greeted: type Greeting, route success

type Greeting { text: String }

worker flaky
  action flaky_call(name: String) -> Greeting
    retry 3 every 1s

step call
  flaky_call(name: name) -> result
  route greeted(text: result.text)
";

/// `retry 3 backoff 1s..2s`: the same count through the other dialect.
const BACKOFF_THREE: &str = r"//! Retry-counting pin: bounded-backoff dialect.
workflow retry_counting_backoff
  input name: String
  outcome greeted: type Greeting, route success

type Greeting { text: String }

worker flaky
  action flaky_call(name: String) -> Greeting
    retry 3 backoff 1s..2s

step call
  flaky_call(name: name) -> result
  route greeted(text: result.text)
";

/// `retry 0`: zero further attempts — one total execution, no retry.
const ZERO: &str = r"//! Retry-counting pin: the degenerate zero declaration.
workflow retry_counting_zero
  input name: String
  outcome greeted: type Greeting, route success

type Greeting { text: String }

worker flaky
  action flaky_call(name: String) -> Greeting
    retry 0 every 1s

step call
  flaky_call(name: name) -> result
  route greeted(text: result.text)
";

/// The emitted `activity.RetryPolicy(max_attempts: …)` fragment for `source`.
fn emitted_policy(source: &str) -> Result<String, Box<dyn std::error::Error>> {
    let document = parse(source)?;
    let artifact = emit_artifact(&document)?;
    let start = artifact
        .source
        .find("activity.RetryPolicy(")
        .ok_or("the emitted source declares no RetryPolicy")?;
    let tail = &artifact.source[start..];
    let end = tail
        .find(", backoff:")
        .ok_or("the emitted RetryPolicy has no argument separator")?;
    Ok(tail[..end].to_owned())
}

/// The `record(retry_policy, [total, …])` line the direct lowering builds.
fn lowered_policy(source: &str) -> Result<String, Box<dyn std::error::Error>> {
    let document = parse(source)?;
    let module = lower(&document, None)?;
    let printed = print_mir(&module);
    printed
        .lines()
        .find(|line| line.contains("record(retry_policy,"))
        .map(str::trim)
        .map(str::to_owned)
        .ok_or_else(|| "the lowered module builds no retry_policy record".into())
}

/// Reference lowering: `retry 3` becomes a FOUR-attempt SDK budget, in both
/// retry dialects.
#[test]
fn reference_emitter_maps_three_further_attempts_to_four_total() -> TestResult {
    assert_eq!(
        emitted_policy(EVERY_THREE)?,
        "activity.RetryPolicy(max_attempts: 4",
        "`retry 3 every 1s` grants three FURTHER attempts — four total"
    );
    assert_eq!(
        emitted_policy(BACKOFF_THREE)?,
        "activity.RetryPolicy(max_attempts: 4",
        "`retry 3 backoff 1s..2s` grants three FURTHER attempts — four total"
    );
    Ok(())
}

/// Direct lowering: the same declaration, the same total, through the MIR
/// backend's own mapping site.
#[test]
fn direct_lowering_maps_three_further_attempts_to_four_total() -> TestResult {
    let every = lowered_policy(EVERY_THREE)?;
    assert!(
        every.contains("record(retry_policy, [4,"),
        "`retry 3 every 1s` must lower to a four-attempt budget: {every}"
    );
    let backoff = lowered_policy(BACKOFF_THREE)?;
    assert!(
        backoff.contains("record(retry_policy, [4,"),
        "`retry 3 backoff 1s..2s` must lower to a four-attempt budget: {backoff}"
    );
    Ok(())
}

/// The degenerate declaration the grammar admits: `retry 0` is zero further
/// attempts, i.e. exactly one execution and no retry. Both lowerings say so
/// explicitly rather than leaning on a budget the engine decodes as absent.
#[test]
fn zero_further_attempts_is_one_total_execution() -> TestResult {
    assert_eq!(
        emitted_policy(ZERO)?,
        "activity.RetryPolicy(max_attempts: 1",
        "`retry 0` runs the action exactly once"
    );
    let lowered = lowered_policy(ZERO)?;
    assert!(
        lowered.contains("record(retry_policy, [1,"),
        "`retry 0` must lower to a single-attempt budget: {lowered}"
    );
    Ok(())
}

/// Contract identity is the number the AUTHOR wrote.
///
/// `RetryContract::count` carries the declared count verbatim into the `.v4`
/// `canonical_bytes`; only the runtime translation of that count moved. The
/// committed golden beside this file is the hex of [`EVERY_THREE`]'s compiled
/// `PackageContract::canonical_bytes`, captured BEFORE the mapping change and
/// byte-unchanged by it — a `retry`-declaring package that redeploys with no
/// source edit keeps its contract identity.
#[test]
fn declared_count_is_the_identity_and_does_not_move() -> TestResult {
    let golden_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/contract-goldens/retry_three_every_1s.hex");
    let golden = std::fs::read_to_string(&golden_path)?;
    let compiled = compile(EVERY_THREE, Path::new("."))?;
    assert_eq!(
        hex(&compiled.contract.canonical_bytes()),
        golden.trim(),
        "the `.v4` canonical bytes of a `retry 3` declaration must not move \
         (golden: {})",
        golden_path.display()
    );
    Ok(())
}

/// Lowercase hex rendering of `bytes`.
fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut out, byte| {
        let _ = write!(out, "{byte:02x}");
        out
    })
}
