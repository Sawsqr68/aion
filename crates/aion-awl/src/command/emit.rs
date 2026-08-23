//! The emitter: a checked declaration plus a parameter set becomes an argv.
//!
//! # The cure, by construction
//!
//! For an invocation declared ADMITTING, `--` is placed between the options
//! and the operands UNCONDITIONALLY. Not when an operand looks dangerous, not
//! when a reviewer remembers, not behind a flag — always. That is what makes
//! the leading-dash family close by construction: there is no path through
//! this function that emits an admitting invocation's operands without the
//! marker in front of them, so the defect is not caught, it is unwritable.
//!
//! The canonical case is the one the estate paid for: `git tag -- <name>
//! <sha>`, where a tag name beginning with `-` would otherwise be read as an
//! option by `git tag`.
//!
//! For an invocation declared REFUSING, there is no marker to place, so the
//! protection is the other discipline: an operand whose emitted bytes begin
//! with `-` is REFUSED, because nothing downstream could tell it from an
//! option. The declaration's mandatory specimen is what proves that refusal
//! fires (see [`super::specimen`]).
//!
//! # The argv is never a string
//!
//! [`Argv`] is a vector of whole elements. Nothing here joins them, quotes
//! them, or escapes them, because every one of those operations exists only
//! to survive a re-split that this surface does not perform.

use std::collections::BTreeMap;

use crate::ast::TypeRef;

use super::cause::CommandCause;
use super::check::CheckedCommand;
use super::decl::{CommandDecl, CommandParam, END_OF_OPTIONS_MARKER};
use super::error::CommandRefusal;
use super::fill::{Fill, FillSegment};

/// One argument value supplied to a parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgumentValue {
    /// A single value, rendered as written.
    Scalar(String),
    /// A list of values. Emitted as one argv element per item, in order, in
    /// the operand that carries the variadic tail.
    List(Vec<String>),
}

impl ArgumentValue {
    /// A scalar from anything string-like.
    pub fn scalar(value: impl Into<String>) -> Self {
        Self::Scalar(value.into())
    }

    /// A list from an iterator of string-likes.
    pub fn list<I, S>(values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self::List(values.into_iter().map(Into::into).collect())
    }

    /// Whether this value is a list.
    #[must_use]
    pub const fn is_list(&self) -> bool {
        matches!(self, Self::List(_))
    }
}

/// The values a caller supplies for a command's declared parameters.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParameterSet {
    values: BTreeMap<String, ArgumentValue>,
}

impl ParameterSet {
    /// An empty set.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Bind one parameter, replacing any earlier binding of that name.
    #[must_use]
    pub fn with(mut self, name: impl Into<String>, value: ArgumentValue) -> Self {
        self.values.insert(name.into(), value);
        self
    }

    /// The value bound to `name`, if any.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&ArgumentValue> {
        self.values.get(name)
    }

    /// Every bound name, in name order.
    #[must_use]
    pub fn names(&self) -> Vec<&str> {
        self.values.keys().map(String::as_str).collect()
    }
}

/// A resolved command line: the argv, the environment, the working
/// directory, and the declared ceilings.
///
/// Every field is what the declaration said, resolved. Nothing here is a
/// default this surface invented: an absent timeout is `None` and stays
/// `None`, and what an absent timeout means is the engine's answer, not this
/// one's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Argv {
    /// The argv, one whole element per entry. Never joined into a string.
    pub argv: Vec<String>,
    /// The environment bindings, in name order.
    pub env: Vec<(String, String)>,
    /// The declared working directory, verbatim — placeholder unexpanded,
    /// because expanding it is the executing side's act against ITS workspace.
    pub cwd: Option<String>,
    /// The hardened PATH, if the declaration states one.
    pub hardened_path: Option<String>,
    /// The declared open-file ceiling, if the declaration states one.
    pub open_files: Option<u64>,
    /// The declared timeout in milliseconds, if the declaration states one.
    ///
    /// `None` means the declaration stated NONE, and that is a legal declared
    /// state: an unset timeout defers to the engine's own behaviour. Nothing
    /// here substitutes a ceiling, because a number this surface invented
    /// would be indistinguishable from one an author chose.
    pub timeout_ms: Option<i64>,
    /// Who owns the declared timeout, if there is one.
    pub timeout_owner: Option<String>,
}

/// Resolve a checked declaration against a parameter set.
///
/// # Errors
///
/// Returns the first emit-layer refusal the resolution earns: a missing or
/// undeclared argument, a type mismatch, or a leading-dash operand on an
/// invocation that refuses the end-of-options marker.
pub fn emit(
    command: CheckedCommand<'_>,
    parameters: &ParameterSet,
) -> Result<Argv, CommandRefusal> {
    let decl = command.decl();
    let bound = bind(decl, parameters)?;

    let mut argv: Vec<String> = command
        .program()
        .words
        .iter()
        .map(|word| word.text.clone())
        .collect();

    for option in &decl.options {
        argv.push(option.flag.clone());
        if let Some(value) = &option.value {
            argv.extend(resolve(&value.fill, &bound));
        }
    }

    // THE CURE. An admitting invocation gets the marker between its options
    // and its operands, every time, with no condition on what the operands
    // turn out to be. There is no other path to the operands below.
    if command.admits_end_of_options() && !decl.operands.is_empty() {
        argv.push(END_OF_OPTIONS_MARKER.to_owned());
    }

    for operand in &decl.operands {
        for element in resolve(&operand.value.fill, &bound) {
            if !command.admits_end_of_options() && element.starts_with('-') {
                return Err(CommandRefusal::refuse(
                    CommandCause::LeadingDashOperand,
                    operand.span,
                    &element,
                    format!(
                        "command `{}` refuses the end-of-options marker, and operand `{}` would \
                         emit `{element}`, which begins with `-`. Nothing downstream could tell \
                         it from an option, and there is no `--` to protect it",
                        decl.name, operand.name
                    ),
                ));
            }
            argv.push(element);
        }
    }

    let env = decl
        .env
        .iter()
        .map(|binding| (binding.name.clone(), join(&resolve(&binding.fill, &bound))))
        .collect();
    let hardening = command.hardening();

    Ok(Argv {
        argv,
        env,
        cwd: decl.cwd.as_ref().map(|cwd| cwd.value.clone()),
        hardened_path: hardening
            .and_then(|block| block.path.as_ref())
            .map(|path| join(&resolve(&path.fill, &bound))),
        open_files: hardening.and_then(|block| block.open_files.as_ref().map(|limit| limit.value)),
        timeout_ms: decl
            .timeout
            .as_ref()
            .and_then(|timeout| timeout.duration.checked_milliseconds()),
        timeout_owner: decl.timeout.as_ref().map(|timeout| timeout.owner.clone()),
    })
}

