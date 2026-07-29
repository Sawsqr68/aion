//! The parsed form of a declared command and its parameter substitution.
//!
//! A declared command is parsed ONCE, at registration, into a fixed argv
//! whose elements are either literal text or a named parameter reference.
//! Substitution then replaces a reference with a parameter's value **as one
//! whole argv element**.
//!
//! That structure is the security property, not a convenience. A parameter
//! value never re-enters a parser, so it cannot introduce a word boundary, a
//! quote, a pipe, a redirect, a `;`, or a `$(…)`. A value of `; rm -rf /` is
//! passed to the program as the single literal argument `; rm -rf /`. There is
//! no shell between the declaration and `execve`, so there is nothing for a
//! value to escape into. Splicing values into a command STRING and handing it
//! to `sh -c` would make every parameter an injection site; this crate never
//! does that.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use thiserror::Error;

/// A declared command could not be parsed into an executable argv.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TemplateError {
    /// The command was empty or contained only whitespace, so it names no
    /// program to run.
    #[error("the declared command is empty; an action body must name a program to run")]
    Empty,
    /// A quote was opened and never closed.
    #[error("the declared command has an unterminated {quote} quote")]
    UnterminatedQuote {
        /// The quote character that was opened and never closed.
        quote: char,
    },
    /// A `$` introduced no parameter name.
    #[error("`$` must be followed by a parameter name in the declared command")]
    EmptyParameterName,
    /// A `${` was opened and never closed.
    #[error("`${{` is missing its closing `}}` in the declared command")]
    UnterminatedParameterBrace,
    /// The command's program name is itself a parameter reference.
    ///
    /// Refused because it would let a caller-supplied value choose which
    /// program executes.
    #[error(
        "the program to run must be a literal, not the parameter `{parameter}`; \
         a caller-supplied value must never choose which program executes"
    )]
    ParameterizedProgram {
        /// The parameter that was found in program position.
        parameter: String,
    },
}

/// A value supplied for a parameter could not be rendered into an argument.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SubstitutionError {
    /// The command references a parameter that the call did not supply.
    #[error("the declared command references parameter `{parameter}`, which was not supplied")]
    MissingParameter {
        /// The referenced parameter name.
        parameter: String,
    },
    /// A supplied value has no single unambiguous argument form.
    #[error(
        "parameter `{parameter}` is {kind}, which has no unambiguous command-argument form; \
         supply a string, number, or boolean"
    )]
    UnrepresentableValue {
        /// The referenced parameter name.
        parameter: String,
        /// The JSON kind that was supplied.
        kind: &'static str,
    },
    /// A supplied value contains a NUL byte.
    ///
    /// Refused rather than truncated: an argument is delivered to the kernel
    /// as a NUL-terminated string, so a NUL inside a value would silently cut
    /// the argument short and change what the program receives.
    #[error(
        "parameter `{parameter}` contains a NUL byte, which cannot appear in a command argument"
    )]
    InteriorNul {
        /// The referenced parameter name.
        parameter: String,
    },
}

/// One piece of a single argv element.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Piece {
    /// Literal text, used exactly as written.
    Literal(String),
    /// A parameter reference, replaced by that parameter's value.
    Parameter(String),
}

/// One argv element: its pieces are concatenated to produce exactly one
/// argument, however many parameters it references.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Word {
    pieces: Vec<Piece>,
}

impl Word {
    /// Whether this word is a single bare parameter reference.
    fn sole_parameter(&self) -> Option<&str> {
        match self.pieces.as_slice() {
            [Piece::Parameter(name)] => Some(name),
            _ => None,
        }
    }

    /// Render this word into one argument using `values`.
    fn render(
        &self,
        values: &BTreeMap<String, serde_json::Value>,
    ) -> Result<String, SubstitutionError> {
        let mut rendered = String::new();
        for piece in &self.pieces {
            match piece {
                Piece::Literal(text) => rendered.push_str(text),
                Piece::Parameter(name) => {
                    let value =
                        values
                            .get(name)
                            .ok_or_else(|| SubstitutionError::MissingParameter {
                                parameter: name.clone(),
                            })?;
                    render_value(name, value, &mut rendered)?;
                }
            }
        }
        Ok(rendered)
    }
}

