//! The differential driver: classify a fixture set into the lowering
//! intersection vs out-of-intersection refusals, build both backends, run
//! both, and accumulate the divergence report.

use std::collections::HashMap;

use aion_awl::mir::lower;
use serde_json::Value;

use crate::fixtures::{self, ActionResult, Loaded};
use crate::harness::{self, RefEntry};
use crate::report::Report;
use crate::run::{Disposition, RunOutcome, run_package};

/// The classified fixture set: everything both backends accept, plus every
/// out-of-intersection refusal.
pub struct Classified {
    /// Reference-build inputs for the lowering fixtures, in input order.
    pub entries: Vec<RefEntry>,
    /// Direct `select`ed bytes per ratchet name.
    pub direct_bytes: HashMap<String, Vec<u8>>,
    /// Minimal schema-valid input per ratchet name.
    pub inputs: HashMap<String, Value>,
    /// Schema-valid canned activity results per ratchet name.
    pub action_results: HashMap<String, HashMap<String, ActionResult>>,
    /// Out-of-intersection refusals as `(ratchet name, refusal text)`.
    pub refusals: Vec<(String, String)>,
    /// Divergences discovered during classification (e.g. IR-12 name skew).
    pub classification_divergences: Vec<(String, String, String)>,
    /// Generated witnesses that do not satisfy the schema they were generated
    /// for, as `(ratchet name, fault)`. A harness fault, reported as INFRA —
    /// but reported HERE, naming the fixture and the keyword, instead of
    /// reaching the engine and surfacing as an opaque run error.
    pub witness_faults: Vec<(String, String)>,
}

/// Classifies each covered fixture: a `lower` refusal (or an emit/select
/// failure) is out-of-intersection and recorded; a fixture both backends
/// accept is resolved to its reference-build inputs and direct bytes.
///
/// # Errors
///
/// Fails only on unexpected I/O (a fixture that cannot be read at all); every
/// refusal is captured, never surfaced as an error.
pub fn classify(names: &[String]) -> Result<Classified, Box<dyn std::error::Error>> {
    let mut classified = Classified {
        entries: Vec::new(),
        direct_bytes: HashMap::new(),
        inputs: HashMap::new(),
        action_results: HashMap::new(),
        refusals: Vec::new(),
        classification_divergences: Vec::new(),
        witness_faults: Vec::new(),
    };
    for name in names {
        let loaded = fixtures::load(name)?;
        match lower(&loaded.document, Some(loaded.dir.as_path())) {
            Ok(module) => classify_lowering(&mut classified, name, &loaded, &module),
            Err(error) => classified.refusals.push((name.clone(), error.to_string())),
        }
    }
    Ok(classified)
}

/// Resolves one lowering fixture; a downstream emit/select failure is recorded
/// as an out-of-intersection refusal, and an entry-module name skew between the
/// two backends is recorded as an IR-12 divergence.
fn classify_lowering(
    classified: &mut Classified,
    name: &str,
    loaded: &Loaded,
    module: &aion_awl::mir::MirModule,
) {
    let entry = match harness::ref_entry(loaded) {
        Ok(entry) => entry,
        Err(error) => {
            classified
                .refusals
                .push((name.to_owned(), format!("reference emit refused: {error}")));
            return;
        }
    };
    let bytes = match harness::select_direct(module) {
        Ok(bytes) => bytes,
        Err(error) => {
            classified
                .refusals
                .push((name.to_owned(), format!("direct select refused: {error}")));
            return;
        }
    };
    if module.name != entry.entry_module {
        classified.classification_divergences.push((
            name.to_owned(),
            format!("direct module name `{}`", module.name),
            format!("reference entry module `{}`", entry.entry_module),
        ));
        return;
    }
    let input = fixtures::example_for_schema(&entry.input_schema);
    if let Some(fault) = fixtures::witness_fault(&entry.input_schema, &input) {
        classified
            .witness_faults
            .push((name.to_owned(), format!("start input: {fault}")));
    }
    let (results, result_faults) = fixtures::action_results(&loaded.document, loaded.dir.as_path());
    for fault in result_faults {
        classified.witness_faults.push((name.to_owned(), fault));
    }
    classified.inputs.insert(name.to_owned(), input);
    classified.action_results.insert(name.to_owned(), results);
    classified.direct_bytes.insert(name.to_owned(), bytes);
    classified.entries.push(entry);
}

