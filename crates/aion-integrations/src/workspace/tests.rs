//! What a per-run working directory does with each shape of input a job can carry.
//!
//! Every refusal here is paired with the acceptance it must not swallow. A resolver that
//! simply refused everything would satisfy the four refusal tests on its own, so each of
//! them stands beside a case that MUST resolve — and the fixtures differ only in the field
//! under test, so a passing acceptance and a passing refusal cannot both be explained by
//! the resolver ignoring the input.

use aion_core::{ContentType, Payload};
use serde_json::json;

use super::HarnessWorkspace;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// An input carrying whatever the caller states, as a job's payload really arrives.
fn input(value: &serde_json::Value) -> Result<Payload, Box<dyn std::error::Error>> {
    Ok(Payload::from_json(value)?)
}

/// The acceptance every refusal below is measured against: the parameter is present, a
/// string, and non-blank, and the resolver hands back exactly what the job wrote.
#[test]
fn a_job_that_names_its_tree_resolves_to_that_tree() -> TestResult {
    let workspace = HarnessWorkspace::PerRun("repo".to_owned());
    let resolved = workspace.for_attempt(&input(&json!({ "repo": "/srv/lane-3" }))?)?;
    assert_eq!(resolved, std::path::PathBuf::from("/srv/lane-3"));
    Ok(())
}

/// The fixed form does not consult the input at all — and the control is an input that
/// carries a DIFFERENT directory under the same name, so a resolver that had quietly
/// preferred the job's value would answer `/srv/other` and fail here.
#[test]
fn a_fixed_tree_ignores_what_the_job_carries() -> TestResult {
    let workspace = HarnessWorkspace::Fixed(std::path::PathBuf::from("/srv/one-tree"));
    let resolved = workspace.for_attempt(&input(&json!({ "repo": "/srv/other" }))?)?;
    assert_eq!(resolved, std::path::PathBuf::from("/srv/one-tree"));
    Ok(())
}

/// The case the whole type exists for: a job that never said where to work does not start
/// an agent anywhere. The refusal names the parameter so the workflow that dispatched it
/// can be corrected.
#[test]
fn a_job_missing_the_parameter_is_refused_and_never_defaulted() -> TestResult {
    let workspace = HarnessWorkspace::PerRun("repo".to_owned());
    let error = workspace
        .for_attempt(&input(&json!({ "branch": "main" }))?)
        .err()
        .ok_or("a job that does not name its tree cannot launch an agent")?;
    assert!(
        error.to_string().contains("repo"),
        "the refusal names the parameter the job omitted: {error}"
    );
    assert!(
        error.is_deterministic(),
        "the same input meets the same wall, so retrying spends an attempt to learn \
         nothing: {error}"
    );
    Ok(())
}

/// An empty string is not "the current directory" — it is a job that did not answer. The
/// twin above proves a real path through the same code path, so this is the blank case and
/// not the resolver refusing every string.
#[test]
fn a_blank_directory_is_refused_rather_than_read_as_here() -> TestResult {
    let workspace = HarnessWorkspace::PerRun("repo".to_owned());
    let error = workspace
        .for_attempt(&input(&json!({ "repo": "   " }))?)
        .err()
        .ok_or("a blank directory is a job that did not say where to work")?;
    assert!(
        error.is_deterministic(),
        "a blank value is deterministic: {error}"
    );
    Ok(())
}

/// A parameter of the wrong type is named for what it actually is, so the operator reading
/// the failure can see the shape mismatch rather than guess at it.
#[test]
fn a_parameter_that_is_not_a_string_says_what_it_was() -> TestResult {
    let workspace = HarnessWorkspace::PerRun("repo".to_owned());
    let error = workspace
        .for_attempt(&input(&json!({ "repo": 7 }))?)
        .err()
        .ok_or("a number is not a path")?;
    assert!(
        error.to_string().contains("a number"),
        "the refusal says what arrived instead of a path: {error}"
    );
    Ok(())
}

