//! Real Gleam compiler proofs for representative emitted modules.

use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::{emitted_archived_exam, emitted_fixture};

/// Flagship compile proof: `awl_hello` and `dev_brief` build under real
/// `gleam build` against the local SDK (#248: if this fails under parallel
/// load, re-run isolated once before treating it as red).
#[test]
fn flagship_fixtures_compile_under_gleam() -> Result<(), Box<dyn Error>> {
    if crate::gleam_test_support::skip_if_unavailable() {
        return Ok(());
    }
    compile_generated_modules(
        "flagship",
        &[
            (
                "awl_hello",
                emitted_fixture("flagship/valid/awl_hello.awl")?,
            ),
            (
                "dev_brief",
                emitted_fixture("flagship/valid/dev_brief.awl")?,
            ),
        ],
    )
}

/// Loop/fork-heavy compile proof: fixtures exercising loops, counters,
/// forks in all three forms, waits, and routes all build under real
/// `gleam build`. (`ship_release_combined` left this set at rev 3: its
/// build/canary cycle now carries a `max … visits` bound, which refuses to
/// emit until B4 lowers the flow shape.)
#[test]
fn loop_and_fork_fixtures_compile_under_gleam() -> Result<(), Box<dyn Error>> {
    if crate::gleam_test_support::skip_if_unavailable() {
        return Ok(());
    }
    compile_generated_modules(
        "loop_fork",
        &[
            (
                "guard_optional_wait",
                emitted_fixture("loop-outcomes/valid/guard_optional_wait.awl")?,
            ),
            (
                "loop_counting_until_max",
                emitted_fixture("loop-outcomes/valid/loop_counting_until_max.awl")?,
            ),
            (
                "release_pipeline",
                emitted_fixture("dag-fork/valid/release_pipeline_combined.awl")?,
            ),
            (
                "fork_named",
                emitted_fixture("dag-fork/valid/fork_named_branches.awl")?,
            ),
            (
                "child_collection_fork",
                emitted_fixture("dag-fork/valid/child_collection_fork.awl")?,
            ),
            (
                "fork_action_fanout",
                emitted_fixture("dag-fork/valid/fork_action_fanout.awl")?,
            ),
            (
                "fork_sequential_route",
                emitted_fixture("dag-fork/valid/fork_sequential_route.awl")?,
            ),
            (
                "fork_named_homogeneous",
                emitted_fixture("dag-fork/valid/fork_named_homogeneous.awl")?,
            ),
            ("archived_awl_exam", emitted_archived_exam()?),
            // `backward_route_bounded_cycle` left this set at rev 3 with
            // `ship_release_combined`: its cycle now carries the
            // `max … visits` bound, which refuses to emit until B4.
            (
                "compound_until_nested",
                emitted_fixture("loop-outcomes/valid/loop_compound_until_nested.awl")?,
            ),
            (
                "loop_without_counting",
                emitted_fixture("loop-outcomes/valid/loop_without_counting.awl")?,
            ),
            (
                "substeps",
                emitted_fixture("loop-outcomes/valid/substeps_two_stage.awl")?,
            ),
            (
                "combinators",
                emitted_fixture("step-bodies/valid/combinators.awl")?,
            ),
            (
                "float_guard",
                emitted_fixture("loop-outcomes/valid/float_threshold_guard.awl")?,
            ),
            (
                "step_bodies",
                emitted_fixture("step-bodies/valid/step_bodies_combined.awl")?,
            ),
            (
                "mixed_doors",
                emitted_fixture("schema-doors/valid/mixed_doors.awl")?,
            ),
            (
                "declarations",
                emitted_fixture("declarations/valid/declarations_combined.awl")?,
            ),
        ],
    )
}

/// Signal-sender compile proof: the emitted `signal.send(…)` call, its
/// outbound `SignalRef` constructors, and the `map_engine_error` threading
/// are real Gleam against the local SDK — not merely well-formed text. This
/// is the proof that catches an arity or argument-order drift against
/// `aion/signal.gleam`, which no AWL-side test could see.
#[test]
fn signal_sender_fixture_compiles_under_gleam() -> Result<(), Box<dyn Error>> {
    if crate::gleam_test_support::skip_if_unavailable() {
        return Ok(());
    }
    compile_generated_modules(
        "signal_sender",
        &[(
            "gated_item",
            emitted_fixture("step-bodies/valid/send_signal.awl")?,
        )],
    )
}

