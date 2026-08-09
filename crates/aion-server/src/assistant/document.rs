//! The embedded assistant document: its bytes, its compiled identity, and the
//! session contract the operator verbs bind to.
//!
//! # One document, one place
//!
//! `crates/aion-server/assistant-embed/assistant.awl` is the ONLY copy of the
//! assistant document in this repository. It is not a copy of an example — the
//! example was moved here, so there is no second file to drift from. Everything
//! downstream (the boot install, the `/assistant` description, the `aion
//! assistant` verbs, the ops-console contract test) reads this one artifact.
//!
//! # The session contract is named once and VERIFIED, never restated
//!
//! An operator verb has to know which input carries the objective, which signal
//! continues the session, and which fields that signal's payload takes. Those
//! names are declared here exactly once, as constants, and [`EmbeddedAssistant::load`]
//! proves each one against the compiled document before returning. A document
//! edit that renames `objective`, drops the `assistant_continue` signal, or
//! changes the continuation's fields therefore fails loudly at load — at boot,
//! in the verb, and in this module's own tests — instead of leaving a constant
//! quietly disagreeing with the document it names.

use std::path::Path;
use std::sync::OnceLock;

use aion_awl::{CompiledWorkflow, TypeBody};
use aion_package::{ContentHash, ExtractionLimits, Package, PackageError, SignalContract};
use serde_json::Value;

/// The embedded assistant document, compiled into the binary.
///
/// `include_str!` from inside the crate, exactly as the ops-console bundle is
/// embedded from `ops-console-embed/`: the file is git-tracked under the crate
/// root, so `cargo package` carries it and an installed binary holds the same
/// bytes this repository does.
pub const EMBEDDED_ASSISTANT_DOCUMENT: &str = include_str!("../../assistant-embed/assistant.awl");

/// The document's own filename, recorded in the assembled archive's `awl/`
/// provenance tree so a deployed package carries the source it was built from.
pub const EMBEDDED_ASSISTANT_FILENAME: &str = "assistant.awl";

/// The start input carrying the operator's opening ask.
pub const OBJECTIVE_INPUT: &str = "objective";

/// The start input carrying the repository the session grounds itself in.
/// The document's documented scratch mode is the empty string.
pub const REPO_PATH_INPUT: &str = "repo_path";

/// The one control signal a parked session listens on.
pub const CONTINUE_SIGNAL: &str = "assistant_continue";

/// The continuation field carrying the operator's next prompt.
pub const CONTINUE_MESSAGE_FIELD: &str = "message";

/// The continuation field that ends the session cleanly.
pub const CONTINUE_END_FIELD: &str = "end";

/// The read-only query reporting a live session's phase and round count.
pub const STATUS_QUERY: &str = "assistant_status";

/// The schema-import root presented to the compiler.
///
/// The document is embedded as bytes with no directory beside it, so it cannot
/// carry `schema(…)` imports — [`EmbeddedAssistant::load`] refuses one before
/// compiling. This root is therefore never read; it is a path that does not
/// exist so that a future import would fail loudly here rather than silently
/// resolve against whatever directory the server happens to be running in.
const EMBEDDED_SCHEMA_ROOT: &str = "<embedded-assistant-has-no-schema-directory>";

/// A refusal to produce the embedded assistant.
///
/// Every variant names what about the document made it unusable. There is no
/// "assistant unavailable" catch-all: an operator reading a boot log or an
/// `/assistant` error must be able to act on it.
#[derive(Debug, thiserror::Error)]
pub enum EmbeddedAssistantError {
    /// The embedded document does not parse.
    #[error("the embedded assistant document does not parse: {message}")]
    Parse {
        /// The parser's diagnostic, verbatim.
        message: String,
    },