/// An input that is not an object carries no parameters at all, and says so — rather than
/// reporting the parameter as merely absent, which would send the reader looking for a
/// field in a document that has no fields.
#[test]
fn an_input_that_is_not_an_object_says_it_carries_no_parameters() -> TestResult {
    let workspace = HarnessWorkspace::PerRun("repo".to_owned());
    let error = workspace
        .for_attempt(&input(&json!(["/srv/lane-3"]))?)
        .err()
        .ok_or("a list carries no named parameters")?;
    assert!(
        error.to_string().contains("carries no parameters"),
        "the refusal distinguishes a shapeless input from a missing field: {error}"
    );
    Ok(())
}

/// Bytes that are not JSON at all reach the same terminal refusal rather than a panic or a
/// silent fall-through to the launching process's directory.
#[test]
fn input_that_is_not_json_is_refused_terminally() -> TestResult {
    let workspace = HarnessWorkspace::PerRun("repo".to_owned());
    let payload = Payload::new(ContentType::Json, b"not json".to_vec());
    let error = workspace
        .for_attempt(&payload)
        .err()
        .ok_or("undecodable bytes cannot name a directory")?;
    assert!(
        error.is_deterministic(),
        "undecodable input is deterministic: {error}"
    );
    Ok(())
}

/// The two accessors answer for their own form and `None` for the other, so a caller that
/// can only report a fixed directory cannot mistake a per-run workspace for one.
#[test]
fn each_form_answers_only_for_itself() {
    let fixed = HarnessWorkspace::Fixed(std::path::PathBuf::from("/srv/one-tree"));
    let per_run = HarnessWorkspace::PerRun("repo".to_owned());
    assert_eq!(fixed.fixed(), Some(std::path::Path::new("/srv/one-tree")));
    assert_eq!(fixed.per_run(), None);
    assert_eq!(per_run.per_run(), Some("repo"));
    assert_eq!(per_run.fixed(), None);
}

// ---------------------------------------------------------------------------------------
// The prompt: what the agent is actually asked, read out of the same input.
//
// The same pairing discipline as above. Every refusal stands beside an acceptance that
// differs only in the field under test, so neither can be explained by a resolver that
// ignored the input — and the first test below is the one that would have caught the live
// defect: an authored action's input is an OBJECT even when it declares one parameter, so
// an adapter that passed it through sent the agent `{"prompt":"…"}` as its instructions.

/// One parameter, one object field, and the agent is asked what the caller wrote — NOT the
/// JSON text that carried it.
#[test]
fn a_one_field_object_asks_the_agent_the_field_and_not_the_json() -> TestResult {
    let workspace = HarnessWorkspace::Fixed(std::path::PathBuf::from("/srv/one-tree"));
    let prompt = workspace.prompt_for_attempt(&input(&json!({ "prompt": "say hi" }))?)?;
    assert_eq!(prompt, "say hi");
    Ok(())
}

/// The bound form: the directory is subtracted and what remains is the prompt. The control
/// is that the SAME input resolves the directory too, so one input serves both reads and a
/// resolver that answered one of them from the wrong field would disagree here.
#[test]
fn a_bound_directory_is_subtracted_and_what_remains_is_the_prompt() -> TestResult {
    let workspace = HarnessWorkspace::PerRun("repo".to_owned());
    let carried = input(&json!({ "repo": "/srv/lane-3", "prompt": "review the diff" }))?;
    assert_eq!(
        workspace.for_attempt(&carried)?,
        std::path::PathBuf::from("/srv/lane-3")
    );
    assert_eq!(workspace.prompt_for_attempt(&carried)?, "review the diff");
    Ok(())
}

/// The field is read by NAME, not by position: an object whose directory is written LAST
/// must still leave the prompt behind. Serialized JSON has no order to rely on, and this
/// is the fixture that distinguishes a by-name read from a take-the-first-field one.
#[test]
fn the_prompt_is_found_wherever_the_directory_sits_in_the_object() -> TestResult {
    let workspace = HarnessWorkspace::PerRun("repo".to_owned());
    let prompt =
        workspace.prompt_for_attempt(&input(&json!({ "prompt": "go", "repo": "/srv/x" }))?)?;
    assert_eq!(prompt, "go");
    Ok(())
}