/// Append `value`'s argument form to `out`, refusing anything ambiguous.
fn render_value(
    parameter: &str,
    value: &serde_json::Value,
    out: &mut String,
) -> Result<(), SubstitutionError> {
    let unrepresentable = |kind: &'static str| SubstitutionError::UnrepresentableValue {
        parameter: parameter.to_owned(),
        kind,
    };
    match value {
        serde_json::Value::String(text) => {
            if text.contains('\0') {
                return Err(SubstitutionError::InteriorNul {
                    parameter: parameter.to_owned(),
                });
            }
            out.push_str(text);
            Ok(())
        }
        serde_json::Value::Number(number) => {
            // `write!` to a String is infallible; the result is consumed so no
            // formatting error can be silently dropped.
            let _ = write!(out, "{number}");
            Ok(())
        }
        serde_json::Value::Bool(flag) => {
            out.push_str(if *flag { "true" } else { "false" });
            Ok(())
        }
        // A null, array, or object has no single obvious argument spelling.
        // Guessing one (empty string? JSON text? space-joined?) would make the
        // command mean something the author never wrote, so each is refused by
        // name instead.
        serde_json::Value::Null => Err(unrepresentable("null")),
        serde_json::Value::Array(_) => Err(unrepresentable("an array")),
        serde_json::Value::Object(_) => Err(unrepresentable("an object")),
    }
}

/// A declared command parsed into an executable argv with named holes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandTemplate {
    program: Word,
    arguments: Vec<Word>,
}

impl CommandTemplate {
    /// Parse a declared command into a program and its arguments.
    ///
    /// Words are split on unquoted whitespace. Single quotes take their
    /// contents literally, including `$`. Double quotes group text into one
    /// word while still substituting parameters. A parameter is written `$name`
    /// or `${name}`; `$$` is a literal `$`.
    ///
    /// # Errors
    ///
    /// Returns [`TemplateError`] when the command is empty, a quote or `${` is
    /// unterminated, a `$` names no parameter, or the program position is a
    /// parameter reference.
    pub fn parse(command: &str) -> Result<Self, TemplateError> {
        let mut words = Vec::new();
        let mut pieces: Vec<Piece> = Vec::new();
        let mut literal = String::new();
        let mut started = false;
        let mut chars = command.chars().peekable();

        // Close the literal run in progress, if any, into `pieces`.
        macro_rules! flush_literal {
            () => {
                if !literal.is_empty() {
                    pieces.push(Piece::Literal(std::mem::take(&mut literal)));
                }
            };
        }

        while let Some(character) = chars.next() {
            match character {
                whitespace if whitespace.is_whitespace() => {
                    flush_literal!();
                    if started {
                        words.push(Word {
                            pieces: std::mem::take(&mut pieces),
                        });
                        started = false;
                    }
                }
                '\'' => {
                    // Single quotes are literal throughout: no substitution.
                    started = true;
                    loop {
                        match chars.next() {
                            Some('\'') => break,
                            Some(inner) => literal.push(inner),
                            None => {
                                return Err(TemplateError::UnterminatedQuote { quote: '\'' });
                            }
                        }
                    }
                }
                '"' => {
                    started = true;
                    loop {
                        match chars.next() {
                            Some('"') => break,
                            Some('$') => {
                                flush_literal!();
                                push_parameter(&mut chars, &mut pieces, &mut literal)?;
                            }
                            Some(inner) => literal.push(inner),
                            None => {
                                return Err(TemplateError::UnterminatedQuote { quote: '"' });
                            }
                        }
                    }
                }
                '$' => {
                    started = true;
                    flush_literal!();
                    push_parameter(&mut chars, &mut pieces, &mut literal)?;
                }
                other => {
                    started = true;
                    literal.push(other);
                }
            }
        }

        flush_literal!();
        if started {
            words.push(Word { pieces });
        }

        let mut words = words.into_iter();
        let program = words.next().ok_or(TemplateError::Empty)?;
        if let Some(parameter) = program.sole_parameter() {
            return Err(TemplateError::ParameterizedProgram {
                parameter: parameter.to_owned(),
            });
        }
        Ok(Self {
            program,
            arguments: words.collect(),
        })
    }

    /// The parameter names this command references, in sorted order.
    #[must_use]
    pub fn referenced_parameters(&self) -> Vec<String> {
        let mut names: Vec<String> = std::iter::once(&self.program)
            .chain(&self.arguments)
            .flat_map(|word| &word.pieces)
            .filter_map(|piece| match piece {
                Piece::Parameter(name) => Some(name.clone()),
                Piece::Literal(_) => None,
            })
            .collect();
        names.sort_unstable();
        names.dedup();
        names
    }