/// Runs the full differential over a fixture set and returns the report.
///
/// # Errors
///
/// Fails on classification or reference-build errors (a genuine backend
/// defect); per-fixture run outcomes are folded into the report, not surfaced.
pub async fn run_differential(
    names: &[String],
    label: &str,
    excluded: &[&str],
) -> Result<Report, Box<dyn std::error::Error>> {
    let mut report = Report::default();
    // Excluded fixtures are out-of-oracle: recorded, never run (their park
    // boundary is unprovable without racy sampling — see covered.rs).
    let run_names: Vec<String> = names
        .iter()
        .filter(|name| {
            if excluded.contains(&name.as_str()) {
                report.record_excluded(name);
                false
            } else {
                true
            }
        })
        .cloned()
        .collect();

    let classified = classify(&run_names)?;
    for (fixture, text) in &classified.refusals {
        report.record_refusal(fixture, text.clone());
    }
    // A witness the harness itself generated wrongly is the harness's fault, so
    // it is INFRA — but it is recorded HERE, naming the fixture and the failing
    // keyword, rather than travelling to the engine and coming back as an
    // opaque run error that reads like an environment problem.
    for (fixture, fault) in &classified.witness_faults {
        report.record_infra(fixture, fault);
    }
    for (fixture, left, right) in &classified.classification_divergences {
        report.compare(
            fixture,
            &[Value::String(left.clone())],
            &[Value::String(right.clone())],
        );
    }

    let reference = harness::build_reference(&classified.entries, label)?;
    for entry in &classified.entries {
        differential_one(&mut report, &reference, &classified, entry).await;
    }
    Ok(report)
}

/// A fixture built through both backends, exposing the RELOADED spliced direct
/// package and its entry bytes so the ABI suite asserts against the bytes the
/// engine actually loads (BLOCKER 4), not fresh `select()` output.
pub struct SplicedFixture {
    /// Emitted entry-module / workflow-type name.
    pub entry_module: String,
    /// The `select`ed entry bytes (identical to the spliced entry beam).
    pub entry_bytes: Vec<u8>,
    /// The reloaded direct package (spliced entry over the SDK closure).
    pub direct_package: aion_package::Package,
    /// The reference (Gleam-built) package, for the oracle self-test.
    pub reference_package: aion_package::Package,
    /// The minimal schema-valid start input.
    pub input: Value,
    /// The schema-valid canned activity results.
    pub action_results: HashMap<String, ActionResult>,
}

/// Builds the spliced direct package for each named fixture (ONE `gleam
/// build`). Every named fixture MUST lie in the lowering intersection; a
/// refusal or classification skew is an error, since the ABI suite pins fixtures
/// it has already established both backends accept.
///
/// # Errors
///
/// Fails when a named fixture refuses/skews, the reference build fails, or the
/// splice-integrity precondition is violated.
pub async fn build_spliced(
    names: &[String],
    label: &str,
) -> Result<Vec<SplicedFixture>, Box<dyn std::error::Error>> {
    let classified = classify(names)?;
    if !classified.refusals.is_empty()
        || !classified.classification_divergences.is_empty()
        || !classified.witness_faults.is_empty()
    {
        return Err(format!(
            "ABI fixtures must be in the intersection with satisfiable witnesses: \
             refusals={:?} skews={:?} witness faults={:?}",
            classified.refusals, classified.classification_divergences, classified.witness_faults
        )
        .into());
    }
    let reference = harness::build_reference(&classified.entries, label)?;
    let mut out = Vec::new();
    for entry in &classified.entries {
        let reference_package = reference
            .package(&entry.name)
            .ok_or_else(|| format!("no reference package for {}", entry.name))?;
        let entry_bytes = classified
            .direct_bytes
            .get(&entry.name)
            .ok_or_else(|| format!("no direct bytes for {}", entry.name))?
            .clone();
        let direct_package = harness::splice_direct(reference_package, &entry_bytes)?;
        out.push(SplicedFixture {
            entry_module: entry.entry_module.clone(),
            entry_bytes,
            direct_package,
            reference_package: reference_package.clone(),
            input: classified
                .inputs
                .get(&entry.name)
                .cloned()
                .unwrap_or(Value::Null),
            action_results: classified
                .action_results
                .get(&entry.name)
                .cloned()
                .unwrap_or_default(),
        });
    }
    Ok(out)
}

