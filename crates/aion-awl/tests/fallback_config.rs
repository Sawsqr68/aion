//! Authored activity fallback syntax, validation, printing, and reference lowering.

use std::error::Error;

use aion_awl::{check, emit, parse, print};

type TestResult = Result<(), Box<dyn Error>>;

const SOURCE: &str = "\
//! Route policy refusals through authored queues.
workflow provider_route
  input prompt: String
  outcome done: type Answer, route success

type Answer { text: String }

worker agents
  action run(prompt: String) -> Answer
    retry 3 every 1s, fallback norn, claude

step run
  run(prompt: prompt) -> answer
  route done(answer)
";

#[test]
fn fallback_round_trips_and_lowers_after_retry() -> TestResult {
    let document = parse(SOURCE)?;
    let errors = check(&document);
    assert!(errors.is_empty(), "{errors:#?}");
    assert_eq!(print(&document), SOURCE);
    assert_eq!(print(&parse(&print(&document))?), SOURCE);

    let generated = emit(&document)?;
    let retry = generated
        .find("|> activity.retry(")
        .ok_or("missing retry")?;
    let fallback = generated
        .find("|> activity.fallback([\"norn\", \"claude\"])")
        .ok_or("missing fallback")?;
    assert!(
        retry < fallback,
        "fallback must lower after retry:\n{generated}"
    );
    Ok(())
}

#[test]
fn fallback_then_retry_is_two_config_keys() -> TestResult {
    let source = SOURCE.replace(
        "retry 3 every 1s, fallback norn, claude",
        "fallback norn, retry 3 every 1s",
    );
    let document = parse(&source)?;
    let config = document.workers[0].actions[0]
        .config
        .as_ref()
        .ok_or("missing config")?;
    assert_eq!(
        config
            .fallback
            .as_ref()
            .ok_or("missing fallback")?
            .entries
            .len(),
        1
    );
    assert!(config.retry.is_some());
    Ok(())
}

#[test]
fn duplicate_fallback_queue_is_anchored_on_the_second_occurrence() -> TestResult {
    let source = SOURCE.replace("fallback norn, claude", "fallback norn, norn");
    let document = parse(&source)?;
    let errors = check(&document);
    let error = errors
        .iter()
        .find(|error| {
            error
                .message
                .contains("duplicate fallback task queue `norn`")
        })
        .ok_or("missing duplicate diagnostic")?;
    assert_eq!(&source[error.span.start..error.span.end], "norn");
    Ok(())
}

#[test]
fn fallback_is_declaration_only() -> TestResult {
    let source = SOURCE.replace(
        "  run(prompt: prompt) -> answer\n",
        "  run(prompt: prompt) -> answer\n    fallback other\n",
    );
    let document = parse(&source)?;
    assert!(check(&document).iter().any(|error| {
        error
            .message
            .contains("`fallback` stays on the action declaration")
    }));
    Ok(())
}

#[test]
fn empty_and_soft_keyword_entries_are_parse_errors() -> TestResult {
    let empty = SOURCE.replace("fallback norn, claude", "fallback");
    let error = parse(&empty)
        .err()
        .ok_or("empty fallback unexpectedly parsed")?;
    assert!(error.message.contains("task queue"), "{}", error.message);

    let keyword = SOURCE.replace("fallback norn, claude", "fallback retry");
    let error = parse(&keyword)
        .err()
        .ok_or("soft-keyword fallback queue unexpectedly parsed")?;
    assert!(
        error
            .message
            .contains("task queue `retry` is a soft keyword"),
        "{}",
        error.message
    );
    Ok(())
}