/// Query-surface compile proof: the emitted `query.handler(…)` call, its
/// answer-type codec argument, the reply thunk, and the `Result`
/// discrimination are real Gleam against the local SDK — not merely
/// well-formed text. This is the proof that catches an arity or
/// argument-order drift against `aion/query.gleam`, which no AWL-side test
/// could see.
#[test]
fn query_surface_fixture_compiles_under_gleam() -> Result<(), Box<dyn Error>> {
    if crate::gleam_test_support::skip_if_unavailable() {
        return Ok(());
    }
    compile_generated_modules(
        "query_surface",
        &[(
            "gated_review",
            emitted_fixture("step-bodies/valid/query_answer.awl")?,
        )],
    )
}

fn compile_generated_modules(
    project_name: &str,
    modules: &[(&str, String)],
) -> Result<(), Box<dyn Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = root.parent().and_then(Path::parent).ok_or_else(|| {
        io::Error::other("failed to resolve repository root from CARGO_MANIFEST_DIR")
    })?;
    let project = scratch_project(project_name)?;
    fs::create_dir_all(project.join("src"))?;
    fs::write(
        project.join("gleam.toml"),
        format!(
            "name = \"awl_{project_name}_compile_proof\"\nversion = \"0.1.0\"\ntarget = \
             \"erlang\"\n\n[dependencies]\naion_flow = {{ path = \"{}\" }}\ngleam_stdlib = \
             \">= 0.34.0 and < 2.0.0\"\ngleam_json = \">= 2.0.0 and < 4.0.0\"\n",
            repo_root.join("gleam/aion_flow").display()
        ),
    )?;
    for (module_name, module_source) in modules {
        fs::write(
            project.join("src").join(format!("{module_name}.gleam")),
            module_source,
        )?;
    }
    // The proof is the `.beam` per module, not merely a zero exit: this test
    // cannot pass without having compiled every emitted module. A failure in
    // `gleam`'s own dependency layer is retried and then named as an
    // infrastructure failure; a rejected module fails immediately.
    let module_names: Vec<&str> = modules.iter().map(|(name, _)| *name).collect();
    crate::gleam_test_support::compile_project(&project, &module_names)?;
    Ok(())
}

/// Stable build root for one compile proof, under the shared cargo target
/// pile — NEVER the system temp directory.
///
/// This used to build under `std::env::temp_dir()` with a name made unique per
/// process and nanosecond, so no run ever reused or reclaimed anything: every
/// invocation left a complete Gleam/Erlang build tree behind, in the one place
/// `cargo clean` cannot reach. The crate's other harness
/// (`runtime_codecs/harness.rs`) already states the rule this broke — build
/// artefacts in the system temp directory are banned, and a stable name under
/// `target/` is what bounds the pile.
///
/// The name is stable per label so repeat runs REUSE the Gleam build cache
/// instead of accumulating, and the four labels are distinct, so parallel
/// tests never race one directory.
///
/// `src/` is emptied first. Reuse without it would leave a previous run's
/// modules beside this run's — `gleam build` compiles the whole `src`
/// directory, so a module deleted from the proof would keep being compiled and
/// a rename would compile both halves. The build cache lives outside `src/`
/// and is deliberately kept.
/// The build root is somewhere `cargo clean` can reach, and is never the
/// system temp directory.
///
/// Pinned because the failure it guards is invisible: a proof that builds into
/// the system temp directory still passes, still compiles the modules, and
/// reports nothing — it just leaves a full Gleam/Erlang tree behind on every
/// single run, in the one place no cleanup command looks. This machine has
/// already been minutes from full once.
///
/// Asserted against the computed roots rather than a written path, so the pin
/// cannot drift from where the build actually goes.
#[test]
fn the_compile_proof_builds_where_cargo_clean_can_reach_it() -> Result<(), Box<dyn Error>> {
    let system_temp = std::env::temp_dir();
    assert!(
        system_temp.is_absolute(),
        "the control is meaningless if the system temp directory is not a real path: \
         {system_temp:?}"
    );
    let target_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .ok_or("failed to resolve the workspace root")?
        .join("target");

    let project = scratch_project("path_pin")?;
    assert!(
        project.starts_with(&target_root),
        "the compile proof must build under the cargo target pile so `cargo clean` bounds it, \
         got {project:?} against {target_root:?}"
    );
    assert!(
        !project.starts_with(&system_temp),
        "build artefacts in the system temp directory are banned — nothing ever reclaims them: \
         {project:?}"
    );
    Ok(())
}

fn scratch_project(project_name: &str) -> Result<PathBuf, Box<dyn Error>> {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| io::Error::other("failed to resolve workspace root for the scratch build"))?
        .to_path_buf();
    let project = workspace_root
        .join("target/awl-test-scratch/compile-proof")
        .join(project_name);
    let sources = project.join("src");
    if sources.exists() {
        fs::remove_dir_all(&sources)?;
    }
    Ok(project)
}