/// Every declared parameter's value: supplied, or its declared default.
fn bind(
    decl: &CommandDecl,
    parameters: &ParameterSet,
) -> Result<BTreeMap<String, ArgumentValue>, CommandRefusal> {
    for name in parameters.names() {
        if !decl.parameters.iter().any(|item| item.name == name) {
            return Err(CommandRefusal::refuse(
                CommandCause::ArgumentUndeclared,
                decl.name_span,
                name,
                format!(
                    "the parameter set names `{name}`, which command `{}` does not declare; a \
                     value nothing consumes is a value nobody reviewed",
                    decl.name
                ),
            ));
        }
    }

    let mut bound: BTreeMap<String, ArgumentValue> = BTreeMap::new();
    for parameter in &decl.parameters {
        let value = if let Some(supplied) = parameters.get(&parameter.name) {
            supplied_value(decl, parameter, supplied)?
        } else {
            defaulted_value(decl, parameter, &bound)?
        };
        bound.insert(parameter.name.clone(), value);
    }
    Ok(bound)
}

/// A supplied value, or the refusal that its shape does not match the
/// parameter's declared type.
fn supplied_value(
    decl: &CommandDecl,
    parameter: &CommandParam,
    supplied: &ArgumentValue,
) -> Result<ArgumentValue, CommandRefusal> {
    let declared_list = matches!(parameter.ty, TypeRef::List { .. });
    if supplied.is_list() == declared_list {
        return Ok(supplied.clone());
    }
    let shape = |list: bool| if list { "list" } else { "single value" };
    Err(CommandRefusal::refuse(
        CommandCause::ArgumentTypeMismatch,
        parameter.name_span,
        describe_value(supplied),
        format!(
            "parameter `{}` of command `{}` is declared a {}, and the parameter set supplies a {}",
            parameter.name,
            decl.name,
            shape(declared_list),
            shape(supplied.is_list())
        ),
    ))
}

/// A parameter's declared default, or the refusal that it has none.
///
/// There is deliberately no third branch: a parameter with neither a supplied
/// value nor a declared default has nothing for this surface to invent.
fn defaulted_value(
    decl: &CommandDecl,
    parameter: &CommandParam,
    bound: &BTreeMap<String, ArgumentValue>,
) -> Result<ArgumentValue, CommandRefusal> {
    let Some(default) = &parameter.default else {
        return Err(CommandRefusal::refuse(
            CommandCause::ArgumentMissing,
            parameter.name_span,
            &parameter.name,
            format!(
                "the parameter set omits `{}`, which command `{}` declares and gives no default; \
                 there is nothing for this surface to invent",
                parameter.name, decl.name
            ),
        ));
    };
    Ok(ArgumentValue::Scalar(join(&resolve(default, bound))))
}

/// A fill resolved against bound values, as whole argv elements.
///
/// One element for every shape but the variadic tail: a fill that is exactly
/// one list-typed hole becomes one element per item, which is the only way a
/// list reaches an argv. Every other fill is one element, whatever it
/// contains — a space inside a value is a space inside that one element and
/// never a new argument, because nothing re-splits it.
fn resolve(fill: &Fill, bound: &BTreeMap<String, ArgumentValue>) -> Vec<String> {
    if let Some(name) = fill.sole_hole() {
        if let Some(ArgumentValue::List(items)) = bound.get(name) {
            return items.clone();
        }
    }
    let mut element = String::new();
    for segment in &fill.segments {
        match segment {
            FillSegment::Literal(text) => element.push_str(text),
            FillSegment::Hole { name, .. } => match bound.get(name.as_str()) {
                Some(ArgumentValue::Scalar(value)) => element.push_str(value),
                // A list written into a larger word is refused at the check
                // layer, so this arm is reached only by a caller that skipped
                // the checker; joining is then the least surprising answer and
                // is never silent, because the check refusal already named it.
                Some(ArgumentValue::List(items)) => element.push_str(&items.join(" ")),
                None => {}
            },
        }
    }
    vec![element]
}

/// One string from resolved elements, for the places an argv element is not
/// what is wanted: an environment value and a PATH are each one string.
fn join(elements: &[String]) -> String {
    elements.join(" ")
}

/// A value described for a diagnostic, without pretending a list is a string.
fn describe_value(value: &ArgumentValue) -> String {
    match value {
        ArgumentValue::Scalar(text) => text.clone(),
        ArgumentValue::List(items) => format!("[{}]", items.join(", ")),
    }
}
