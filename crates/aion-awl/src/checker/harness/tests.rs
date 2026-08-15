//! Targeted harness-section rules the fixture corpus cannot pin: the exact
//! kind-scoping of the required set, and the shape of a refusal.

use crate::ast::{HARNESS_KIND_ACP, HARNESS_KIND_NORN};

/// A workflow document declaring one agent queue with `settings` as its harness.
fn document(settings: &str) -> String {
    let block = settings.lines().fold(String::new(), |mut block, line| {
        block.push_str("    ");
        block.push_str(line);
        block.push('\n');
        block
    });
    format!(
        "//! Harness-section probe.\nworkflow ask\n  input question: String\n  outcome \
         answered: type Answer, route success\n\ntype Answer {{ reply: String }}\n\ntype Reply \
         {{ text: String, final_message: String, stop_reason: String, session_id: String }}\n\nworker assistants\n  harness\n{block}  \
         action ask(prompt: String) -> Reply\n    agent\n\nstep ask\n  ask(prompt: question) -> \
         reply\n  route answered(reply: reply.text)\n"
    )
}

/// Every complaint `settings` earns, as plain messages.
fn complaints(settings: &str) -> Vec<String> {
    let source = document(settings);
    match crate::parse(&source) {
        Err(error) => vec![format!("PARSE: {}", error.message)],
        Ok(parsed) => crate::check_in(&parsed, std::env::temp_dir().as_path())
            .into_iter()
            .map(|error| error.message)
            .collect(),
    }
}

/// A complete ACP section.
const ACP: &str = "kind acp\nconcurrency 4\nreconnect_initial_backoff 5s\nreconnect_max_backoff \
                   2m\nreconnect_max_attempts 100\nenv_pass \"PATH\", \
                   \"HOME\"\ncommand \"/usr/local/bin/claude-code-acp\"\ncwd \
                   \"/srv/agents\"\npermission \"deny\"\nexit_grace 10s";

/// A complete Norn section.
///
/// `cwd` is here because it is required of BOTH kinds. It is not an ACP setting that Norn
/// happens to tolerate: the directory an agent stands in decides which tree it reads, which
/// is a property of the work and not of the protocol carrying it.
const NORN: &str = "kind norn\nconcurrency 2\nreconnect_initial_backoff 5s\nreconnect_max_backoff \
                    1m\nreconnect_max_attempts 50\nenv_pass \"PATH\"\ncwd \"/srv/agents\"";

#[test]
fn an_acp_section_is_complete_without_any_norn_setting() {
    assert_eq!(
        complaints(ACP),
        Vec::<String>::new(),
        "the required set is scoped to the DECLARED kind"
    );
}

#[test]
fn a_norn_section_is_complete_without_command_permission_or_exit_grace() {
    assert_eq!(
        complaints(NORN),
        Vec::<String>::new(),
        "an ACP setting is not a Norn requirement"
    );
}

#[test]
fn every_required_acp_field_is_named_when_it_is_the_one_removed() {
    for field in [
        "kind",
        "concurrency",
        "reconnect_initial_backoff",
        "reconnect_max_backoff",
        "reconnect_max_attempts",
        "env_pass",
        "command",
        "cwd",
        "permission",
        "exit_grace",
    ] {
        let settings = ACP
            .lines()
            .filter(|line| !line.starts_with(field))
            .collect::<Vec<_>>()
            .join("\n");
        let complaints = complaints(&settings);
        assert!(
            complaints
                .iter()
                .any(|message| message.contains(&format!("declares no `{field}`"))),
            "removing `{field}` must produce a refusal naming it; got {complaints:#?}"
        );
    }
}

#[test]
fn every_required_norn_field_is_named_when_it_is_the_one_removed() {
    for field in [
        "kind",
        "concurrency",
        "reconnect_initial_backoff",
        "reconnect_max_backoff",
        "reconnect_max_attempts",
        "env_pass",
        "cwd",
    ] {
        let settings = NORN
            .lines()
            .filter(|line| !line.starts_with(field))
            .collect::<Vec<_>>()
            .join("\n");
        let complaints = complaints(&settings);
        assert!(
            complaints
                .iter()
                .any(|message| message.contains(&format!("declares no `{field}`"))),
            "removing `{field}` must produce a refusal naming it; got {complaints:#?}"
        );
    }
}