/// A caller that passes a bare JSON string — the shape a hand-written worker produces —
/// is asked exactly that text, escapes and newlines resolved rather than carried.
#[test]
fn a_json_string_input_is_the_prompt_verbatim() -> TestResult {
    let workspace = HarnessWorkspace::Fixed(std::path::PathBuf::from("/srv/one-tree"));
    let encoded = serde_json::to_vec(&json!("line one\nsay \"hi\"\nline three"))?;
    let payload = Payload::new(ContentType::Json, encoded);
    assert_eq!(
        workspace.prompt_for_attempt(&payload)?,
        "line one\nsay \"hi\"\nline three"
    );
    Ok(())
}

/// JSON-tagged bytes that are not JSON keep the pass-through: `Payload` does not validate
/// on construction, so this is a real shape and refusing it would refuse a working caller.
#[test]
fn json_tagged_text_that_is_not_json_passes_through_as_the_prompt() -> TestResult {
    let workspace = HarnessWorkspace::Fixed(std::path::PathBuf::from("/srv/one-tree"));
    let payload = Payload::new(ContentType::Json, b"plain prompt, not json".to_vec());
    assert_eq!(
        workspace.prompt_for_attempt(&payload)?,
        "plain prompt, not json"
    );
    Ok(())
}

/// An object carrying more than one thing to ask with is refused rather than guessed at,
/// and the refusal names the fields so the caller can see which it left in.
#[test]
fn an_object_carrying_two_possible_prompts_is_refused_by_name() -> TestResult {
    let workspace = HarnessWorkspace::PerRun("repo".to_owned());
    let carried = input(&json!({ "repo": "/srv/x", "prompt": "go", "note": "also go" }))?;
    let error = workspace
        .prompt_for_attempt(&carried)
        .err()
        .ok_or("two candidate prompts is a choice no adapter may make")?;
    assert!(
        error.to_string().contains("prompt") && error.to_string().contains("note"),
        "the refusal names the fields it found: {error}"
    );
    assert!(
        error.is_deterministic(),
        "the same input meets the same wall on every attempt: {error}"
    );
    Ok(())
}

/// An object carrying ONLY the directory has nothing to ask the agent, and is refused for
/// that rather than sending it an empty turn.
#[test]
fn an_object_carrying_only_the_directory_is_refused() -> TestResult {
    let workspace = HarnessWorkspace::PerRun("repo".to_owned());
    let error = workspace
        .prompt_for_attempt(&input(&json!({ "repo": "/srv/x" }))?)
        .err()
        .ok_or("a job with no prompt asks the agent nothing")?;
    assert!(
        error.is_deterministic(),
        "a missing prompt is missing on every attempt: {error}"
    );
    Ok(())
}

/// A prompt field that is not text is refused naming what arrived — an agent is asked in
/// words, and re-serializing a structure into words is a projection nobody wrote.
#[test]
fn a_prompt_field_that_is_not_text_is_refused_naming_what_arrived() -> TestResult {
    let workspace = HarnessWorkspace::Fixed(std::path::PathBuf::from("/srv/one-tree"));
    let error = workspace
        .prompt_for_attempt(&input(&json!({ "prompt": { "task": "build" } }))?)
        .err()
        .ok_or("a structured prompt is not words")?;
    assert!(
        error.to_string().contains("an object"),
        "the refusal says what arrived instead of text: {error}"
    );
    Ok(())
}

/// Non-UTF-8 bytes are a protocol mismatch, not a configuration fault: nothing an author
/// wrote can be corrected to make arbitrary bytes into a turn.
#[test]
fn non_utf8_input_is_refused_as_a_protocol_fault() -> TestResult {
    let workspace = HarnessWorkspace::Fixed(std::path::PathBuf::from("/srv/one-tree"));
    let payload = Payload::new(ContentType::Json, vec![0xff, 0xfe, 0xfd]);
    let error = workspace
        .prompt_for_attempt(&payload)
        .err()
        .ok_or("non-UTF-8 input carries no prompt")?;
    assert!(
        error.to_string().contains("not valid UTF-8"),
        "the refusal names the UTF-8 mismatch: {error}"
    );
    Ok(())
}
