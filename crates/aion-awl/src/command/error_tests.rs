//! What the four layer types and their constructor discipline must satisfy.
//!
//! RED FIRST: the constructor's refusals are written and asserted before any
//! passing construction below them, because a constructor that cannot refuse
//! is a constructor that admits anything.

use crate::Span;

use super::super::cause::{CommandCause, CommandLayer};
use super::{
    CauseMisfiled, CommandCheckError, CommandComposeError, CommandEmitError, CommandFireError,
    CommandParseError, CommandRefusal, CommandSpecimenError,
};

fn span() -> Span {
    Span {
        start: 10,
        end: 20,
        line: 3,
        column: 5,
    }
}

/// RED: a parse-layer constructor handed a check-layer cause returns a typed
/// error naming both layers. No panic, no silent relabelling.
#[test]
fn a_constructor_fed_a_cause_from_another_layer_returns_a_typed_error() {
    let refused = CommandParseError::new(
        CommandCause::LiteralWhereDerived,
        span(),
        "abc",
        "a message",
    );
    assert_eq!(
        refused,
        Err(CauseMisfiled::WrongLayer {
            cause: CommandCause::LiteralWhereDerived,
            actual: CommandLayer::Check,
            attempted: CommandLayer::Parse,
        })
    );

    let refused = CommandCheckError::new(
        CommandCause::BacktickSubstitution,
        span(),
        "`x`",
        "a message",
    );
    assert!(matches!(
        refused,
        Err(CauseMisfiled::WrongLayer {
            actual: CommandLayer::Parse,
            attempted: CommandLayer::Check,
            ..
        })
    ));

    let refused = CommandEmitError::new(CommandCause::BodyEmpty, span(), "", "a message");
    assert!(matches!(refused, Err(CauseMisfiled::WrongLayer { .. })));

    let refused = CommandSpecimenError::new(CommandCause::ArgumentMissing, span(), "", "a message");
    assert!(matches!(refused, Err(CauseMisfiled::WrongLayer { .. })));
}

/// RED: an empty diagnostic is refused. A refusal that says nothing is a
/// catch-all wearing a specific cause's name.
#[test]
fn a_constructor_fed_an_empty_diagnostic_returns_a_typed_error() {
    for message in ["", "   ", "\n\t "] {
        assert_eq!(
            CommandParseError::new(CommandCause::BodyEmpty, span(), "", message),
            Err(CauseMisfiled::EmptyDiagnostic {
                cause: CommandCause::BodyEmpty,
                attempted: CommandLayer::Parse,
            }),
            "an empty diagnostic must be refused: {message:?}"
        );
    }
}

/// EVERY cause is constructible at its OWN layer, so the refusals above are
/// about misfiling and not about a layer that refuses everything.
#[test]
fn every_cause_constructs_at_its_own_layer_and_nowhere_else() {
    for cause in CommandCause::ALL {
        let built =
            match cause.layer() {
                CommandLayer::Parse => {
                    CommandParseError::new(cause, span(), "bytes", "why").map(CommandRefusal::from)
                }
                CommandLayer::Check => {
                    CommandCheckError::new(cause, span(), "bytes", "why").map(CommandRefusal::from)
                }
                CommandLayer::Emit => {
                    CommandEmitError::new(cause, span(), "bytes", "why").map(CommandRefusal::from)
                }
                CommandLayer::Compose => CommandComposeError::new(cause, span(), "bytes", "why")
                    .map(CommandRefusal::from),
                CommandLayer::Fire => {
                    CommandFireError::new(cause, span(), "bytes", "why").map(CommandRefusal::from)
                }
                CommandLayer::Specimen => CommandSpecimenError::new(cause, span(), "bytes", "why")
                    .map(CommandRefusal::from),
            };
        let built = built.unwrap_or_else(|misfiled| {
            unreachable!("{cause} must construct at its own layer: {misfiled}")
        });
        assert_eq!(built.cause(), cause);
        assert_eq!(built.layer(), cause.layer());
        assert_eq!(built.observed_bytes(), "bytes");
        assert_eq!(built.span(), span());

        for wrong in CommandLayer::ALL {
            if wrong == cause.layer() {
                continue;
            }
            let attempted = match wrong {
                CommandLayer::Parse => {
                    CommandParseError::new(cause, span(), "bytes", "why").map(CommandRefusal::from)
                }
                CommandLayer::Check => {
                    CommandCheckError::new(cause, span(), "bytes", "why").map(CommandRefusal::from)
                }
                CommandLayer::Emit => {
                    CommandEmitError::new(cause, span(), "bytes", "why").map(CommandRefusal::from)
                }
                CommandLayer::Compose => CommandComposeError::new(cause, span(), "bytes", "why")
                    .map(CommandRefusal::from),
                CommandLayer::Fire => {
                    CommandFireError::new(cause, span(), "bytes", "why").map(CommandRefusal::from)
                }
                CommandLayer::Specimen => CommandSpecimenError::new(cause, span(), "bytes", "why")
                    .map(CommandRefusal::from),
            };
            assert!(
                attempted.is_err(),
                "{cause} belongs to the {} layer and must not construct at the {wrong} layer",
                cause.layer()
            );
        }
    }
}

/// The internal door derives the layer from the cause, so misfiling is not
/// caught — it is unwritable. Every cause files itself correctly through it.
#[test]
fn the_internal_door_files_every_cause_at_its_own_layer() {
    for cause in CommandCause::ALL {
        let refusal = CommandRefusal::refuse(cause, span(), "observed", "why");
        assert_eq!(refusal.layer(), cause.layer(), "{cause} was misfiled");
        assert_eq!(refusal.cause(), cause);
    }
}

/// The internal door cannot produce a silent refusal either: an empty message
/// is replaced by the cause's own code rather than accepted.
#[test]
fn the_internal_door_never_produces_a_silent_refusal() {
    let refusal = CommandRefusal::refuse(CommandCause::BodyEmpty, span(), "", "   ");
    assert_eq!(refusal.message(), CommandCause::BodyEmpty.as_str());
    assert!(!refusal.rendered().is_empty());
}

/// A rendered refusal carries the bytes it observed and the cause it names,
/// because a diagnostic that carries neither cannot be checked against the
/// document it came from.
#[test]
fn a_rendered_refusal_carries_its_observed_bytes_and_its_cause() {
    let refusal = CommandRefusal::refuse(
        CommandCause::LiteralWhereDerived,
        span(),
        "5e2fb5a3ed32b53d48dbc585aa914acd056458c6",
        "a base pin is derived, never written by hand",
    );
    let rendered = refusal.rendered();
    assert!(
        rendered.contains("5e2fb5a3ed32b53d48dbc585aa914acd056458c6"),
        "{rendered}"
    );
    assert!(rendered.contains("literal_where_derived"), "{rendered}");
    assert!(
        rendered.contains("a base pin is derived, never written by hand"),
        "{rendered}"
    );
}