/// Runs one fixture through both backends and folds the outcome into the
/// report.
async fn differential_one(
    report: &mut Report,
    reference: &harness::ReferenceBuild,
    classified: &Classified,
    entry: &RefEntry,
) {
    let name = entry.name.as_str();
    let Some(reference_package) = reference.package(name) else {
        report.record_infra(name, "no reference package produced");
        return;
    };
    let Some(direct_bytes) = classified.direct_bytes.get(name) else {
        report.record_infra(name, "no direct select bytes");
        return;
    };
    // The splice is mutation-sensitive by construction: it fails unless the
    // reloaded direct package carries the select() bytes as its entry module,
    // differs from the reference entry, and hashes differently.
    let direct_package = match harness::splice_direct(reference_package, direct_bytes) {
        Ok(package) => package,
        Err(error) => {
            report.record_infra(name, &format!("splice integrity violation: {error}"));
            return;
        }
    };
    let Some(reference_entry) = reference_package.beams().get(&entry.entry_module) else {
        report.record_infra(name, "reference package has no entry beam");
        return;
    };
    let reference_entry = reference_entry.to_vec();
    let input = classified.inputs.get(name).cloned().unwrap_or(Value::Null);
    let results = classified
        .action_results
        .get(name)
        .cloned()
        .unwrap_or_default();
    let workflow_type = entry.entry_module.as_str();

    // Splice binding (r2 finding 1): each run is bound to the exact entry bytes
    // of ITS production — the reference run to the reference entry, the direct
    // run to the select() bytes. run_package refuses a package whose entry does
    // not match, so a run against the wrong package fails before execution.
    let reference_run = run_package(
        reference_package.clone(),
        workflow_type,
        &input,
        results.clone(),
        &reference_entry,
    )
    .await;
    let direct_run =
        run_package(direct_package, workflow_type, &input, results, direct_bytes).await;
    fold_runs(report, name, reference_run, direct_run);
}

/// Compares the two run outcomes and records the result.
///
/// A `Stuck` disposition or a serialization failure is INFRA (a hard failure),
/// never quiesced. A backend disagreement on disposition is a divergence. A
/// run REFUSAL is classified by whether the backends agree on it: identical
/// refusals are agreement, a one-sided or differing refusal is a divergence,
/// and neither is infrastructure noise.
/// Aliases the run-uuid inside `timer:named:deadline:<run-id>` keys so the
/// two backends' park summaries compare on timer STRUCTURE; every other key
/// passes through byte-exact.
fn alias_deadline_timers(timers: &[String]) -> Vec<String> {
    timers
        .iter()
        .map(|key| {
            key.strip_prefix("timer:named:deadline:")
                .map_or_else(|| key.clone(), |_| "timer:named:deadline:<run>".to_owned())
        })
        .collect()
}

/// Aliases per-run identities out of a refusal message so two refusals compare
/// on their REASON.
///
/// The two backends necessarily carry DIFFERENT package version hashes — that
/// difference is the whole point of the splice — so an identical refusal
/// reaches this function as two strings that differ. Comparing them raw would
/// class every agreed refusal as a divergence.
fn alias_run_identities(message: &str) -> String {
    let mut out = String::with_capacity(message.len());
    let mut run = String::new();
    for character in message.chars() {
        if character.is_ascii_hexdigit() || character == '-' {
            run.push(character);
            continue;
        }
        flush_identity_run(&mut out, &mut run);
        out.push(character);
    }
    flush_identity_run(&mut out, &mut run);
    out
}

/// Flushes an accumulated `[0-9a-f-]` run, replacing it with `<id>` when it
/// carries enough hex digits to be a content hash or a uuid rather than a word.
fn flush_identity_run(out: &mut String, run: &mut String) {
    if run.chars().filter(char::is_ascii_hexdigit).count() >= 32 {
        out.push_str("<id>");
    } else {
        out.push_str(run);
    }
    run.clear();
}

