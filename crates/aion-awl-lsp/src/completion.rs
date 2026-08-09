//! Contextual completion for AWL documents.
//!
//! WHY THIS DOES NOT PARSE. Every other feature in this crate opens with
//! `aion_awl::parse(source).ok()?`, and for hover or go-to-definition that is
//! right — mid-edit there is nothing to hover. **Completion is the one feature
//! where it would be fatal.** Completion fires precisely when the document is
//! half-written: the author has typed `wait a` and wants the signal names. At
//! that instant the document does not parse, so a provider written the same
//! way would return nothing in exactly the case it exists for — correct in
//! every visible respect and useless in practice.
//!
//! So candidates are harvested from the TOKEN stream, which accepts every
//! half-written form the parser rejects, and from a repaired copy of the source
//! if even that fails. When nothing can be read at all the contextual keyword
//! set is still returned, because a keyword list needs no document.

use std::collections::BTreeSet;

use aion_awl::{
    ANSWER_WORD, IN_WORD, Keyword, POSITIONAL_WORDS, QUERY_WORD, RUN_WORD, SEND_WORD, TO_WORD,
    Token, TokenKind,
};
use lsp_types::{CompletionItem, CompletionItemKind, Position};

use crate::analysis::byte_offset_at;

/// The built-in leaf types an author can write without declaring anything.
const BUILTIN_TYPES: [&str; 4] = ["String", "Int", "Float", "Bool"];

/// What the cursor is positioned to receive.
///
/// Decided from the text before the cursor, never from a syntax tree — see the
/// module note.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Context {
    /// Column zero: the document's own declarations.
    TopLevel,
    /// Indented under `workflow`: the header's clauses.
    WorkflowHeader,
    /// Indented under `worker`: its actions.
    WorkerBlock,
    /// Indented under `step` or `subflow`: a statement may start here.
    StepBody,
    /// Indented under `type`: a field declaration.
    TypeBody,
    /// After `route `: where control goes.
    RouteTarget,
    /// After `after `: an ordering dependency on another step.
    StepReference,
    /// After `wait ` or `send `: a signal.
    SignalReference,
    /// After `|>`: a pipe stage.
    PipeStage,
    /// After `:`: a type.
    TypeReference,
    /// After `when `: a condition.
    Condition,
}

/// A name the document declares, and what kind of thing it names.
struct Declared {
    name: String,
    kind: CompletionItemKind,
    detail: &'static str,
}

/// Everything the document declares, by role.
#[derive(Default)]
struct Harvest {
    /// Callables in a step body: worker actions and child workflows.
    callables: Vec<Declared>,
    /// Signal names, for `wait` and `send`.
    signals: Vec<Declared>,
    /// Step and subflow names, for `after` and `route`.
    steps: Vec<Declared>,
    /// Outcome names, for `route`.
    outcomes: Vec<Declared>,
    /// Declared type names.
    types: Vec<Declared>,
    /// Values in scope: inputs, consts, and the bindings statements produce.
    values: Vec<Declared>,
}

/// Completion items for `position` in `source`.
///
/// Never fails and never returns `None`: an unreadable document still has a
/// vocabulary, and offering it is strictly better than offering nothing.
#[must_use]
pub(crate) fn completions(source: &str, position: Position) -> Vec<CompletionItem> {
    let cursor = byte_offset_at(source, position);
    let line = line_bounds(source, cursor);
    let context = context_at(source, cursor, line);
    let harvest = harvest(source, line);
    build(context, &harvest)
}

/// The byte range of the line containing `cursor`, excluding its newline.
fn line_bounds(source: &str, cursor: usize) -> (usize, usize) {
    let cursor = cursor.min(source.len());
    let start = source[..cursor].rfind('\n').map_or(0, |index| index + 1);
    let end = source[cursor..]
        .find('\n')
        .map_or(source.len(), |index| cursor + index);
    (start, end)
}

