use std::ffi::OsString;
use std::path::PathBuf;

use super::{SKILL_RESOURCES, tail, workspace_root_from};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn workspace_root_override_wins_over_home() -> TestResult {
    let resolved = workspace_root_from(
        Some(OsString::from("/durable/clones")),
        Some(OsString::from("/home/user")),
    )?;
    assert_eq!(resolved, PathBuf::from("/durable/clones"));
    Ok(())
}

#[test]
fn workspace_root_defaults_under_home() -> TestResult {
    let resolved = workspace_root_from(None, Some(OsString::from("/home/user")))?;
    assert_eq!(resolved, PathBuf::from("/home/user/.aion/clones"));
    Ok(())
}

#[test]
fn workspace_root_empty_override_falls_back_to_home() -> TestResult {
    let resolved = workspace_root_from(Some(OsString::new()), Some(OsString::from("/home/user")))?;
    assert_eq!(resolved, PathBuf::from("/home/user/.aion/clones"));
    Ok(())
}

#[test]
fn workspace_root_without_home_or_override_is_a_hard_error() -> TestResult {
    let failure = workspace_root_from(None, None)
        .err()
        .ok_or("missing workspace-root error")?;
    let message = failure.message();
    assert!(
        message.contains("AION_WORKSPACE_ROOT") && message.contains("HOME"),
        "the error must name both the override and HOME; got: {message}"
    );
    Ok(())
}

#[test]
fn workspace_root_relative_override_is_a_hard_error() -> TestResult {
    // A relative root resolves against the worker CWD at each call site, so the
    // recorded history's path silently changes meaning after a restart from a
    // different directory — refuse it loudly.
    let failure = workspace_root_from(
        Some(OsString::from("relative/clones")),
        Some(OsString::from("/home/user")),
    )
    .err()
    .ok_or("missing relative-root error")?;
    let message = failure.message();
    assert!(
        message.contains("absolute") && message.contains("relative/clones"),
        "the error must name the offending path and demand an absolute one; got: {message}"
    );
    Ok(())
}

#[test]
fn workspace_root_relative_home_is_a_hard_error() -> TestResult {
    let failure = workspace_root_from(None, Some(OsString::from("relative-home")))
        .err()
        .ok_or("missing relative-HOME error")?;
    let message = failure.message();
    assert!(
        message.contains("absolute"),
        "a relative HOME-derived root must be refused; got: {message}"
    );
    Ok(())
}

#[test]
fn tail_keeps_the_end_of_long_output_and_short_output_whole() -> TestResult {
    let short = std::str::from_utf8(b"short")?;
    let multibyte = std::str::from_utf8("ééééé".as_bytes())?;
    assert_eq!(tail(short, 10), "short");
    assert_eq!(tail("abcdefgh", 3), "fgh");
    // Multi-byte chars truncate on a char boundary, never mid-codepoint.
    assert_eq!(tail(multibyte, 2), "éé");
    Ok(())
}

#[test]
fn skill_resources_publish_awl_reference_without_gleam_scaffold_or_sdk() -> TestResult {
    let names: Vec<&str> = SKILL_RESOURCES
        .iter()
        .map(|(name, _contents)| *name)
        .collect();
    assert!(names.contains(&"AWL-REFERENCE.md"));
    assert!(!names.contains(&"SCAFFOLD.md"));
    assert!(!names.contains(&"SDK.md"));

    let reference = SKILL_RESOURCES
        .iter()
        .find_map(|(name, contents)| (*name == "AWL-REFERENCE.md").then_some(*contents))
        .ok_or("AWL reference contents are missing")?;
    assert!(reference.contains("distribute"));
    assert!(reference.contains("worker name (`dev`) is the task queue"));
    Ok(())
}
