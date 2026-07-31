//! What this build actually covered, announced in the one channel a test
//! runner always prints: the test's NAME.
//!
//! # The trap this closes (#119)
//!
//! `liminal-transport` is OFF by default, deliberately — a default worker build
//! is byte-identical and never links `liminal-sdk`. The consequence is that
//! `cargo test -p aion-worker` compiles NONE of the liminal transport
//! (`runtime::liminal*`, the redial loop, the serve loop, the liveness probe)
//! and exits **0**.
//!
//! That green is not a weak result. It is a result about a different program.
//! And it is how a P0 reached `main` on 2026-07-31: plain dispatches were moved
//! onto a `tokio::spawn` that the real worker's current-thread runtime never
//! polls, so every liminal worker would have accepted dispatches and executed
//! none of them. The coverage existed, and the per-package gate a developer
//! naturally reaches for compiled none of it and said `0`.
//!
//! # 🔴 The second axis: turning the feature ON is still not coverage
//!
//! `cargo test -p aion-worker --all-features` compiles the transport and
//! exercises none of it. This crate has exactly ONE integration target
//! (`tests/grpc_registration.rs`), and it does not drive the liminal dispatch
//! path. The behaviour is proven in **`aion-server`**, whose dev-dependency
//! declares `aion-worker = { features = ["liminal-transport"] }` and 13 of whose
//! test files exercise the transport end to end.
//!
//! So there are two independent ways for a per-package run to be vacuous:
//!
//! ```text
//! cargo test -p aion-worker                 -> transport NOT COMPILED
//! cargo test -p aion-worker --all-features  -> COMPILED, NOT EXERCISED
//! cargo test -p aion-server                 -> the run that actually proves it
//! ```
//!
//! The second axis needs no gate of its own: six of those `aion-server` targets
//! name `aion_worker::LiminalActivityWorker` directly, so dropping the dev-dep's
//! `features = ["liminal-transport"]` breaks their BUILD rather than silently
//! retiring the coverage. That is the same anchor the positive arm below uses,
//! one level up, and it is already load-bearing.
//!
//! The announcements below can only speak to the first axis, because a test can
//! observe its own build and not the operator's package selection. That is why
//! the positive arm's NAME stops at "compiled" and names `aion-server` rather
//! than claiming its surface was tested — a name asserting coverage this run
//! does not have would be the exact defect this module exists to prevent. It was
//! written that way on the second pass: the first draft made that very claim,
//! and it was caught by checking the module's own doc comment against the
//! crate's actual test targets.
//!
//! # Why an announcement and not a failure
//!
//! Failing the default build would break `cargo test -p aion-worker` for
//! everyone and destroy the byte-identical default the feature exists to
//! protect. Skipping is worse still: `#[ignore]` is banned outright by the
//! house rules precisely because it makes absence invisible.
//!
//! So the surface is neither failed nor skipped — it is **named**. A test's
//! name is printed on every run, passing or not, without `--nocapture` and
//! without anyone opting in. The name states what this build did not cover, so
//! a vacuous green announces its own vacuity in the operator's own terminal.
//!
//! The two arms are mutually exclusive by construction, so exactly one is
//! compiled and exactly one is printed: a run can never be silent about which
//! program it tested.
//!
//! # Printed is not checked — the control lives in the battery
//!
//! An announcement nothing compares is observability, not a gate: a run where
//! the WRONG arm fires still reconciles perfectly, because every count adds up.
//! So `scripts/battery.sh` carries `posture:*` gates that assert the
//! posture-appropriate name appeared AND its opposite did not. That external
//! assertion is also what anchors the negative arm below — on its own it claims
//! very little, and deleting it would otherwise go unnoticed.
//!
//! # The limit, stated rather than papered over
//!
//! A FILTERED run (`cargo test -p aion-worker some_other_test`) skips the
//! announcer entirely — and a narrow command is exactly the case this exists to
//! protect, because the developer most likely to be misled is the one most
//! likely to filter. No in-suite announcement can reach that, and the battery
//! gates cannot either: they filter *to* `build_coverage::` precisely so the
//! announcer is the only thing they run. They prove the arms are wired to the
//! posture; they cannot make an unrelated ad-hoc filter announce anything.

#[cfg(not(feature = "liminal-transport"))]
#[test]
fn liminal_transport_absent_from_this_build_so_none_of_it_was_tested() {
    // Deliberately passes. This is an ANNOUNCEMENT, not a defect: the operator
    // asked for a default build and got one. The finding is in the name, which
    // the runner prints whether or not anyone is reading carefully.
    //
    // The arm's self-anchor: if someone ever deletes the `cfg` above, this stops
    // the BUILD rather than letting the name quietly claim absence in a build
    // that has the transport.
    //
    // It is a `const` block deliberately. `cfg!` is a compile-time constant, so
    // a plain runtime `assert!` here would be constant-folded — a vacuous
    // assertion inside the one module whose whole purpose is to stop vacuous
    // greens. Clippy's `assertions_on_constants` says exactly that. Evaluating
    // it in const context turns the same claim into a compile error, which is
    // both honest about when it is decided and louder than any runtime check.
    //
    // It stays weak by design: it can only catch the `cfg` being deleted. The
    // real anchor is the `posture:worker-default` gate in scripts/battery.sh.
    const {
        assert!(
            !cfg!(feature = "liminal-transport"),
            "this arm claims the liminal transport is absent, but the feature is ON — \
             the announcement would be lying about the build it ran in"
        );
    }
}

/// The counterpart, and it must not be able to pass vacuously: it names a type
/// that only exists behind the feature, so it cannot compile — let alone pass —
/// in a build where the transport is absent.
///
/// Its name stops at COMPILED deliberately. See the second axis above: this
/// crate cannot honestly claim the transport was exercised, because it has no
/// target that exercises it.
#[cfg(feature = "liminal-transport")]
#[test]
fn liminal_transport_compiled_here_but_its_behaviour_is_only_proven_by_aion_server() {
    // Referencing the type IS the assertion. If the transport ever stopped
    // being exported under its own feature, this stops compiling — which is a
    // louder failure than any runtime check could be.
    let _: Option<crate::LiminalActivityWorker> = None;
}
