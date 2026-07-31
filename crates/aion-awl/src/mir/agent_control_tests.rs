//! The standing control arm for the `agent` marker.
//!
//! `agent` classifies an action DECLARATION — it tells a worker to hand the
//! call to an agent harness instead of a typed handler. It must therefore be
//! invisible to the flow: the same workflow, marked and unmarked, has to lower
//! to the same MIR. If it ever does not, the marker has stopped being a
//! classification and started being a control-flow construct, and every
//! existing document's meaning is in question.
//!
//! This is deliberately a comparison against a BASELINE rather than a golden of
//! its own. A golden pins what the lowering currently is; this pins the
//! PROPERTY, so it keeps holding through every future change to how activity
//! calls lower.

use std::fs;
use std::path::{Path, PathBuf};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// The committed MIR golden for a `valid/` fixture.
fn golden(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/mir-goldens")
        .join(format!("{name}.mir"))
}

/// The one identifier the comparison normalises, and the reason it must.
///
/// A workflow's name becomes its package entry module, and two workflows
/// sharing one name cannot be loaded together — the BC-4 differential loads the
/// whole covered corpus at once and refuses the duplicate outright. So the
/// control arm carries its own name, and exactly that name is mapped back
/// before the diff. Nothing else is normalised: every other byte must match on
/// its own.
///
/// It is mapped in BOTH casings because a workflow name reaches the MIR twice —
/// as the module/`snake_case` identifier and as the `TitleCase` stem of its
/// generated input and outcome types (`AskTheAgentControlInput`). Missing the
/// second casing would leave real text differing and make the comparison
/// permanently red for a reason that is not about the marker at all.
const RENAMES: [(&str, &str); 2] = [
    ("ask_the_agent_control", "ask_the_agent"),
    ("AskTheAgentControl", "AskTheAgent"),
];

/// Map the control arm's own name back onto the marked fixture's.
fn normalise_name(source: &str) -> String {
    RENAMES
        .iter()
        .fold(source.to_owned(), |text, (from, to)| text.replace(from, to))
}

/// THE INVARIANT: marked and unmarked lower identically.
#[test]
fn agent_marked_and_unmarked_lower_identically() -> TestResult {
    let marked = fs::read_to_string(golden("declarations/valid/agent_seam"))?;
    let control = fs::read_to_string(golden("declarations/valid/agent_seam_unmarked_control"))?;

    // Guard the normalisation itself: a rename that stops matching would make
    // the substitution a no-op, and the test would be comparing something other
    // than what it claims to.
    for (from, _) in RENAMES {
        assert!(
            control.contains(from),
            "the control golden must carry `{from}`, or that normalisation is silently \
             doing nothing and the comparison is not what it says it is"
        );
    }
    let unmarked = normalise_name(&control);

    assert_eq!(
        marked, unmarked,
        "the `agent` marker changed the lowering. It classifies the DECLARATION — it \
         tells a worker to route the call to an agent harness — and it must not touch \
         the flow. If this is a deliberate change, it is a language change, not a \
         refactor: the marker now means something it did not mean when documents were \
         written against it."
    );
    Ok(())
}

/// The non-vacuity guard. Two goldens that agree because the SOURCES are the
/// same file, or because one is empty, would pass the test above while proving
/// nothing. This pins that the inputs really are a marked/unmarked pair.
#[test]
fn the_control_pair_differs_only_by_the_marker() -> TestResult {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rev2");
    let marked = fs::read_to_string(fixtures.join("declarations/valid/agent_seam.awl"))?;
    let unmarked =
        fs::read_to_string(fixtures.join("declarations/valid/agent_seam_unmarked_control.awl"))?;

    assert_ne!(
        marked, unmarked,
        "the pair must be two distinct documents, or the comparison is a file against itself"
    );
    assert!(
        marked.contains("agent, timeout 10m"),
        "the marked fixture must actually carry the marker"
    );
    assert!(
        !unmarked.contains("\n    agent") && !unmarked.contains("agent,"),
        "the control fixture must NOT carry the marker, or both arms are the same arm"
    );

    // Body equality below the doc header: a drift in the flow itself would make
    // the identical-MIR assertion true for the wrong reason. The marker and the
    // workflow name are the two sanctioned differences, so both are mapped away
    // here and everything else must match line for line.
    let body = |source: &str| {
        source
            .lines()
            .skip_while(|line| line.starts_with("//!") || line.trim().is_empty())
            .map(|line| normalise_name(&line.replace("agent, timeout 10m", "timeout 10m")))
            .filter(|line| !line.trim_start().starts_with("///"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert_eq!(
        body(&marked),
        body(&unmarked),
        "the two fixtures must differ ONLY by the marker; any other divergence means \
         they are no longer a control pair"
    );

    let marked_golden = fs::read_to_string(golden("declarations/valid/agent_seam"))?;
    assert!(
        !marked_golden.trim().is_empty(),
        "an empty golden would make the identity assertion vacuously true"
    );
    Ok(())
}