/// The removed `concurrency` is what keeps this test from passing vacuously: a section that
/// earned no refusal at all would satisfy every assertion below without proving anything
/// about which names the checker is willing to say.
///
/// `cwd` is deliberately NOT in the ACP-only list. It used to be, and it is the one setting
/// this contract moved: it is now demanded of both kinds, so a norn section that omits it IS
/// asked for it by name — pinned in
/// [`every_required_norn_field_is_named_when_it_is_the_one_removed`]. Leaving it here would
/// assert the opposite of what the checker now does.
#[test]
fn a_norn_section_is_never_asked_for_an_acp_setting() {
    let settings = NORN
        .lines()
        .filter(|line| !line.starts_with("concurrency"))
        .collect::<Vec<_>>()
        .join("\n");
    let complaints = complaints(&settings);
    assert!(
        !complaints.is_empty(),
        "the fixture must earn at least one refusal, or this proves nothing"
    );
    for acp_only in ["command", "permission", "exit_grace"] {
        assert!(
            !complaints
                .iter()
                .any(|message| message.contains(&format!("declares no `{acp_only}`"))),
            "a `{HARNESS_KIND_NORN}` section has no use for `{acp_only}`; got {complaints:#?}"
        );
    }
}

#[test]
fn an_unknown_kind_suppresses_the_per_kind_demands_rather_than_guessing_one() {
    let settings = ACP.replace("kind acp", "kind gemini");
    let complaints = complaints(&settings);
    assert!(
        complaints
            .iter()
            .any(|message| message.contains("names no agent harness")),
        "the kind itself is refused: {complaints:#?}"
    );
    assert!(
        !complaints
            .iter()
            .any(|message| message.contains("declares no `")),
        "guessing a kind to check against would report settings nobody asked for: \
         {complaints:#?}"
    );
}

#[test]
fn a_setting_for_the_other_kind_is_refused_by_name_in_both_directions() {
    let norn_with_acp = format!("{NORN}\ncommand \"/usr/local/bin/agent\"");
    assert!(
        complaints(&norn_with_acp)
            .iter()
            .any(|message| message.contains(&format!(
                "`command` configures the `{HARNESS_KIND_ACP}` harness"
            ))),
        "{:#?}",
        complaints(&norn_with_acp)
    );

    let acp_with_norn = format!("{ACP}\nbinary \"/usr/local/bin/norn\"");
    assert!(
        complaints(&acp_with_norn)
            .iter()
            .any(|message| message.contains(&format!(
                "`binary` configures the `{HARNESS_KIND_NORN}` harness"
            ))),
        "{:#?}",
        complaints(&acp_with_norn)
    );
}

#[test]
fn an_env_pass_entry_carrying_a_value_is_refused_rather_than_split() {
    let settings = ACP.replace(
        "env_pass \"PATH\", \"HOME\"",
        "env_pass \"PATH\", \"OPENAI_API_KEY=sk-live\"",
    );
    let complaints = complaints(&settings);
    assert!(
        complaints
            .iter()
            .any(|message| message.contains("takes variable NAMES")
                && message.contains("OPENAI_API_KEY=sk-live")),
        "{complaints:#?}"
    );
}

#[test]
fn a_duplicate_setting_is_refused_at_the_parse_because_neither_reading_is_honest() {
    let settings = format!("{ACP}\nconcurrency 8");
    let complaints = complaints(&settings);
    assert!(
        complaints
            .iter()
            .any(|message| message.contains("duplicate `concurrency` setting")),
        "{complaints:#?}"
    );
}

#[test]
fn an_unknown_setting_is_refused_naming_what_the_section_carries() {
    let settings = format!("{ACP}\nmodel \"opus\"");
    let complaints = complaints(&settings);
    assert!(
        complaints.iter().any(|message| {
            message.contains("`model` is not a `harness` setting") && message.contains("`command`")
        }),
        "{complaints:#?}"
    );
}
