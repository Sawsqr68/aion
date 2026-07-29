//! `$` references are the one piece of a `run` command AWL interprets;
//! these pin exactly what it finds, what it refuses, and where. The
//! agreement with the worker SDK's executor is pinned separately, by the
//! differential test in `tests/action_body_executor_parity.rs`.

use super::command::{CommandReference, ScannedCommand, scan_command};

fn reference(name: &str, offset: usize, len: usize) -> CommandReference {
    CommandReference {
        name: name.to_owned(),
        offset,
        len,
    }
}

fn scanned(command: &str) -> ScannedCommand {
    match scan_command(command) {
        Ok(scanned) => scanned,
        Err(error) => unreachable!("{command:?} must scan: {error}"),
    }
}

#[test]
fn a_command_without_a_dollar_has_no_references() {
    let found = scanned("cargo test --workspace");
    assert!(found.references.is_empty());
    assert_eq!(found.program_end, 5);
}

#[test]
fn the_empty_command_scans_to_nothing() {
    let found = scanned("");
    assert!(found.references.is_empty());
    assert_eq!(found.program_end, 0);
}

#[test]
fn a_bare_reference_reports_its_name_offset_and_length() {
    assert_eq!(
        scanned("echo hello $name!").references,
        vec![reference("name", 11, 5)]
    );
}

#[test]
fn a_reference_ending_the_command_is_measured_to_the_end() {
    assert_eq!(
        scanned("cat $path").references,
        vec![reference("path", 4, 5)]
    );
}

#[test]
fn a_name_may_carry_digits_and_underscores() {
    assert_eq!(
        scanned("cat $repo_root2").references,
        vec![reference("repo_root2", 4, 11)]
    );
}

#[test]
fn adjacent_references_are_both_found() {
    assert_eq!(
        scanned("cat $one$two").references,
        vec![reference("one", 4, 4), reference("two", 8, 4)]
    );
}

#[test]
fn braces_delimit_a_reference_that_abuts_more_name_characters() {
    assert_eq!(
        scanned("cat ${branch}_snapshot").references,
        vec![reference("branch", 4, 9)]
    );
}

#[test]
fn a_doubled_dollar_is_a_literal_dollar_and_never_a_reference() {
    assert_eq!(
        scanned("echo $$HOME is $home").references,
        vec![reference("home", 15, 5)]
    );
}

#[test]
fn a_doubled_dollar_before_a_brace_stays_literal() {
    assert!(scanned("echo $${HOME}").references.is_empty());
}

#[test]
fn a_dollar_inside_single_quotes_is_literal() {
    // Single quotes are literal throughout in the executor; the checker
    // must not claim a reference the executor will never substitute.
    assert!(scanned("echo '$name'").references.is_empty());
}

#[test]
fn a_dollar_inside_double_quotes_is_a_reference() {
    assert_eq!(
        scanned("echo \"$name\"").references,
        vec![reference("name", 6, 5)]
    );
}

#[test]
fn multibyte_text_before_a_reference_keeps_byte_offsets_honest() {
    // "héllo" is six bytes; the reference must report the byte offset,
    // which is what the checker's span arithmetic consumes.
    assert_eq!(
        scanned("echo héllo $name").references,
        vec![reference("name", 12, 5)]
    );
}

#[test]
fn the_program_word_ends_at_the_first_unquoted_whitespace() {
    assert_eq!(scanned("echo $name").program_end, 4);
    assert_eq!(scanned("  echo $name").program_end, 6);
    assert_eq!(scanned("'my prog' $name").program_end, 9);
    // No whitespace at all: the whole command is the program word.
    assert_eq!(scanned("echo").program_end, 4);
}

#[test]
fn a_reference_in_program_position_is_reported_inside_the_program_word() {
    let found = scanned("$program arg");
    assert_eq!(found.references, vec![reference("program", 0, 8)]);
    assert_eq!(found.program_end, 8);
    assert!(found.references[0].offset < found.program_end);
}

#[test]
fn parameters_are_sorted_and_deduplicated() {
    assert_eq!(
        scanned("cp $source $target $source").parameters(),
        ["source".to_owned(), "target".to_owned()]
    );
}

#[test]
fn an_uppercase_shell_variable_is_refused_with_the_escape_hint() {
    let Err(error) = scan_command("echo $HOME") else {
        unreachable!("`$HOME` must be refused");
    };
    assert_eq!(error.offset, 5);
    assert_eq!(error.len, 5);
    assert!(
        error.message.contains("$$"),
        "the refusal must teach the escape: {}",
        error.message
    );
}

#[test]
fn a_unicode_name_is_refused_rather_than_split() {
    // The executor would take the whole `naïve`; splitting it here into a
    // reference to `na` would be a silent disagreement, so it is refused.
    let Err(error) = scan_command("echo $naïve") else {
        unreachable!("`$naïve` must be refused");
    };
    assert_eq!(error.offset, 5);
    assert!(error.message.contains("naïve"), "{}", error.message);
}

#[test]
fn a_dangling_dollar_at_the_end_is_refused() {
    let Err(error) = scan_command("echo $") else {
        unreachable!("a trailing `$` must be refused");
    };
    assert_eq!(error.offset, 5);
    assert_eq!(error.len, 1);
}

#[test]
fn a_dollar_before_a_space_is_refused() {
    let Err(error) = scan_command("echo $ 5") else {
        unreachable!("`$ ` must be refused");
    };
    assert_eq!(error.offset, 5);
}

#[test]
fn a_dollar_before_a_hyphen_is_refused() {
    let Err(error) = scan_command("echo $-x") else {
        unreachable!("`$-` must be refused");
    };
    assert_eq!(error.offset, 5);
}

#[test]
fn an_unterminated_brace_reference_is_refused() {
    let Err(error) = scan_command("echo ${name") else {
        unreachable!("`${{name` must be refused");
    };
    assert_eq!(error.offset, 5);
    assert!(error.message.contains('}'), "{}", error.message);
}

#[test]
fn an_empty_brace_reference_is_refused() {
    let Err(error) = scan_command("echo ${}") else {
        unreachable!("`${{}}` must be refused");
    };
    assert_eq!(error.offset, 5);
}

#[test]
fn a_brace_reference_holding_a_non_name_is_refused() {
    let Err(error) = scan_command("echo ${NAME}") else {
        unreachable!("`${{NAME}}` must be refused");
    };
    assert_eq!(error.offset, 5);
}

#[test]
fn a_brace_reference_holding_spaces_is_refused() {
    let Err(error) = scan_command("echo ${a b}") else {
        unreachable!("`${{a b}}` must be refused");
    };
    assert_eq!(error.offset, 5);
}

#[test]
fn an_unterminated_single_quote_is_refused() {
    let Err(error) = scan_command("echo 'open") else {
        unreachable!("an unclosed `'` must be refused");
    };
    assert_eq!(error.offset, 5);
    assert!(error.message.contains("unterminated"), "{}", error.message);
}

#[test]
fn an_unterminated_double_quote_is_refused() {
    let Err(error) = scan_command("echo \"open") else {
        unreachable!("an unclosed `\"` must be refused");
    };
    assert_eq!(error.offset, 5);
    assert!(error.message.contains("unterminated"), "{}", error.message);
}
