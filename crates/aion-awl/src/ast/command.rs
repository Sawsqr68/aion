//! Parameter references inside a declarative action body's command.
//!
//! A `run "…"` command may reference the action's declared parameters. This
//! module finds those references so the checker can rule on them BEFORE a
//! document is ever deployed, and so the compile seam can record which
//! parameters a command binds.
//!
//! # Agreement with the executor
//!
//! The word and reference scanning here mirrors the worker SDK's
//! `CommandTemplate::parse` character for character — words split on
//! unquoted whitespace, `'…'` is literal throughout (a `$` inside one is
//! not a reference), `"…"` groups while still substituting, `$name` and
//! `${name}` are references, and `$$` is a literal dollar. It differs ONLY
//! by being stricter, never more permissive:
//!
//! * a reference must name a `snake_case` AWL parameter, so `$HOME` and
//!   `${Not A Name}` are refused here and never reach the executor;
//! * a reference anywhere in the program word is refused, not only a bare
//!   one, because a caller-supplied value must never influence which
//!   program executes.
//!
//! Every command this module accepts therefore parses identically in the
//! executor and binds exactly the same parameter names. Nothing here
//! renders a command back into a string: the executor parses the declared
//! text into an argv itself and substitutes each value as one whole
//! argument.

/// One parameter reference found in a declared command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandReference {
    /// Referenced parameter name.
    pub name: String,
    /// Byte offset of the `$` opening the reference, within the command
    /// text (not the source document).
    pub offset: usize,
    /// Byte length of the whole reference (`$name` or `${name}`) within the
    /// command text.
    pub len: usize,
}

/// What scanning a declared command found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScannedCommand {
    /// Every parameter reference, in source order.
    pub references: Vec<CommandReference>,
    /// Byte offset just past the command's program word — the first
    /// whitespace-separated word, quotes included. A reference starting
    /// before this offset sits in program position.
    pub program_end: usize,
}

impl ScannedCommand {
    /// The referenced parameter names, sorted and deduplicated: the binding
    /// a runtime must supply for this command.
    #[must_use]
    pub fn parameters(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .references
            .iter()
            .map(|reference| reference.name.clone())
            .collect();
        names.sort_unstable();
        names.dedup();
        names
    }
}

/// A malformed reference or quote in a declared command.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct CommandError {
    /// Byte offset where the offending run starts, within the command text.
    pub offset: usize,
    /// Byte length of the offending run within the command text.
    pub len: usize,
    /// Human-readable diagnostic, ready to anchor at a span.
    pub message: String,
}

/// The escape every malformed-reference diagnostic teaches, so an author
/// learns it from the failure itself.
const ESCAPE_HINT: &str = "write `$$` for a literal dollar sign";

/// The scanning cursor: characters with their byte offsets, one ahead.
type Cursor<'a> = std::iter::Peekable<std::str::CharIndices<'a>>;

/// Scan a declared command for its parameter references and program word.
///
/// # Errors
///
/// Returns [`CommandError`] when a quote or a `${` is never closed, a `$`
/// names no parameter, or a reference names something that is not a
/// `snake_case` AWL parameter.
pub fn scan_command(command: &str) -> Result<ScannedCommand, CommandError> {
    let mut references = Vec::new();
    let mut started = false;
    let mut program_end = None;
    let mut chars = command.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        if ch.is_whitespace() {
            if started {
                started = false;
                if program_end.is_none() {
                    program_end = Some(index);
                }
            }
            continue;
        }
        started = true;
        match ch {
            '\'' => skip_single_quoted(&mut chars, command, index)?,
            '"' => read_double_quoted(&mut chars, command, index, &mut references)?,
            '$' => {
                if let Some(found) = read_reference(&mut chars, command, index)? {
                    references.push(found);
                }
            }
            _ => {}
        }
    }
    Ok(ScannedCommand {
        references,
        program_end: program_end.unwrap_or(command.len()),
    })
}