/// The trailing word the author is part-way through typing, which is a prefix
/// to filter by and never a token that decides context.
fn split_partial(prefix: &str) -> &str {
    let boundary = prefix
        .rfind(|ch: char| !(ch.is_alphanumeric() || ch == '_'))
        .map_or(0, |index| index + ch_len(prefix, index));
    &prefix[..boundary]
}

fn ch_len(text: &str, index: usize) -> usize {
    text[index..].chars().next().map_or(1, char::len_utf8)
}

/// Decide the context from the text before the cursor.
fn context_at(source: &str, cursor: usize, line: (usize, usize)) -> Context {
    let (line_start, _) = line;
    let prefix = &source[line_start..cursor.min(source.len())];
    let head = split_partial(prefix).trim_end();

    // An immediate cue beats the enclosing block: `route ` inside a step body
    // wants outcomes, not another statement opener.
    if head.ends_with("|>") {
        return Context::PipeStage;
    }
    if head.ends_with(':') {
        return Context::TypeReference;
    }
    match last_word(head) {
        Some(word) if word == Keyword::Route.as_word() => return Context::RouteTarget,
        Some(word) if word == Keyword::After.as_word() => return Context::StepReference,
        Some(word) if word == Keyword::Wait.as_word() || word == SEND_WORD => {
            return Context::SignalReference;
        }
        Some(word) if word == Keyword::When.as_word() => return Context::Condition,
        _ => {}
    }

    if prefix.trim_start().len() == prefix.len() {
        // Nothing indents this line, so it opens a top-level declaration.
        return Context::TopLevel;
    }
    match enclosing_declaration(source, line_start) {
        Some(word) if word == Keyword::Worker.as_word() => Context::WorkerBlock,
        Some(word) if word == Keyword::Type.as_word() => Context::TypeBody,
        Some(word) if word == Keyword::Workflow.as_word() || word == Keyword::Child.as_word() => {
            Context::WorkflowHeader
        }
        _ => Context::StepBody,
    }
}

/// The last whole word in `head`, or `None` when it ends in punctuation.
fn last_word(head: &str) -> Option<&str> {
    let word = head
        .rsplit(|ch: char| !(ch.is_alphanumeric() || ch == '_'))
        .next()?;
    (!word.is_empty()).then_some(word)
}

/// The first word of the nearest unindented line above `line_start`, which is
/// the declaration the cursor's line belongs to.
fn enclosing_declaration(source: &str, line_start: usize) -> Option<&str> {
    source[..line_start]
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty() && !line.starts_with(char::is_whitespace))
        .and_then(|line| line.split_whitespace().next())
}

/// Read every name the document declares out of its token stream.
///
/// Lexing is attempted on the source as written, then on a copy with the
/// cursor's line blanked — a half-typed line is the overwhelmingly common
/// reason a document in an editor will not lex, and blanking it recovers every
/// declaration around it. The blanked copy keeps the original length, so no
/// offset shifts.
fn harvest(source: &str, line: (usize, usize)) -> Harvest {
    let tokens = aion_awl::lex(source).ok().or_else(|| {
        let (start, end) = line;
        let mut repaired = source.to_owned();
        repaired.replace_range(start..end, &" ".repeat(end.saturating_sub(start)));
        aion_awl::lex(&repaired).ok()
    });
    tokens.as_deref().map_or_else(Harvest::default, collect)
}

/// Walk `<opener> <name>` pairs and `-> <binding>` out of the token stream.
fn collect(tokens: &[Token]) -> Harvest {
    let mut harvest = Harvest::default();
    let mut previous: Option<&TokenKind> = None;
    for token in tokens {
        let Some(name) = named(&token.kind) else {
            previous = Some(&token.kind);
            continue;
        };
        match previous {
            Some(TokenKind::Keyword(keyword)) => push_for_keyword(&mut harvest, *keyword, name),
            Some(TokenKind::Arrow) => harvest.values.push(Declared {
                name: name.to_owned(),
                kind: CompletionItemKind::VARIABLE,
                detail: "binding",
            }),
            Some(TokenKind::Identifier(word)) if word == QUERY_WORD => {
                harvest.callables.push(Declared {
                    name: name.to_owned(),
                    kind: CompletionItemKind::PROPERTY,
                    detail: "query",
                });
            }
            Some(TokenKind::Identifier(word)) if word == SEND_WORD => {
                harvest.signals.push(Declared {
                    name: name.to_owned(),
                    kind: CompletionItemKind::EVENT,
                    detail: "signal",
                });
            }
            _ => {}
        }
        previous = Some(&token.kind);
    }
    harvest
}

