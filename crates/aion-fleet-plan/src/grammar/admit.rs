//! The `admit` expression (design §2.1).
//!
//! `admit=<always|stage-id.conclusion==value|stage-id.conclusion!=value>`
//!
//! That is the whole admissible surface. Design §4: *"Free-form report text
//! is never a branch condition."* Design §10 refuses free-form prose
//! conditions outright as "not safely typed, auditable, or
//! replay-deterministic". So the parser admits three shapes and nothing
//! else — there is no expression language here to grow one.

use crate::cause::CauseCode;
use crate::error::PlanRefusal;
use crate::graph::{Admit, Comparison};

/// The accessor an admit condition reads. Only a stage's recorded
/// conclusion is readable; there is no other field to name.
const ACCESSOR: &str = ".conclusion";

/// Parse an `admit` value.
///
/// # Errors
///
/// Refuses with [`CauseCode::MalformedAdmit`] for anything that is not one
/// of the three admissible shapes.
pub fn parse_admit(raw: &str, stage_id: &str) -> Result<Admit, PlanRefusal> {
    if raw == "always" {
        return Ok(Admit::Always);
    }

    // Order matters only in that `==` and `!=` are both two bytes and
    // neither is a prefix of the other, so the first occurrence of either
    // is the operator.
    let (comparison, at) = match (raw.find("=="), raw.find("!=")) {
        (Some(eq), Some(ne)) if eq < ne => (Comparison::Equals, eq),
        (Some(_) | None, Some(ne)) => (Comparison::NotEquals, ne),
        (Some(eq), None) => (Comparison::Equals, eq),
        (None, None) => {
            return Err(malformed(raw, stage_id, "no `==` or `!=` comparison"));
        }
    };

    let (lhs, rhs) = raw.split_at(at);
    let value = &rhs[2..];

    let Some(referenced) = lhs.strip_suffix(ACCESSOR) else {
        return Err(malformed(
            raw,
            stage_id,
            "the left side must be `<stage-id>.conclusion`",
        ));
    };
    if referenced.is_empty() {
        return Err(malformed(raw, stage_id, "the left side names no stage"));
    }
    if referenced.contains('.') {
        return Err(malformed(
            raw,
            stage_id,
            "the left side names more than one accessor",
        ));
    }
    if value.is_empty() {
        return Err(malformed(raw, stage_id, "the right side is empty"));
    }
    if value.contains(['=', '!']) {
        return Err(malformed(
            raw,
            stage_id,
            "the right side carries a second comparison",
        ));
    }

    Ok(Admit::Conclusion {
        stage_id: referenced.to_owned(),
        comparison,
        value: value.to_owned(),
    })
}

fn malformed(raw: &str, stage_id: &str, why: &str) -> PlanRefusal {
    PlanRefusal::at(
        CauseCode::MalformedAdmit,
        stage_id,
        format!(
            "`admit={raw}` is not one of `always`, `<stage-id>.conclusion==<value>`, \
             or `<stage-id>.conclusion!=<value>` — {why}"
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn always_parses() {
        assert_eq!(parse_admit("always", "l2"), Ok(Admit::Always));
    }

    #[test]
    fn the_exemplar_condition_parses() {
        // design §9.1, stage l3
        assert_eq!(
            parse_admit("l1.conclusion!=measured-red", "l3"),
            Ok(Admit::Conclusion {
                stage_id: "l1".to_owned(),
                comparison: Comparison::NotEquals,
                value: "measured-red".to_owned(),
            })
        );
    }

    #[test]
    fn equals_parses() {
        assert_eq!(
            parse_admit("l1.conclusion==design-ready", "l2"),
            Ok(Admit::Conclusion {
                stage_id: "l1".to_owned(),
                comparison: Comparison::Equals,
                value: "design-ready".to_owned(),
            })
        );
    }

    #[test]
    fn canonical_round_trips() {
        for raw in [
            "always",
            "l1.conclusion==design-ready",
            "l1.conclusion!=measured-red",
        ] {
            let parsed = parse_admit(raw, "s");
            assert_eq!(
                parsed.map(|a| a.canonical()),
                Ok(raw.to_owned()),
                "canonical form of {raw:?} did not round-trip"
            );
        }
    }

    #[test]
    fn prose_is_refused() {
        for raw in [
            "if l1 concluded measured red",
            "l1.conclusion",
            "l1.verdict==ready",
            ".conclusion==x",
            "l1.conclusion==",
            "l1.conclusion==a==b",
            "l1.extra.conclusion==x",
        ] {
            let got = parse_admit(raw, "l3");
            assert!(
                got.is_err(),
                "free-form condition {raw:?} was ACCEPTED; design §4 makes report text \
                 never a branch condition"
            );
            if let Err(refusal) = got {
                assert_eq!(refusal.cause_code, CauseCode::MalformedAdmit);
            }
        }
    }
}