/// Renders one side of a run outcome for a refusal-split divergence: either it
/// ran, or it refused for a stated reason.
fn describe_run(error: Option<&(dyn std::error::Error + 'static)>) -> String {
    error.map_or_else(
        || String::from("ran"),
        |error| format!("refused: {}", alias_run_identities(&error.to_string())),
    )
}

fn fold_runs(
    report: &mut Report,
    name: &str,
    reference_run: Result<RunOutcome, Box<dyn std::error::Error>>,
    direct_run: Result<RunOutcome, Box<dyn std::error::Error>>,
) {
    let (reference, direct) = match (reference_run, direct_run) {
        (Ok(reference), Ok(direct)) => (reference, direct),
        // BOTH backends refused the run, identically. That is the differential
        // AGREEING — the exact signal this harness exists to produce — and
        // scoring it as infrastructure noise made the instrument quietest at
        // the moment it was working. It gets its own bucket, pinned empty by
        // the covered ratchet, so it is classified honestly AND still loud.
        (Err(reference_error), Err(direct_error))
            if alias_run_identities(&reference_error.to_string())
                == alias_run_identities(&direct_error.to_string()) =>
        {
            report.record_agreed_refusal(name, &alias_run_identities(&reference_error.to_string()));
            return;
        }
        // One backend refused and the other did not, or they refused for
        // different reasons. That is a behavioral SPLIT between the backends,
        // which is a divergence — not noise either.
        (reference_run, direct_run) => {
            report.compare(
                name,
                &[Value::String(describe_run(reference_run.err().as_deref()))],
                &[Value::String(describe_run(direct_run.err().as_deref()))],
            );
            return;
        }
    };
    if reference.disposition == Disposition::Stuck || direct.disposition == Disposition::Stuck {
        report.record_infra(
            name,
            &format!(
                "run never reached a terminal or a stable durable-timer park \
                 (ref={:?}, direct={:?})",
                reference.disposition, direct.disposition
            ),
        );
        return;
    }
    let reference_trail = match crate::trail_norm::normalized_trail(&reference.trail) {
        Ok(trail) => trail,
        Err(error) => {
            report.record_infra(name, &format!("reference trail serialize: {error}"));
            return;
        }
    };
    let direct_trail = match crate::trail_norm::normalized_trail(&direct.trail) {
        Ok(trail) => trail,
        Err(error) => {
            report.record_infra(name, &format!("direct trail serialize: {error}"));
            return;
        }
    };
    if reference.disposition != direct.disposition {
        report.compare(
            name,
            &[Value::String(format!(
                "disposition={:?}",
                reference.disposition
            ))],
            &[Value::String(format!(
                "disposition={:?}",
                direct.disposition
            ))],
        );
        return;
    }
    // The two backends must agree on the pending-timer identities too (a
    // different timer on either side would be a real behavioral split). The
    // declared-timeout deadline embeds each run's own uuid in its name, so
    // that one identity is aliased before comparison — wrong-run deadline
    // divergence stays caught in the normalized trails, where run ids alias
    // positionally.
    let reference_timers = alias_deadline_timers(&reference.pending_timers);
    let direct_timers = alias_deadline_timers(&direct.pending_timers);
    if reference.disposition == Disposition::Parked && reference_timers != direct_timers {
        report.compare(
            name,
            &[Value::String(format!("timers={reference_timers:?}"))],
            &[Value::String(format!("timers={direct_timers:?}"))],
        );
        return;
    }
    if report.compare(name, &reference_trail, &direct_trail) > 0 {
        return;
    }
    match reference.disposition {
        Disposition::Completed => report.record_succeeded(name),
        Disposition::Failed => {
            // A fixture landing in the failed bucket is only a correct pin if it
            // fails for the DESIGNED reason, and the bucket name alone cannot
            // tell you which. `BC4_DUMP_FAILED=<fixture name>` prints that
            // fixture's reference trail so the recorded failure can be read
            // rather than inferred — the difference between pinning a property
            // and fossilising a defect as expected behaviour.
            if std::env::var("BC4_DUMP_FAILED").is_ok_and(|wanted| wanted == name) {
                for event in &reference.trail {
                    eprintln!("TRAIL {event:?}");
                }
            }
            report.record_failed(name);
        }
        Disposition::Cancelled => report.record_cancelled(name),
        Disposition::Parked => report.record_parked_timer(name, reference_timers),
        // `Stuck` was handled above as infra.
        Disposition::Stuck => report.record_infra(name, "unreachable stuck classification"),
    }
}

/// The classification arms proven by construction, because the situations they
/// exist for are ones a healthy corpus never produces.
///
/// This is the control arm for a guard that has never been observed to fire.
/// `fold_runs` learned to tell an AGREEMENT on refusal apart from a backend
/// SPLIT in the same change that removed the corpus's only refusing fixture —
/// so on every real run since, both arms have been dead code that reads as
/// working. A branch nobody has watched execute is indistinguishable from one
/// that cannot.
#[cfg(test)]
mod classification_tests {
    use super::{Report, alias_run_identities, fold_runs};

    /// One backend's run outcome, exactly as `fold_runs` receives it.
    type RunResult = Result<crate::run::RunOutcome, Box<dyn std::error::Error>>;

    /// Two runs that failed with the same reason, differing only in the
    /// package hash each backend necessarily carries.
    fn refusals(reason: &str) -> (RunResult, RunResult) {
        (
            Err(format!("{reason} `{}`", "a".repeat(64)).into()),
            Err(format!("{reason} `{}`", "b".repeat(64)).into()),
        )
    }

    #[test]
    fn identical_refusals_are_agreement_not_infrastructure_noise() {
        let mut report = Report::default();
        let (reference, direct) = refusals("start input does not satisfy version");
        fold_runs(&mut report, "fixture/one", reference, direct);

        assert_eq!(
            report.agreed_refusal_fixtures(),
            vec![String::from("fixture/one")],
            "two identical refusals are the differential AGREEING"
        );
        assert!(
            report.infra.is_empty(),
            "agreement must not be scored as infrastructure noise: {:?}",
            report.infra
        );
        assert!(
            report.divergences.is_empty(),
            "agreement is not a divergence"
        );
        assert!(
            report.agreed_refusals[0].1.contains("<id>"),
            "the per-backend package hash must be aliased out, or two identical \
             refusals could never compare equal: {:?}",
            report.agreed_refusals[0].1
        );
    }

    #[test]
    fn a_one_sided_refusal_is_a_divergence() {
        let mut report = Report::default();
        let (reference, _) = refusals("start input does not satisfy version");
        fold_runs(
            &mut report,
            "fixture/two",
            reference,
            Ok(crate::run::RunOutcome {
                trail: Vec::new(),
                disposition: crate::run::Disposition::Completed,
                pending_timers: Vec::new(),
            }),
        );

        assert_eq!(
            report.diverged,
            vec![String::from("fixture/two")],
            "one backend refusing and the other running is a behavioural SPLIT"
        );
        assert!(
            report.agreed_refusals.is_empty(),
            "a one-sided refusal is not agreement"
        );
        assert!(report.infra.is_empty(), "nor is it infrastructure noise");
    }

    #[test]
    fn refusals_for_different_reasons_are_a_divergence() {
        let mut report = Report::default();
        fold_runs(
            &mut report,
            "fixture/three",
            Err("input is too short".into()),
            Err("input is the wrong type".into()),
        );

        assert_eq!(
            report.diverged,
            vec![String::from("fixture/three")],
            "two backends refusing for DIFFERENT reasons disagree"
        );
        assert!(report.agreed_refusals.is_empty());
    }

    /// The aliaser must erase identities without erasing the reason, or every
    /// pair of refusals would compare equal and every split would read as
    /// agreement.
    #[test]
    fn aliasing_erases_identities_and_nothing_else() {
        let hashed = alias_run_identities(&format!("version `{}` refused", "0123abcd".repeat(8)));
        assert_eq!(hashed, "version `<id>` refused");

        let uuid = alias_run_identities("run 550e8400-e29b-41d4-a716-446655440000 failed");
        assert_eq!(uuid, "run <id> failed");

        // Ordinary words built from hex letters must survive: `deadbeef` and
        // `added` are shorter than any identity and carry meaning.
        assert_eq!(
            alias_run_identities("deadbeef added to the cafe"),
            "deadbeef added to the cafe"
        );
        assert_ne!(
            alias_run_identities("input is too short"),
            alias_run_identities("input is the wrong type"),
            "aliasing must not collapse two different reasons into one"
        );
    }
}