fn named(kind: &TokenKind) -> Option<&str> {
    match kind {
        TokenKind::Identifier(name) | TokenKind::TypeIdentifier(name) => Some(name),
        _ => None,
    }
}

fn push_for_keyword(harvest: &mut Harvest, keyword: Keyword, name: &str) {
    let (bucket, kind, detail): (&mut Vec<Declared>, CompletionItemKind, &'static str) =
        match keyword {
            Keyword::Action => (
                &mut harvest.callables,
                CompletionItemKind::FUNCTION,
                "action",
            ),
            Keyword::Child => (&mut harvest.callables, CompletionItemKind::CLASS, "child"),
            Keyword::Signal => (&mut harvest.signals, CompletionItemKind::EVENT, "signal"),
            Keyword::Step => (&mut harvest.steps, CompletionItemKind::METHOD, "step"),
            Keyword::Subflow => (&mut harvest.steps, CompletionItemKind::METHOD, "subflow"),
            Keyword::Outcome => (
                &mut harvest.outcomes,
                CompletionItemKind::ENUM_MEMBER,
                "outcome",
            ),
            Keyword::Type => (&mut harvest.types, CompletionItemKind::STRUCT, "type"),
            Keyword::Input => (&mut harvest.values, CompletionItemKind::VARIABLE, "input"),
            Keyword::Const => (&mut harvest.values, CompletionItemKind::CONSTANT, "const"),
            _ => return,
        };
    bucket.push(Declared {
        name: name.to_owned(),
        kind,
        detail,
    });
}

/// The keywords offered in each context.
///
/// Curated per context on purpose: offering all 57 reserved words everywhere is
/// not completion, it is a dictionary. `every_keyword_is_offered_somewhere`
/// pins that curation never drops one entirely.
fn keywords_for(context: Context) -> Vec<&'static str> {
    use Keyword as K;
    let words: Vec<K> = match context {
        Context::TopLevel => vec![
            K::Workflow,
            K::Type,
            K::Worker,
            K::Child,
            K::Const,
            K::Subflow,
            K::Step,
            K::Schema,
        ],
        Context::WorkflowHeader => vec![
            K::Input,
            K::Signal,
            K::Outcome,
            K::Timeout,
            K::Type,
            K::Route,
            K::Success,
            K::Failure,
            K::Node,
            K::Retry,
            K::Every,
            K::Backoff,
            K::Advisory,
            K::Agent,
            K::Of,
            K::Json,
        ],
        Context::WorkerBlock => vec![
            K::Action,
            K::Node,
            K::Timeout,
            K::Retry,
            K::Every,
            K::Backoff,
            K::Advisory,
            K::Agent,
        ],
        Context::StepBody => vec![
            K::Spawn,
            K::Wait,
            K::Sleep,
            K::Fork,
            K::Join,
            K::Loop,
            K::Counting,
            K::Until,
            K::Max,
            K::Visits,
            K::Sequential,
            K::Distribute,
            K::Sequence,
            K::Collect,
            K::Outcome,
            K::Route,
            K::After,
            K::On,
            K::Failure,
            K::Timeout,
            // An `outcome` line inside a step body carries its guard, so the
            // condition words start here too.
            K::When,
        ],
        Context::TypeBody => vec![K::Json, K::Schema, K::Of],
        Context::RouteTarget | Context::StepReference => vec![K::Success, K::Failure],
        Context::SignalReference => vec![K::Timeout],
        Context::PipeStage => vec![
            K::Filter,
            K::Sort,
            K::Map,
            K::Count,
            K::Any,
            K::All,
            K::Route,
        ],
        Context::TypeReference => vec![K::Schema, K::Json, K::Of],
        Context::Condition => vec![
            K::Is,
            K::Not,
            K::And,
            K::Or,
            K::Empty,
            K::Present,
            K::Absent,
            K::True,
            K::False,
            K::Otherwise,
            K::Any,
            K::All,
            K::Count,
        ],
    };
    let mut offered: Vec<&'static str> = words.into_iter().map(K::as_word).collect();
    // The positional words are not in `Keyword` — deliberately, since reserving
    // them would break real documents — but an author still has to type them.
    offered.extend(positional_for(context));
    offered
}

