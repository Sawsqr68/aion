//! What this build actually covered, announced in the one channel a test
//! runner always prints: the test's NAME.
//!
//! # The trap this closes (#119, second member of the class)
//!
//! `generated` is OFF by default, so `cargo test -p aion-proto` compiles none
//! of the tonic/prost stubs in `aion-proto-generated` and exits **0**. The
//! hand-written contracts in this crate exist to be checked AGAINST those
//! stubs; with the feature off, the crate's tests validate the hand-written
//! side against nothing.
//!
//! This crate has already paid for that once. Both worker SDKs shipped wire-dead
//! against a green suite because the tests validated against a FOSSIL of the
//! generated stubs rather than the stubs themselves — a suite that agrees with
//! a stale artifact reports agreement, not correctness.
//!
//! # Why an announcement and not a failure
//!
//! Failing the default build would break the default build for everyone;
//! `#[ignore]` is banned outright by the house rules for making absence
//! invisible. So the gap is neither failed nor skipped — it is **named**, in a
//! test name the runner prints on every run without anyone opting in.
//!
//! The two arms are mutually exclusive by construction, so exactly one is
//! compiled and exactly one is printed.
//!
//! # 🔴 The posture depends on WHICH PACKAGES YOU NAME, not only on the flags
//!
//! `aion-worker` depends on this crate with `features = ["generated"]`, so
//! cargo's feature unification turns `generated` ON whenever both are selected:
//!
//! ```text
//! cargo test -p aion-proto                  -> generated ABSENT   (vacuous)
//! cargo test -p aion-worker -p aion-proto   -> generated PRESENT
//! ```
//!
//! Same flags, same crate, opposite coverage — decided by the other package on
//! the command line. This was found by the announcement on its first run, not
//! by reasoning, and it is why `scripts/battery.sh` pins the ALONE invocation
//! specifically: a run that names a neighbour is testing a different program
//! from the one the operator asked about.
//!
//! # Printed is not checked
//!
//! An announcement nothing compares is observability, not a gate. The
//! `posture:proto-*` gates in `scripts/battery.sh` assert that the
//! posture-appropriate name appeared and its opposite did not; they are also
//! the real anchor for the negative arm, which claims little on its own. A
//! FILTERED run skips the announcer entirely — a limit no in-suite mechanism
//! can close, and the battery gates never filter.

#[cfg(not(feature = "generated"))]
#[test]
fn generated_stubs_absent_from_this_build_so_no_contract_was_checked_against_them() {
    // Deliberately passes. The announcement IS the name: a green here means the
    // hand-written contracts were compiled, not that they agree with the wire.
    //
    // Weak self-anchor, for the case where someone deletes the `cfg` above; the
    // real anchor is `posture:proto-default` in scripts/battery.sh.
    assert!(
        !cfg!(feature = "generated"),
        "this arm claims the generated stubs are absent, but the feature is ON — \
         the announcement would be lying about the build it ran in"
    );
}

/// The counterpart, and it must not be able to pass vacuously: it names the
/// generated crate directly, so it cannot compile in a build that lacks it.
#[cfg(feature = "generated")]
#[test]
fn generated_stubs_compiled_into_this_build_so_contracts_can_be_checked() {
    // Naming the dependency IS the assertion. If the feature ever stopped
    // pulling the generated crate, this stops compiling — louder than any
    // runtime check.
    let _ = std::any::type_name::<crate::generated::ActivityDescriptor>();
}
