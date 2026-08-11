//! The standing control arm for the `harness` section.
//!
//! A `harness` section declares how a WORKER PROCESS launches an agent: which harness,
//! how many activities at once, how it reconnects, what environment the subprocess gets.
//! None of that is flow meaning, so it must be invisible to the lowering: the same
//! workflow, with the section and without it, has to produce the same MIR. If it ever
//! does not, the section has stopped being launch configuration and started being a
//! control-flow construct, and every deployed document's meaning is in question.
//!
//! This is the ABSENCE arm the invariance claim needs. Three fixtures that all carry a
//! section, agreeing with each other, cannot tell "the section does not change the
//! lowering" from "these three sections happen to agree" — the comparison has to be
//! against a document that carries none.
//!
//! Deliberately a comparison against a BASELINE rather than a golden of its own: a golden
//! pins what the lowering currently is, this pins the PROPERTY, so it keeps holding
//! through every future change to how activity calls lower.

use std::fs;
use std::path::{Path, PathBuf};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// The committed MIR golden for a `valid/` fixture.
fn golden(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/mir-goldens")
        .join(format!("{name}.mir"))
}

/// The fixture corpus root.
fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rev2/harness-section/valid")
}

/// The one identifier the comparison normalises, and the reason it must.
///
/// A workflow's name becomes its package entry module, and two workflows sharing one name
/// cannot be loaded together — the BC-4 differential loads the whole covered corpus at
/// once and refuses the duplicate outright. So the control arm carries its own name, and
/// exactly that name is mapped back before the diff. Nothing else is normalised: every
/// other byte must match on its own.
///
/// Both casings, because a workflow name reaches the MIR twice — as the
/// module/`snake_case` identifier and as the `TitleCase` stem of its generated input and
/// outcome types. Missing the second would leave real text differing and make the
/// comparison permanently red for a reason that is not about the section at all.
const RENAMES: [(&str, &str); 2] = [
    ("norn_harness_launch_control", "norn_harness_launch"),
    ("NornHarnessLaunchControl", "NornHarnessLaunch"),
];

/// Map the control arm's own name back onto the declaring fixture's.
fn normalise_name(source: &str) -> String {
    RENAMES
        .iter()
        .fold(source.to_owned(), |text, (from, to)| text.replace(from, to))
}

/// THE INVARIANT: a document with a `harness` section and the same document without one
/// lower identically.
#[test]
fn a_harness_section_does_not_change_the_lowering() -> TestResult {
    let declared = fs::read_to_string(golden("harness-section/valid/norn_harness"))?;
    let control = fs::read_to_string(golden("harness-section/valid/harness_absent_control"))?;

    // Guard the normalisation itself: a rename that stopped matching would make the
    // substitution a no-op, and the test would be comparing something other than what it
    // claims to.
    for (from, _) in RENAMES {
        assert!(
            control.contains(from),
            "the control golden must carry `{from}`, or that normalisation is silently \
             doing nothing and the comparison is not what it says it is"
        );
    }
    let absent = normalise_name(&control);

    assert_eq!(
        declared, absent,
        "the `harness` section changed the lowering. It declares how a WORKER PROCESS \
         launches an agent — it must not touch the flow. If this is a deliberate change, \
         it is a language change and not a refactor: the section now means something it \
         did not mean when documents were written against it."
    );
    Ok(())
}

/// The non-vacuity guard. Two goldens that agree because the SOURCES are the same file,
/// or because one is empty, would pass the test above while proving nothing.
#[test]
fn the_control_pair_differs_only_by_the_section() -> TestResult {
    let declared = fs::read_to_string(fixtures().join("norn_harness.awl"))?;
    let control = fs::read_to_string(fixtures().join("harness_absent_control.awl"))?;

    assert_ne!(
        declared, control,
        "the pair must be two distinct documents, or the comparison is a file against \
         itself"
    );
    assert!(
        declared.contains("\n  harness\n"),
        "the declaring fixture must actually carry a section"
    );
    assert!(
        !control.contains("\n  harness\n"),
        "the control fixture must NOT carry a section, or both arms are the same arm"
    );

    // Body equality below the doc header, with the section and the workflow name — the
    // two sanctioned differences — mapped away. A drift in the flow itself would make the
    // identical-MIR assertion true for the wrong reason.
    let body = |source: &str, drop_section: bool| {
        let mut kept = Vec::new();
        let mut inside_section = false;
        for line in source
            .lines()
            .skip_while(|line| line.starts_with("//!") || line.trim().is_empty())
        {
            if drop_section && line == "  harness" {
                inside_section = true;
                continue;
            }
            if inside_section {
                if line.starts_with("    ") {
                    continue;
                }
                inside_section = false;
            }
            kept.push(normalise_name(line));
        }
        kept.join("\n")
    };
    assert_eq!(
        body(&declared, true),
        body(&control, false),
        "the two fixtures must differ ONLY by the section; any other divergence means \
         they are no longer a control pair"
    );

    let declared_golden = fs::read_to_string(golden("harness-section/valid/norn_harness"))?;
    assert!(
        !declared_golden.trim().is_empty(),
        "an empty golden would make the identity assertion vacuously true"
    );
    Ok(())
}