/// Consume a `'…'` run; its opening quote at `open` has been consumed.
/// Single quotes are literal throughout, so a `$` inside one is a dollar
/// sign and never a reference — the executor agrees.
fn skip_single_quoted(
    chars: &mut Cursor<'_>,
    command: &str,
    open: usize,
) -> Result<(), CommandError> {
    loop {
        match chars.next() {
            Some((_, '\'')) => return Ok(()),
            Some(_) => {}
            None => return Err(unterminated_quote(command, open, '\'')),
        }
    }
}

/// Consume a `"…"` run, collecting the references inside it; its opening
/// quote at `open` has been consumed.
fn read_double_quoted(
    chars: &mut Cursor<'_>,
    command: &str,
    open: usize,
    references: &mut Vec<CommandReference>,
) -> Result<(), CommandError> {
    loop {
        match chars.next() {
            Some((_, '"')) => return Ok(()),
            Some((at, '$')) => {
                if let Some(found) = read_reference(chars, command, at)? {
                    references.push(found);
                }
            }
            Some(_) => {}
            None => return Err(unterminated_quote(command, open, '"')),
        }
    }
}

/// Read what follows a `$` at `dollar`: `Ok(None)` for the `$$` escape,
/// `Ok(Some(_))` for a reference.
fn read_reference(
    chars: &mut Cursor<'_>,
    command: &str,
    dollar: usize,
) -> Result<Option<CommandReference>, CommandError> {
    if matches!(chars.peek(), Some(&(_, '$'))) {
        chars.next();
        return Ok(None);
    }
    let name = if matches!(chars.peek(), Some(&(_, '{'))) {
        chars.next();
        read_braced_name(chars, command, dollar)?
    } else {
        read_bare_name(chars)
    };
    let len = position(chars, command) - dollar;
    if name.is_empty() {
        return Err(CommandError {
            offset: dollar,
            len,
            message: format!(
                "`$` must be followed by a parameter name (`$name` or `${{name}}`); {ESCAPE_HINT}"
            ),
        });
    }
    if !is_snake_case(&name) {
        return Err(CommandError {
            offset: dollar,
            len,
            message: format!(
                "`{name}` is not a parameter name; a `run` command may only interpolate the \
                 action's own snake_case parameters ([a-z][a-z0-9_]*), so {ESCAPE_HINT} when \
                 the dollar belongs to the command itself"
            ),
        });
    }
    Ok(Some(CommandReference {
        name,
        offset: dollar,
        len,
    }))
}

/// Read a bare `$name`: every following name character, over exactly the
/// character set the executor consumes, so the two agree on where a name
/// ends. Whether the result is a legal AWL name is the caller's ruling.
fn read_bare_name(chars: &mut Cursor<'_>) -> String {
    let mut name = String::new();
    while let Some(&(_, next)) = chars.peek() {
        if !next.is_alphanumeric() && next != '_' {
            break;
        }
        name.push(next);
        chars.next();
    }
    name
}

/// Read a `${name}`: everything up to the closing brace, exactly as the
/// executor does, so a name it would take is never silently reinterpreted
/// here. Whether the contents are a legal AWL name is the caller's ruling.
fn read_braced_name(
    chars: &mut Cursor<'_>,
    command: &str,
    dollar: usize,
) -> Result<String, CommandError> {
    let mut name = String::new();
    loop {
        match chars.next() {
            Some((_, '}')) => return Ok(name),
            Some((_, inner)) => name.push(inner),
            None => {
                return Err(CommandError {
                    offset: dollar,
                    len: command.len() - dollar,
                    message: format!(
                        "`${{` is missing its closing `}}` in the command; {ESCAPE_HINT}"
                    ),
                });
            }
        }
    }
}

fn unterminated_quote(command: &str, open: usize, quote: char) -> CommandError {
    CommandError {
        offset: open,
        len: command.len() - open,
        message: format!("the command has an unterminated {quote} quote"),
    }
}

/// The cursor's byte offset: the next character's offset, or the end of the
/// command when the cursor is exhausted.
fn position(chars: &mut Cursor<'_>, command: &str) -> usize {
    chars.peek().map_or(command.len(), |&(index, _)| index)
}

/// Whether `name` is a `snake_case` AWL name (`[a-z][a-z0-9_]*`).
fn is_snake_case(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|first| first.is_ascii_lowercase())
        && chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
}
