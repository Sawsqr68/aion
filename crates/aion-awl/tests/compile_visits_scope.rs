//! Compiler-path parity for the guard-only `visits` binding.

use std::error::Error;
use std::path::Path;

use aion_awl::{CompileError, compile};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn visits_route_payload_is_refused_by_the_shared_check_before_lowering() -> TestResult {
    let source = "//! Payload scope.\n\
                  workflow t\n\
                  \x20 outcome done: type Report, route success\n\
                  \n\
                  type Report { rounds: Int }\n\
                  \n\
                  step tick\n\
                  \x20 sleep 1s\n\
                  \x20 outcome over: when visits >= 3, route done(rounds: visits)\n\
                  \x20 outcome again: otherwise, route tick\n\
                  \x20 max 3 visits\n";

    match compile(source, Path::new(".")) {
        Err(CompileError::Check(errors)) => {
            assert_eq!(errors.len(), 1, "unexpected diagnostics: {errors:?}");
            let error = &errors[0];
            assert_eq!((error.span.line, error.span.column), (9, 54));
            assert!(
                error
                    .message
                    .contains("`visits` is the builtin visit counter")
                    && error
                        .message
                        .contains("readable only in the outcome guards"),
                "the shared refusal must teach the guard-only rule: {}",
                error.message
            );
        }
        other => {
            return Err(format!(
                "expected the shared check refusal before lowering, got {other:?}"
            )
            .into());
        }
    }
    Ok(())
}
