//! Four layer error types, and the constructor discipline that keeps them
//! apart.
//!
//! The carrier-contract law says a parse failure, a check refusal, an emit
//! refusal, and a specimen-fire failure are different layers with different
//! causes. A law stated only in prose is a law that gets broken by the next
//! edit, so it is enforced by types here:
//!
//! * each layer has its OWN error type — there is no shared `CommandError`
//!   with a layer field somebody can set wrongly;
//! * each constructor is fallible and refuses a cause belonging to another
//!   layer, returning [`CauseMisfiled`] rather than panicking;
//! * each constructor refuses an empty diagnostic. A refusal that says
//!   nothing is a catch-all wearing a specific cause's name.
//!
//! There is deliberately no catch-all cause to refuse: [`CommandCause`] is a
//! closed enum with no `Other`, so the whole class is unrepresentable rather
//! than merely rejected. What the constructors add is the guarantee that a
//! representable cause cannot be filed at the wrong layer.
//!
//! Every refusal carries `observed_bytes` — the bytes actually seen, not a
//! paraphrase of them. That is what makes a refusal reviewable against the
//! document instead of against someone's memory of it.

use crate::Span;

use super::cause::{CommandCause, CommandLayer};

/// A constructor was handed a cause that does not belong to its layer, or a
/// diagnostic that says nothing.
///
/// Returned, never panicked: library code in this estate does not `unwrap`,
/// `expect`, or `panic`, and a misfiled cause is a programming error the
/// caller can see and a test can assert on.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CauseMisfiled {
    /// The cause belongs to a different layer than the error being built.
    #[error(
        "cause `{cause}` belongs to the {actual} layer and cannot be carried by a {attempted} \
         refusal — a parse failure, a check refusal, an emit refusal and a specimen-fire failure \
         are different layers with different causes"
    )]
    WrongLayer {
        /// The cause the caller supplied.
        cause: CommandCause,
        /// The layer that cause actually belongs to.
        actual: CommandLayer,
        /// The layer the caller was building a refusal for.
        attempted: CommandLayer,
    },
    /// The diagnostic text was empty or whitespace.
    #[error(
        "a {attempted} refusal naming `{cause}` was built with an empty diagnostic; a refusal \
         that says nothing is a catch-all wearing a specific cause's name"
    )]
    EmptyDiagnostic {
        /// The cause the caller supplied.
        cause: CommandCause,
        /// The layer the caller was building a refusal for.
        attempted: CommandLayer,
    },
}

/// The parts every layer's refusal carries, so the four types cannot drift
/// into carrying different evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Refusal {
    cause: CommandCause,
    span: Span,
    observed_bytes: String,
    message: String,
}

impl Refusal {
    /// Build a refusal for `layer`, or say why it cannot be built.
    fn build(
        layer: CommandLayer,
        cause: CommandCause,
        span: Span,
        observed_bytes: impl Into<String>,
        message: impl Into<String>,
    ) -> Result<Self, CauseMisfiled> {
        if cause.layer() != layer {
            return Err(CauseMisfiled::WrongLayer {
                cause,
                actual: cause.layer(),
                attempted: layer,
            });
        }
        let message = message.into();
        if message.trim().is_empty() {
            return Err(CauseMisfiled::EmptyDiagnostic {
                cause,
                attempted: layer,
            });
        }
        Ok(Self {
            cause,
            span,
            observed_bytes: observed_bytes.into(),
            message,
        })
    }

    /// The rendered diagnostic: what went wrong, the bytes seen, and the
    /// stable cause an operator or a fixture matches on.
    fn render(&self) -> String {
        format!(
            "{} (observed: {:?}; named cause: {})",
            self.message, self.observed_bytes, self.cause
        )
    }
}

/// Declares one layer's error type over [`Refusal`].
///
/// A macro rather than four hand-written copies because the four types must
/// be structurally identical in everything except their layer — a hand-copied
/// fourth type is exactly where the fifth field, or the missing empty-message
/// check, would appear.
macro_rules! layer_error {
    ($name:ident, $layer:expr, $what:literal, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
        #[error("{}", .0.render())]
        pub struct $name(Refusal);

        impl $name {
            #[doc = concat!("Build a ", $what, " refusal.")]
            ///
            /// # Errors
            ///
            /// Returns [`CauseMisfiled`] when `cause` belongs to another
            /// layer, or when `message` is empty or whitespace.
            pub fn new(
                cause: CommandCause,
                span: Span,
                observed_bytes: impl Into<String>,
                message: impl Into<String>,
            ) -> Result<Self, CauseMisfiled> {
                Refusal::build($layer, cause, span, observed_bytes, message).map(Self)
            }

            /// The stable cause code this refusal names.
            #[must_use]
            pub const fn cause(&self) -> CommandCause {
                self.0.cause
            }

            /// The layer this refusal came from. Constant by construction.
            #[must_use]
            pub const fn layer(&self) -> CommandLayer {
                $layer
            }

            /// Where in the source the refusal lands.
            #[must_use]
            pub const fn span(&self) -> Span {
                self.0.span
            }

            /// The bytes actually seen, verbatim.
            #[must_use]
            pub fn observed_bytes(&self) -> &str {
                &self.0.observed_bytes
            }

            /// The diagnostic without the cause and byte suffix.
            #[must_use]
            pub fn message(&self) -> &str {
                &self.0.message
            }

            /// The full rendered diagnostic, as a surface prints it.
            #[must_use]
            pub fn rendered(&self) -> String {
                self.0.render()
            }
        }
    };
}