    /// Render the full argv, substituting `values` for parameter references.
    ///
    /// The first element is the program; the rest are its arguments. Each
    /// element is one whole argument regardless of what a value contains.
    ///
    /// # Errors
    ///
    /// Returns [`SubstitutionError`] when a referenced parameter is absent or
    /// its value has no unambiguous argument form.
    pub fn render(
        &self,
        values: &BTreeMap<String, serde_json::Value>,
    ) -> Result<Vec<String>, SubstitutionError> {
        let mut argv = Vec::with_capacity(self.arguments.len() + 1);
        argv.push(self.program.render(values)?);
        for argument in &self.arguments {
            argv.push(argument.render(values)?);
        }
        Ok(argv)
    }
}

/// Consume a parameter reference that follows a `$`.
///
/// `$$` is a literal `$` and is pushed onto `literal` rather than becoming a
/// reference.
fn push_parameter(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    pieces: &mut Vec<Piece>,
    literal: &mut String,
) -> Result<(), TemplateError> {
    if chars.peek() == Some(&'$') {
        chars.next();
        literal.push('$');
        return Ok(());
    }
    let mut name = String::new();
    if chars.peek() == Some(&'{') {
        chars.next();
        loop {
            match chars.next() {
                Some('}') => break,
                Some(inner) => name.push(inner),
                None => return Err(TemplateError::UnterminatedParameterBrace),
            }
        }
    } else {
        while let Some(&next) = chars.peek() {
            if next.is_alphanumeric() || next == '_' {
                name.push(next);
                chars.next();
            } else {
                break;
            }
        }
    }
    if name.is_empty() {
        return Err(TemplateError::EmptyParameterName);
    }
    pieces.push(Piece::Parameter(name));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{CommandTemplate, SubstitutionError, TemplateError};
    use std::collections::BTreeMap;

    fn values(pairs: &[(&str, serde_json::Value)]) -> BTreeMap<String, serde_json::Value> {
        pairs
            .iter()
            .map(|(name, value)| ((*name).to_owned(), value.clone()))
            .collect()
    }

    /// What a test returns. Every fallible step is carried rather than
    /// unwrapped, because the workspace denies panicking accessors in test
    /// code as firmly as in library code.
    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn render(
        command: &str,
        pairs: &[(&str, serde_json::Value)],
    ) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        Ok(CommandTemplate::parse(command)?.render(&values(pairs))?)
    }

    #[test]
    fn splits_a_plain_command_into_program_and_arguments() -> TestResult {
        assert_eq!(render("echo hello world", &[])?, ["echo", "hello", "world"]);
        Ok(())
    }

    #[test]
    fn collapses_runs_of_whitespace_between_words() -> TestResult {
        assert_eq!(render("echo   a\t\tb", &[])?, ["echo", "a", "b"]);
        Ok(())
    }

    #[test]
    fn substitutes_a_bare_parameter_as_one_argument() -> TestResult {
        assert_eq!(
            render("echo $name", &[("name", serde_json::json!("Ada"))])?,
            ["echo", "Ada"]
        );
        Ok(())
    }

    #[test]
    fn substitutes_a_braced_parameter() -> TestResult {
        assert_eq!(
            render("echo ${name}!", &[("name", serde_json::json!("Ada"))])?,
            ["echo", "Ada!"]
        );
        Ok(())
    }

    #[test]
    fn a_value_containing_spaces_stays_one_argument() -> TestResult {
        // The whole point: no word splitting after substitution.
        assert_eq!(
            render("echo $name", &[("name", serde_json::json!("Ada Lovelace"))])?,
            ["echo", "Ada Lovelace"]
        );
        Ok(())
    }

    #[test]
    fn shell_metacharacters_in_a_value_are_inert_literal_text() -> TestResult {
        // A value can never introduce a command, a pipe, or a redirect: it is
        // handed to the program as one argument and never re-parsed.
        for hostile in [
            "; rm -rf /",
            "$(rm -rf /)",
            "`rm -rf /`",
            "a | b",
            "a && b",
            "a > /etc/passwd",
            "'; DROP TABLE users; --",
            "$HOME",
            "\n rm -rf /",
        ] {
            assert_eq!(
                render("echo $value", &[("value", serde_json::json!(hostile))])?,
                ["echo", hostile],
                "a value must never be re-parsed: {hostile}"
            );
        }
        Ok(())
    }

    #[test]
    fn a_substituted_value_is_never_rescanned_for_parameters() -> TestResult {
        assert_eq!(
            render(
                "echo $outer",
                &[
                    ("outer", serde_json::json!("$inner")),
                    ("inner", serde_json::json!("substituted twice")),
                ]
            )?,
            ["echo", "$inner"]
        );
        Ok(())
    }

    #[test]
    fn double_quotes_group_text_and_still_substitute() -> TestResult {
        assert_eq!(
            render(
                "echo \"hello $name\"",
                &[("name", serde_json::json!("Ada"))]
            )?,
            ["echo", "hello Ada"]
        );
        Ok(())
    }

    #[test]
    fn single_quotes_are_literal_including_dollar_signs() -> TestResult {
        assert_eq!(render("echo '$name'", &[])?, ["echo", "$name"]);
        Ok(())
    }

    #[test]
    fn an_empty_quoted_word_survives_as_an_empty_argument() -> TestResult {
        assert_eq!(render("echo '' x", &[])?, ["echo", "", "x"]);
        Ok(())
    }

    #[test]
    fn a_doubled_dollar_is_a_literal_dollar() -> TestResult {
        assert_eq!(render("echo $$name", &[])?, ["echo", "$name"]);
        Ok(())
    }

    #[test]
    fn numbers_and_booleans_render_without_quotes() -> TestResult {
        assert_eq!(
            render(
                "run $count $flag",
                &[
                    ("count", serde_json::json!(42)),
                    ("flag", serde_json::json!(true)),
                ]
            )?,
            ["run", "42", "true"]
        );
        Ok(())
    }

    #[test]
    fn an_empty_command_is_refused() {
        assert_eq!(CommandTemplate::parse("   "), Err(TemplateError::Empty));
    }

    #[test]
    fn an_unterminated_quote_is_refused() {
        assert_eq!(
            CommandTemplate::parse("echo 'oops"),
            Err(TemplateError::UnterminatedQuote { quote: '\'' })
        );
        assert_eq!(
            CommandTemplate::parse("echo \"oops"),
            Err(TemplateError::UnterminatedQuote { quote: '"' })
        );
    }

    #[test]
    fn an_unterminated_brace_is_refused() {
        assert_eq!(
            CommandTemplate::parse("echo ${name"),
            Err(TemplateError::UnterminatedParameterBrace)
        );
    }

    #[test]
    fn a_dollar_naming_nothing_is_refused() {
        assert_eq!(
            CommandTemplate::parse("echo $ x"),
            Err(TemplateError::EmptyParameterName)
        );
    }

    #[test]
    fn a_parameterized_program_is_refused() {
        // A caller-supplied value must never choose which program runs.
        assert_eq!(
            CommandTemplate::parse("$program arg"),
            Err(TemplateError::ParameterizedProgram {
                parameter: "program".to_owned()
            })
        );
    }

    #[test]
    fn a_missing_parameter_is_refused_by_name() -> TestResult {
        let Err(error) = CommandTemplate::parse("echo $absent")?.render(&BTreeMap::new()) else {
            return Err("a missing parameter must be refused, not rendered".into());
        };
        assert_eq!(
            error,
            SubstitutionError::MissingParameter {
                parameter: "absent".to_owned()
            }
        );
        Ok(())
    }

    #[test]
    fn structured_and_null_values_are_refused_rather_than_guessed() -> TestResult {
        for (value, kind) in [
            (serde_json::json!(null), "null"),
            (serde_json::json!([1, 2]), "an array"),
            (serde_json::json!({"a": 1}), "an object"),
        ] {
            let Err(error) =
                CommandTemplate::parse("echo $value")?.render(&values(&[("value", value)]))
            else {
                return Err(format!("a value that is {kind} must be refused, not guessed").into());
            };
            assert_eq!(
                error,
                SubstitutionError::UnrepresentableValue {
                    parameter: "value".to_owned(),
                    kind
                }
            );
        }
        Ok(())
    }

    #[test]
    fn an_interior_nul_is_refused_rather_than_truncating_the_argument() -> TestResult {
        let Err(error) = CommandTemplate::parse("echo $value")?
            .render(&values(&[("value", serde_json::json!("a\0b"))]))
        else {
            return Err("an interior NUL must be refused, not silently truncated".into());
        };
        assert_eq!(
            error,
            SubstitutionError::InteriorNul {
                parameter: "value".to_owned()
            }
        );
        Ok(())
    }

    #[test]
    fn referenced_parameters_are_reported_sorted_and_deduplicated() -> TestResult {
        let template = CommandTemplate::parse("run $b $a ${b} literal")?;
        assert_eq!(template.referenced_parameters(), ["a", "b"]);
        Ok(())
    }
}