/// The non-reserved opener words offered in each context.
fn positional_for(context: Context) -> Vec<&'static str> {
    match context {
        Context::TopLevel | Context::WorkflowHeader => vec![QUERY_WORD, SEND_WORD],
        Context::WorkerBlock => vec![RUN_WORD],
        // `in` belongs here because `distribute <var> in <collection>` is a
        // step-body line; it is the only place an author types the word.
        Context::StepBody => vec![SEND_WORD, ANSWER_WORD, IN_WORD],
        Context::SignalReference => vec![TO_WORD],
        _ => Vec::new(),
    }
}

/// The declared names offered in each context, in the order they are offered.
fn names_for(context: Context, harvest: &Harvest) -> Vec<&Declared> {
    let groups: Vec<&Vec<Declared>> = match context {
        Context::StepBody => vec![&harvest.callables, &harvest.values],
        Context::RouteTarget => vec![&harvest.outcomes, &harvest.steps],
        Context::StepReference => vec![&harvest.steps],
        Context::SignalReference => vec![&harvest.signals],
        Context::TypeReference | Context::TypeBody | Context::WorkflowHeader => {
            vec![&harvest.types]
        }
        Context::Condition | Context::PipeStage => vec![&harvest.values],
        Context::TopLevel | Context::WorkerBlock => Vec::new(),
    };
    groups.into_iter().flatten().collect()
}

/// Assemble the items, declared names first.
///
/// `sort_text` puts what the document itself declares above the language's
/// keywords. An author reaching for completion in a step body wants the action
/// they wrote, not the word `sequential`; every keyword is still there, just
/// below.
fn build(context: Context, harvest: &Harvest) -> Vec<CompletionItem> {
    let mut seen = BTreeSet::new();
    let mut items = Vec::new();

    for declared in names_for(context, harvest) {
        if !seen.insert(declared.name.clone()) {
            continue;
        }
        items.push(CompletionItem {
            label: declared.name.clone(),
            kind: Some(declared.kind),
            detail: Some(declared.detail.to_owned()),
            sort_text: Some(format!("0{}", declared.name)),
            ..CompletionItem::default()
        });
    }

    if matches!(context, Context::TypeReference | Context::TypeBody) {
        for builtin in BUILTIN_TYPES {
            if seen.insert(builtin.to_owned()) {
                items.push(CompletionItem {
                    label: builtin.to_owned(),
                    kind: Some(CompletionItemKind::STRUCT),
                    detail: Some("built-in type".to_owned()),
                    sort_text: Some(format!("0{builtin}")),
                    ..CompletionItem::default()
                });
            }
        }
    }

    for word in keywords_for(context) {
        if !seen.insert(word.to_owned()) {
            continue;
        }
        let positional = POSITIONAL_WORDS.contains(&word);
        items.push(CompletionItem {
            label: word.to_owned(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some(if positional { "word" } else { "keyword" }.to_owned()),
            // The offered word carries the language's own sentence about it.
            // A positional word is honest here in a way it is not in hover:
            // it is being offered precisely because the cursor sits in the
            // one position where the parser gives it that meaning.
            documentation: crate::glossary::documentation_for(word),
            sort_text: Some(format!("1{word}")),
            ..CompletionItem::default()
        });
    }
    items
}

#[cfg(test)]
#[path = "completion_tests.rs"]
mod completion_tests;