layer_error!(
    CommandParseError,
    CommandLayer::Parse,
    "parse-layer",
    "The bytes are not a command declaration of this language.\n\nA form refused here is \
     UNREPRESENTABLE — the grammar has no production for it — rather than discouraged by a \
     later check."
);
layer_error!(
    CommandCheckError,
    CommandLayer::Check,
    "check-layer",
    "The declaration parses, and what it says is wrong.\n\nEvery refusal at this layer names a \
     span in the document the author wrote and the bytes it read there."
);
layer_error!(
    CommandEmitError,
    CommandLayer::Emit,
    "emit-layer",
    "A parameter set, or the argv it would produce, is refused.\n\nThe declaration itself is \
     sound; this is the refusal that keeps a sound declaration from emitting an unsound argv."
);
layer_error!(
    CommandSpecimenError,
    CommandLayer::Specimen,
    "specimen-fire-layer",
    "A declared refusal specimen was fired and did not refuse, or refused for the wrong \
     reason.\n\nThis is how a MANDATORY specimen proves it is load-bearing rather than \
     decorative: the specimen is executed, not merely written."
);

/// A command-surface refusal, whichever layer produced it.
///
/// Callers that funnel every layer into one diagnostic stream — `aion awl
/// check` is the first — need one type to carry. Erasing the layer for
/// TRANSPORT is not the same as conflating the layers at construction: the
/// layer and the cause both survive, and [`CommandRefusal::layer`] still
/// answers which discipline produced it.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CommandRefusal {
    /// The bytes are not a command declaration of this language.
    #[error(transparent)]
    Parse(#[from] CommandParseError),
    /// The declaration parses and what it says is wrong.
    #[error(transparent)]
    Check(#[from] CommandCheckError),
    /// A parameter set, or the argv it would produce, is refused.
    #[error(transparent)]
    Emit(#[from] CommandEmitError),
    /// A declared refusal specimen did not refuse, or refused wrongly.
    #[error(transparent)]
    Specimen(#[from] CommandSpecimenError),
}

impl CommandRefusal {
    /// Build a refusal whose LAYER IS DERIVED FROM ITS CAUSE.
    ///
    /// This is the door every refusal inside this surface comes through, and
    /// it is total: there is no way to hand it a cause and get a refusal
    /// filed at a different layer, because the caller does not choose the
    /// layer at all — [`CommandCause::layer`] does. Misfiling is therefore
    /// not a mistake that is caught, it is a mistake that cannot be written.
    ///
    /// The fallible per-layer constructors ([`CommandParseError::new`] and
    /// its siblings) remain the door for callers OUTSIDE this surface, who do
    /// name a layer and must be told when they name the wrong one.
    ///
    /// An empty message is replaced by the cause's own wire string rather
    /// than accepted: a refusal always says something, and this function
    /// cannot fail.
    pub(super) fn refuse(
        cause: CommandCause,
        span: Span,
        observed_bytes: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        let message = message.into();
        let message = if message.trim().is_empty() {
            cause.as_str().to_owned()
        } else {
            message
        };
        let refusal = Refusal {
            cause,
            span,
            observed_bytes: observed_bytes.into(),
            message,
        };
        match cause.layer() {
            CommandLayer::Parse => Self::Parse(CommandParseError(refusal)),
            CommandLayer::Check => Self::Check(CommandCheckError(refusal)),
            CommandLayer::Emit => Self::Emit(CommandEmitError(refusal)),
            CommandLayer::Specimen => Self::Specimen(CommandSpecimenError(refusal)),
        }
    }

    /// The stable cause code this refusal names.
    #[must_use]
    pub const fn cause(&self) -> CommandCause {
        match self {
            Self::Parse(error) => error.cause(),
            Self::Check(error) => error.cause(),
            Self::Emit(error) => error.cause(),
            Self::Specimen(error) => error.cause(),
        }
    }

    /// Which layer refused. Read from the variant, not from the cause, so a
    /// disagreement between the two is observable rather than assumed away.
    #[must_use]
    pub const fn layer(&self) -> CommandLayer {
        match self {
            Self::Parse(_) => CommandLayer::Parse,
            Self::Check(_) => CommandLayer::Check,
            Self::Emit(_) => CommandLayer::Emit,
            Self::Specimen(_) => CommandLayer::Specimen,
        }
    }

    /// Where in the source the refusal lands.
    #[must_use]
    pub const fn span(&self) -> Span {
        match self {
            Self::Parse(error) => error.span(),
            Self::Check(error) => error.span(),
            Self::Emit(error) => error.span(),
            Self::Specimen(error) => error.span(),
        }
    }

    /// The bytes actually seen, verbatim.
    #[must_use]
    pub fn observed_bytes(&self) -> &str {
        match self {
            Self::Parse(error) => error.observed_bytes(),
            Self::Check(error) => error.observed_bytes(),
            Self::Emit(error) => error.observed_bytes(),
            Self::Specimen(error) => error.observed_bytes(),
        }
    }

    /// The diagnostic without the cause and byte suffix.
    #[must_use]
    pub fn message(&self) -> &str {
        match self {
            Self::Parse(error) => error.message(),
            Self::Check(error) => error.message(),
            Self::Emit(error) => error.message(),
            Self::Specimen(error) => error.message(),
        }
    }

    /// The full rendered diagnostic, as a surface prints it.
    #[must_use]
    pub fn rendered(&self) -> String {
        match self {
            Self::Parse(error) => error.rendered(),
            Self::Check(error) => error.rendered(),
            Self::Emit(error) => error.rendered(),
            Self::Specimen(error) => error.rendered(),
        }
    }
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod error_tests;
