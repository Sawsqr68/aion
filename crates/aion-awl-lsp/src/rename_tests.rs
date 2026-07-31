use lsp_types::{Position, Uri};

use super::{RenameRefusal, prepare, references, rename};

const DOCUMENT: &str = "\
//! Rename fixture.
workflow triage
  input incident_id: String
  outcome resolved: type Resolved, route success

type Resolved { incident_id: String }

worker ops
  action gather(incident_id: String) -> Resolved

step gather
  gather(incident_id: incident_id) -> report

step finish
  outcome ok: otherwise, route resolved(incident_id: incident_id)
";

/// The zero-based position of the `index`-th occurrence of `needle`.
fn position_of(needle: &str, index: usize) -> Position {
    let byte = DOCUMENT.match_indices(needle).nth(index).map_or_else(
        || unreachable!("fixture must contain `{needle}` #{index}"),
        |(at, _)| at,
    );
    crate::analysis::position_at(DOCUMENT, byte)
}

/// The position `skip` bytes into the `index`-th occurrence of `needle`, for
/// pointing at a NAME that follows a keyword rather than at the keyword.
fn position_within(needle: &str, index: usize, skip: usize) -> Position {
    let byte = DOCUMENT.match_indices(needle).nth(index).map_or_else(
        || unreachable!("fixture must contain `{needle}` #{index}"),
        |(at, _)| at + skip,
    );
    crate::analysis::position_at(DOCUMENT, byte)
}

fn uri() -> Uri {
    "file:///tmp/rename.awl"
        .parse()
        .unwrap_or_else(|_| unreachable!("the fixture URI must parse"))
}

fn renamed(position: Position, proposed: &str) -> Result<String, RenameRefusal> {
    let mut edits = rename(DOCUMENT, position, proposed, None)?;
    // Apply back-to-front so earlier edits do not shift later offsets.
    edits.sort_by_key(|edit| (edit.range.start.line, edit.range.start.character));
    let mut text = DOCUMENT.to_owned();
    for edit in edits.iter().rev() {
        let start = crate::analysis::byte_offset_at(&text, edit.range.start);
        let end = crate::analysis::byte_offset_at(&text, edit.range.end);
        text.replace_range(start..end, &edit.new_text);
    }
    Ok(text)
}

/// A rename edits the declaration AND every use, and the result still checks.
///
/// "Still checks" is the real assertion. A rename that produced a document the
/// checker rejects would have broken the thing it was asked to preserve, and
/// counting edits would not have noticed.
#[test]
fn renaming_an_input_rewrites_every_use_and_leaves_a_checking_document() {
    let renamed = renamed(position_of("incident_id", 0), "case_id")
        .unwrap_or_else(|refusal| unreachable!("rename refused: {refusal}"));

    assert!(
        renamed.contains("input case_id: String"),
        "the declaration itself must be rewritten:\n{renamed}"
    );
    assert!(
        renamed.contains("gather(incident_id: case_id)"),
        "the USE must be rewritten while the action's PARAMETER name, a different declaration, \
         must not:\n{renamed}"
    );

    let document = aion_awl::parse(&renamed)
        .unwrap_or_else(|error| unreachable!("renamed document must parse: {error}"));
    assert_eq!(
        aion_awl::check(&document),
        Vec::new(),
        "a rename must leave a document that still checks:\n{renamed}"
    );
}

/// NOT A TEXT SEARCH. The same word naming a different declaration is left
/// alone.
///
/// `incident_id` names three separate things here — a workflow input, an
/// action parameter, and a type field. A rename driven by matching text would
/// rewrite all three and silently produce nonsense.
#[test]
fn a_different_declaration_sharing_the_name_is_untouched() {
    let renamed = renamed(position_of("incident_id", 0), "case_id")
        .unwrap_or_else(|refusal| unreachable!("rename refused: {refusal}"));
    assert!(
        renamed.contains("type Resolved { incident_id: String }"),
        "the type's FIELD is a different declaration and must survive:\n{renamed}"
    );
    assert!(
        renamed.contains("action gather(incident_id: String)"),
        "the action's PARAMETER is a different declaration and must survive:\n{renamed}"
    );
}

/// A type is renamed in `TitleCase`, and its uses follow.
#[test]
fn renaming_a_type_rewrites_its_uses() {
    let renamed = renamed(position_of("Resolved", 0), "Settled")
        .unwrap_or_else(|refusal| unreachable!("rename refused: {refusal}"));
    assert!(
        renamed.contains("type Settled {") && renamed.contains("-> Settled"),
        "the type declaration and its use must both move:\n{renamed}"
    );
    assert!(
        !renamed.contains("Resolved {"),
        "no occurrence of the old type name may survive:\n{renamed}"
    );
}