    /// The embedded document carries a `schema(…)` import.
    ///
    /// The binary embeds one file and no directory, so an import has nothing to
    /// resolve against. This is a refusal to guess, not a limitation dressed up
    /// as one: inlining the type into the document removes it.
    #[error(
        "the embedded assistant document imports schema `{path}`, but the binary embeds the \
         document alone and has no directory to resolve imports against; declare the type \
         inline in the document"
    )]
    SchemaImport {
        /// The import path, verbatim from the document.
        path: String,
    },

    /// The embedded document does not compile.
    #[error("the embedded assistant document does not compile: {message}")]
    Compile {
        /// The compiler's diagnostic, verbatim.
        message: String,
    },

    /// The compiled document could not be assembled into an archive.
    #[error("the embedded assistant document could not be packaged: {message}")]
    Assemble {
        /// The assembler's diagnostic, verbatim.
        message: String,
    },

    /// The assembled archive did not load back as a validated package.
    #[error("the embedded assistant package did not validate: {source}")]
    Package {
        /// The package validation failure.
        #[from]
        source: PackageError,
    },

    /// The document does not declare an input the session contract names.
    #[error(
        "the embedded assistant document declares no `{name}` input; the session contract in \
         crates/aion-server/src/assistant/document.rs names it, so document and contract have \
         diverged"
    )]
    MissingInput {
        /// The input the contract names.
        name: &'static str,
    },

    /// The document does not declare the continuation signal.
    #[error(
        "the embedded assistant document declares no `{name}` signal; the session contract in \
         crates/aion-server/src/assistant/document.rs names it, so document and contract have \
         diverged"
    )]
    MissingSignal {
        /// The signal the contract names.
        name: &'static str,
    },

    /// The continuation signal's payload lacks a field the contract sends.
    #[error(
        "the `{signal}` signal payload declares no `{field}` field; the session contract in \
         crates/aion-server/src/assistant/document.rs sends it, so document and contract have \
         diverged"
    )]
    MissingSignalField {
        /// The signal whose payload was inspected.
        signal: &'static str,
        /// The field the contract sends.
        field: &'static str,
    },

    /// The document does not declare the status query.
    #[error(
        "the embedded assistant document declares no `{name}` query; the session contract in \
         crates/aion-server/src/assistant/document.rs names it, so document and contract have \
         diverged"
    )]
    MissingQuery {
        /// The query the contract names.
        name: &'static str,
    },

    /// The compiled package declares no signal contract at all.
    #[error(
        "the embedded assistant package carries no contract, so its signal payloads cannot be \
         read: {message}"
    )]
    MissingContract {
        /// Why the contract could not be read.
        message: String,
    },
}

/// The embedded assistant: the document's bytes, the package compiled from
/// them, and the contract surfaces an operator drives it through.
///
/// Construction is the verification: holding one of these is proof that the
/// embedded document compiled, packaged, and carries every surface the session
/// contract names.
#[derive(Debug, Clone)]
pub struct EmbeddedAssistant {
    source: &'static str,
    package: Package,
    workflow_type: String,
    input_schema: Value,
    signals: Vec<SignalContract>,
    queries: Vec<String>,
}

impl EmbeddedAssistant {
    /// Compiles, packages, and verifies the embedded document.
    ///
    /// # Errors
    ///
    /// Returns [`EmbeddedAssistantError`] naming the stage that refused: parse,
    /// a schema import the binary cannot carry, compilation, archive assembly,
    /// package validation, or a session-contract surface the document does not
    /// declare.
    pub fn load() -> Result<Self, EmbeddedAssistantError> {
        Self::from_source(EMBEDDED_ASSISTANT_DOCUMENT)
    }

    /// The whole preparation, over an arbitrary document.
    ///
    /// [`Self::load`] is this applied to the embedded bytes. It is separate so
    /// the session-contract verification can be exercised against a document
    /// that deliberately omits a surface — a check nothing ever fails is a
    /// check nobody has measured.
    ///
    /// # Errors
    ///
    /// As [`Self::load`].
    pub fn from_source(source: &'static str) -> Result<Self, EmbeddedAssistantError> {
        let document = aion_awl::parse(source).map_err(|error| EmbeddedAssistantError::Parse {
            message: error.message,
        })?;
        for declaration in &document.types {
            if let TypeBody::SchemaImport { path, .. } = &declaration.body {
                return Err(EmbeddedAssistantError::SchemaImport { path: path.clone() });
            }
        }

        let root = Path::new(EMBEDDED_SCHEMA_ROOT);
        let prepared =
            aion_awl_package::compile_and_assemble_awl(source, root, EMBEDDED_ASSISTANT_FILENAME)
                .map_err(|error| match error {
                aion_awl_package::PrepareAwlError::Compile(compile) => {
                    EmbeddedAssistantError::Compile {
                        message: compile.to_string(),
                    }
                }
                other => EmbeddedAssistantError::Assemble {
                    message: other.to_string(),
                },
            })?;
        let CompiledWorkflow { input_schema, .. } = prepared.compiled;

        // Trusted, compile-time content assembled by this process moments ago —
        // not network input — so extraction carries no inflate ceiling
        // (`ExtractionLimits::unbounded`'s stated use).
        let package = Package::load_from_bytes(&prepared.archive, ExtractionLimits::unbounded())?;
        let workflow_type = package.manifest().entry_module.clone();

        let contract =
            package
                .contract()
                .map_err(|error| EmbeddedAssistantError::MissingContract {
                    message: error.to_string(),
                })?;
        let signals = contract.signals.clone();

        for name in [OBJECTIVE_INPUT, REPO_PATH_INPUT] {
            if !document.inputs.iter().any(|input| input.name == name) {
                return Err(EmbeddedAssistantError::MissingInput { name });
            }
        }
        let continuation = signals
            .iter()
            .find(|signal| signal.name == CONTINUE_SIGNAL)
            .ok_or(EmbeddedAssistantError::MissingSignal {
                name: CONTINUE_SIGNAL,
            })?;
        for field in [CONTINUE_MESSAGE_FIELD, CONTINUE_END_FIELD] {
            if !schema_declares_property(&continuation.input_schema, field) {
                return Err(EmbeddedAssistantError::MissingSignalField {
                    signal: CONTINUE_SIGNAL,
                    field,
                });
            }
        }
        let queries: Vec<String> = document
            .queries
            .iter()
            .map(|query| query.name.clone())
            .collect();
        if !queries.iter().any(|name| name == STATUS_QUERY) {
            return Err(EmbeddedAssistantError::MissingQuery { name: STATUS_QUERY });
        }

        Ok(Self {
            source,
            package,
            workflow_type,
            input_schema,
            signals,
            queries,
        })
    }

