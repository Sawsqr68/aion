//! Runtime opt-in for the proofs too costly to run on every `cargo test` (#86).
//!
//! # Why this exists rather than `#[ignore]`
//!
//! The house rules ban `#[ignore]` outright, and the reason is visibility: an
//! ignored test contributes nothing to the log except the word `ignored` in a
//! tally nobody reads, so the most expensive proofs in the repository were
//! absent from every run and **the headline number could not say so**. The
//! cross-node kill-9 failover proof and the parked-timer resume proof are the
//! tests that establish the durability claim, and by default they did not run.
//!
//! # What changes, and what deliberately does NOT
//!
//! The default behaviour is unchanged on purpose: these still do not run unless
//! [`OPT_IN`] is set. Turning a 3-node cluster kill-9 test on by default would
//! add many minutes to every battery and trade one problem for a worse one.
//!
//! What changes is that the absence is now **stated, in the run's own output**,
//! naming the proof that did not happen. A skip you can read is a different
//! object from a skip you cannot.
//!
//! # 🔴 The honest cost of this shape
//!
//! A runtime-gated skip returns `Ok(())`, so it counts as a **pass**. The pass
//! total therefore includes tests that proved nothing — which is strictly worse
//! than `#[ignore]`'s tally, where they were at least counted separately.
//!
//! That trade is only acceptable because the announcement is *checked*, not
//! merely printed: `gates:costly-announced` in `scripts/battery.sh` asserts
//! every one of these lines appears in a default run. Printed-and-unchecked
//! would be observability; printed-and-asserted is a gate. Without that
//! battery check this module would be a downgrade, and if the check is ever
//! deleted, this becomes one.

use std::io::{self, Write};
use std::panic::Location;

/// Set this to any non-empty value to run the costly proofs.
///
/// One variable rather than one per cost class: an operator opting in wants the
/// expensive suite, and splitting it invents a taxonomy nobody asked for.
pub const OPT_IN: &str = "AION_RUN_COSTLY_TESTS";

/// The prefix every announcement carries, and the string the battery greps.
///
/// Deliberately unlovely: it is a machine interface, and a token a person might
/// write in prose is a token with a false positive waiting in it.
pub const SKIP_TOKEN: &str = "COSTLY_PROOF_SKIPPED";

/// True when the caller should announce and return `Ok(())`.
///
/// `proof` names what did not get established — not what the test does. A
/// reader of a skipped run needs to know which claim is now unproven, and
/// "spawns a 3-node cluster" does not tell them that.
#[must_use]
#[track_caller]
pub fn skip_unless_opted_in(proof: &str) -> bool {
    if std::env::var_os(OPT_IN).is_some_and(|value| !value.is_empty()) {
        return false;
    }

    let location = Location::caller();
    announce(&format!(
        "{SKIP_TOKEN} (PROVES NOTHING): {proof} — not established by this run. \
         Set {OPT_IN}=1 to run it. [{}:{}]",
        location.file(),
        location.line()
    ));
    true
}

/// Write to the real stderr handle, which the test harness does not capture.
///
/// `println!` goes through the harness's per-test capture and is swallowed on a
/// passing test — and a gated skip always passes, so a captured announcement
/// would be invisible in exactly the case it exists for. Falls back to the
/// captured stream rather than going silent if the real handle fails.
fn announce(message: &str) {
    let mut stderr = io::stderr();
    if writeln!(stderr, "{message}")
        .and_then(|()| stderr.flush())
        .is_err()
    {
        println!("{message}");
    }
}
