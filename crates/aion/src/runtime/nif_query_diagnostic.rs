//! Non-durable diagnostic tap for the workflow-side query pump.

use beamr::native::ProcessContext;
use beamr::term::Term;

use super::engine_nifs::decode_string_arg;
use crate::runtime::nif_result_term::{error_result_term, ok_result_term};

// Pump diagnostics are already bounded before crossing the FFI. These limits
// also bound direct calls to the NIF before data reaches the tracing subscriber.
const MAX_CATEGORY_BYTES: usize = 64;
const MAX_MESSAGE_BYTES: usize = 2048;

pub(super) fn report_query_diagnostic(
    args: &[Term],
    ctx: &mut ProcessContext,
) -> Result<Term, Term> {
    if args.len() != 2 {
        let message = format!(
            "report_query_diagnostic: expected 2 arguments, got {}",
            args.len()
        );
        return error_result_term(ctx, &message);
    }

    // Decode both terms before allocating a result because attached calls may
    // collect during allocation and rewrite argument terms in place.
    let category = match decode_string_arg(args[0]) {
        Ok(value) => value,
        Err(error) => {
            return error_result_term(ctx, &format!("report_query_diagnostic category: {error}"));
        }
    };
    let message = match decode_string_arg(args[1]) {
        Ok(value) => value,
        Err(error) => {
            return error_result_term(ctx, &format!("report_query_diagnostic message: {error}"));
        }
    };

    tracing::warn!(
        workflow_pid = ?ctx.pid(),
        category = bounded_text(&category, MAX_CATEGORY_BYTES),
        diagnostic = bounded_text(&message, MAX_MESSAGE_BYTES),
        "workflow query pump diagnostic"
    );

    ok_result_term(ctx, b"reported")
}

fn bounded_text(value: &str, max_bytes: usize) -> &str {
    if value.len() <= max_bytes {
        return value;
    }

    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    &value[..end]
}

#[cfg(test)]
mod tests {
    use super::{MAX_MESSAGE_BYTES, bounded_text};

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn diagnostic_bound_preserves_utf8_boundaries() -> TestResult {
        let value = format!("{}é", "a".repeat(MAX_MESSAGE_BYTES - 1));

        let bounded = bounded_text(&value, MAX_MESSAGE_BYTES);
        let validated = std::str::from_utf8(bounded.as_bytes())?;

        assert_eq!(validated.len(), MAX_MESSAGE_BYTES - 1);
        assert!(bounded.chars().all(|character| character == 'a'));
        Ok(())
    }
}