    /// The validated package the engine loads.
    #[must_use]
    pub const fn package(&self) -> &Package {
        &self.package
    }

    /// The workflow type an operator starts.
    #[must_use]
    pub fn workflow_type(&self) -> &str {
        &self.workflow_type
    }

    /// The package's content hash — this document's version identity.
    #[must_use]
    pub const fn content_hash(&self) -> &ContentHash {
        self.package.content_hash()
    }

    /// The document source, verbatim.
    #[must_use]
    pub const fn source(&self) -> &'static str {
        self.source
    }

    /// The derived JSON Schema of the start input.
    #[must_use]
    pub const fn input_schema(&self) -> &Value {
        &self.input_schema
    }

    /// Every declared signal with its payload schema.
    #[must_use]
    pub fn signals(&self) -> &[SignalContract] {
        &self.signals
    }

    /// Every declared query name, in document order.
    #[must_use]
    pub fn queries(&self) -> &[String] {
        &self.queries
    }

    /// The continuation signal's payload schema.
    ///
    /// Present by construction: [`Self::load`] refuses a document that does not
    /// declare [`CONTINUE_SIGNAL`].
    ///
    /// # Errors
    ///
    /// Returns [`EmbeddedAssistantError::MissingSignal`] if the signal set is
    /// ever mutated out from under construction — reported rather than assumed
    /// away.
    pub fn continuation_schema(&self) -> Result<&Value, EmbeddedAssistantError> {
        self.signals
            .iter()
            .find(|signal| signal.name == CONTINUE_SIGNAL)
            .map(|signal| &signal.input_schema)
            .ok_or(EmbeddedAssistantError::MissingSignal {
                name: CONTINUE_SIGNAL,
            })
    }
}

/// Whether `schema` declares `property` under `properties`.
///
/// A signal payload's derived schema for a NAMED type is a reference beside its
/// own definitions — `{"$ref": "#/$defs/Continuation", "$defs": {…}}` — so the
/// properties live one hop away. That one local form is followed; anything else
/// is left unresolved rather than guessed at, because a guess here would report
/// a field the payload does not carry and the operator verbs would send it.
fn schema_declares_property(schema: &Value, property: &str) -> bool {
    resolve_local_ref(schema)
        .and_then(|resolved| resolved.get("properties"))
        .and_then(Value::as_object)
        .is_some_and(|properties| properties.contains_key(property))
}

/// Follows a top-level `#/$defs/<name>` reference into the sibling `$defs` map.
/// A schema with no `$ref` is already the definition; an unresolvable reference
/// yields `None`.
fn resolve_local_ref(schema: &Value) -> Option<&Value> {
    let Some(reference) = schema.get("$ref").and_then(Value::as_str) else {
        return Some(schema);
    };
    let name = reference.strip_prefix("#/$defs/")?;
    schema.get("$defs")?.get(name)
}

/// The process-wide embedded assistant, compiled once on first use.
///
/// The document is compile-time constant, so its compiled form is too: every
/// caller (boot install, `/assistant`, the operator verbs) reads this one
/// value, and a failure is computed once and reported identically everywhere.
///
/// # Errors
///
/// Returns the [`EmbeddedAssistantError`] from the single load attempt.
pub fn embedded_assistant() -> Result<&'static EmbeddedAssistant, &'static EmbeddedAssistantError> {
    static EMBEDDED: OnceLock<Result<EmbeddedAssistant, EmbeddedAssistantError>> = OnceLock::new();
    EMBEDDED.get_or_init(EmbeddedAssistant::load).as_ref()
}

#[cfg(test)]
#[path = "document_tests.rs"]
mod document_tests;
