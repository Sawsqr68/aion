//! Firing a declared refusal specimen.
//!
//! A refusing invocation carries a specimen because "this command refuses a
//! leading dash" is a claim, and an unfired claim is a comment. Firing it
//! runs the specimen bytes through the real emitter, in the real operand
//! position, and asserts the real refusal — so what protects the invocation
//! is tested rather than assumed.
//!
//! This is its own LAYER. A specimen that does not refuse is not an emit
//! failure and not a check failure: it is a claim the document made about
//! itself that turned out to be false, and it gets its own causes so a
//! reader can tell the three apart.

use super::cause::CommandCause;
use super::check::CheckedCommand;
use super::decl::CommandParam;
use super::emit::{ArgumentValue, ParameterSet, emit};
use super::error::CommandRefusal;

/// Fire the declaration's leading-dash refusal specimen.
///
/// Builds a parameter set in which every declared parameter carries the
/// specimen bytes, emits, and requires the emitter to refuse with
/// [`CommandCause::LeadingDashOperand`]. An admitting invocation carries no
/// specimen and there is nothing to fire, so it returns `Ok(())`.
///
/// # Errors
///
/// Returns a specimen-layer refusal when the specimen was accepted, or was
/// refused for some other reason than the one it stands for.
pub fn fire_specimen(command: CheckedCommand<'_>) -> Result<(), CommandRefusal> {
    let Some(specimen) = command.specimen() else {
        return Ok(());
    };
    let decl = command.decl();
    let mut parameters = ParameterSet::new();
    for parameter in &decl.parameters {
        parameters = parameters.with(&parameter.name, specimen_value(parameter, &specimen.bytes));
    }

    match emit(command, &parameters) {
        Err(refusal) if refusal.cause() == CommandCause::LeadingDashOperand => Ok(()),
        Err(refusal) => Err(CommandRefusal::refuse(
            CommandCause::SpecimenWrongCause,
            specimen.span,
            &specimen.bytes,
            format!(
                "command `{}`'s leading-dash specimen was refused as `{}`, not as `{}`; the \
                 specimen stands for the leading-dash family and a refusal for another reason \
                 leaves that family untested",
                decl.name,
                refusal.cause(),
                CommandCause::LeadingDashOperand
            ),
        )),
        Ok(resolved) => Err(CommandRefusal::refuse(
            CommandCause::SpecimenAccepted,
            specimen.span,
            &specimen.bytes,
            format!(
                "command `{}` declares that it refuses `--`, and its specimen `{}` was ACCEPTED, \
                 emitting {:?}. The invocation has no end-of-options marker, so that argv would \
                 be read with an operand in option position",
                decl.name, specimen.bytes, resolved.argv
            ),
        )),
    }
}

/// The specimen bytes shaped to the parameter's declared type, so that
/// firing exercises the operand position rather than tripping over an
/// unrelated type mismatch on the way there.
fn specimen_value(parameter: &CommandParam, bytes: &str) -> ArgumentValue {
    if matches!(parameter.ty, crate::ast::TypeRef::List { .. }) {
        ArgumentValue::list([bytes])
    } else {
        ArgumentValue::scalar(bytes)
    }
}