/// REFUSAL. A `snake_case` name cannot become `TitleCase`, and vice versa —
/// checked by lexing the proposal, so the rule comes from the language.
#[test]
fn a_name_of_the_wrong_case_class_is_refused() {
    let refusal = rename(DOCUMENT, position_of("incident_id", 0), "CaseId", None)
        .err()
        .unwrap_or_else(|| unreachable!("a TitleCase name must not be accepted for an input"));
    assert!(
        matches!(refusal, RenameRefusal::WrongShape { .. }),
        "expected a shape refusal, got {refusal:?}"
    );
    assert!(
        refusal.to_string().contains("snake_case"),
        "the refusal must tell the author what the language wants: {refusal}"
    );

    let refusal = rename(DOCUMENT, position_of("Resolved", 0), "settled", None)
        .err()
        .unwrap_or_else(|| unreachable!("a snake_case name must not be accepted for a type"));
    assert!(
        refusal.to_string().contains("TitleCase"),
        "the refusal must name the type case class: {refusal}"
    );
}

/// REFUSAL. A reserved keyword is not a name, and accepting one would change
/// what the document means rather than what it calls things.
#[test]
fn a_reserved_keyword_is_refused() {
    for word in ["step", "outcome", "route", "workflow"] {
        let refusal = rename(DOCUMENT, position_of("incident_id", 0), word, None)
            .err()
            .unwrap_or_else(|| unreachable!("`{word}` is reserved and must not be accepted"));
        assert!(
            matches!(refusal, RenameRefusal::WrongShape { .. }),
            "`{word}` must be refused as a shape error, got {refusal:?}"
        );
    }
}

/// REFUSAL. Renaming onto an existing name of the same category would merge
/// two different things, and the author would be told it worked.
#[test]
fn a_collision_with_the_same_kind_is_refused() {
    let refusal = rename(
        DOCUMENT,
        position_within("step gather", 0, "step ".len()),
        "finish",
        None,
    )
    .err()
    .unwrap_or_else(|| unreachable!("renaming a step onto another step must be refused"));
    assert!(
        matches!(refusal, RenameRefusal::Collides { .. }),
        "expected a collision refusal, got {refusal:?}"
    );
    assert!(
        refusal.to_string().contains("merge two different things"),
        "the refusal must say WHY it is dangerous, not just that it is: {refusal}"
    );
}

/// NON-VACUITY. The refusals above are not a provider that refuses everything:
/// a legal rename of the same declaration still succeeds.
#[test]
fn the_refusals_are_not_a_provider_that_refuses_everything() {
    assert!(
        rename(
            DOCUMENT,
            position_within("step gather", 0, "step ".len()),
            "collect_data",
            None
        )
        .is_ok(),
        "a legal step rename must still be accepted, or every refusal above proves nothing"
    );
}

/// Nothing at the cursor, nothing to rename — and no edits invented.
#[test]
fn a_position_with_no_declaration_is_refused() {
    let comment = crate::analysis::position_at(DOCUMENT, 4);
    assert_eq!(
        rename(DOCUMENT, comment, "anything", None),
        Err(RenameRefusal::NotRenameable)
    );
    assert_eq!(prepare(DOCUMENT, comment, None), None);
}

/// `prepareRename` points at the declaration's own name, so the editor
/// pre-fills the prompt with the right text.
#[test]
fn prepare_selects_the_declaration_name() {
    let range = prepare(DOCUMENT, position_of("incident_id", 0), None)
        .unwrap_or_else(|| unreachable!("an input's name must be renameable"));
    let start = crate::analysis::byte_offset_at(DOCUMENT, range.start);
    let end = crate::analysis::byte_offset_at(DOCUMENT, range.end);
    assert_eq!(&DOCUMENT[start..end], "incident_id");
}

/// References find every use, and honour `includeDeclaration`.
#[test]
fn references_find_every_use_and_honour_the_declaration_flag() {
    let with = references(DOCUMENT, &uri(), position_of("incident_id", 0), None, true)
        .unwrap_or_else(|| unreachable!("an input must have references"));
    let without = references(DOCUMENT, &uri(), position_of("incident_id", 0), None, false)
        .unwrap_or_else(|| unreachable!("an input must have references"));
    assert!(
        with.len() > without.len(),
        "excluding the declaration must return strictly fewer locations: {} vs {}",
        with.len(),
        without.len()
    );
    assert_eq!(
        with.len(),
        without.len() + 1,
        "exactly one location — the declaration — separates the two"
    );
    assert!(
        without.len() >= 2,
        "the input is used more than once in the fixture, so a single result would mean the \
         references are not being collected: {without:?}"
    );
}
