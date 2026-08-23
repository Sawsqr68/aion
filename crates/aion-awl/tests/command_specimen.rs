//! The specimen-fire layer: a declared refusal specimen is EXECUTED, and the
//! two ways executing it can go wrong.
//!
//! This layer exists because "this command refuses a leading dash" is a claim
//! a document makes about itself, and an unfired claim is a comment. It is
//! kept distinct from the emit layer on purpose: an emit refusal says a
//! parameter set was wrong, and a specimen failure says the DOCUMENT was
//! wrong about itself. Collapsing them would be the catch-all the
//! carrier-contract law forbids.

mod command;

use aion_awl::command::{CommandCause, CommandLayer, check_command, fire_specimen};

use command::specimens::{DIFF_CENSUS, GIT_TAG, document};

/// Parse, check, and fire the one command a document declares.
fn fire(command_text: &str) -> Result<(), (CommandCause, CommandLayer, String)> {
    let source = document(command_text);
    let parsed = match aion_awl::parse(&source) {
        Ok(parsed) => parsed,
        Err(error) => unreachable!("must parse: {}", error.message),
    };
    let Some(decl) = parsed.commands.first() else {
        unreachable!("the specimen declares one command");
    };
    let checked = match check_command(decl) {
        Ok(checked) => checked,
        Err(refusals) => unreachable!(
            "must check: {:#?}",
            refusals
                .iter()
                .map(aion_awl::command::CommandRefusal::rendered)
                .collect::<Vec<_>>()
        ),
    };
    fire_specimen(checked).map_err(|refusal| {
        (
            refusal.cause(),
            refusal.layer(),
            refusal.observed_bytes().to_owned(),
        )
    })
}

/// RED: a refusing invocation whose specimen the surface ACCEPTS. The
/// declaration has no operand for the specimen bytes to stand in, so the
/// refusal it claims can never fire — a protection that is written and does
/// nothing.
#[test]
fn a_specimen_that_is_accepted_fails_at_the_specimen_layer() {
    let refused = fire(
        "command c\n  program \"git\" \"diff\"\n  refuses \"--\"\n  specimen \"-x\"\n  \
         option \"--name-only\"",
    );
    match refused {
        Ok(()) => unreachable!("a specimen with no operand to stand in must not pass"),
        Err((cause, layer, observed)) => {
            assert_eq!(cause, CommandCause::SpecimenAccepted);
            assert_eq!(layer, CommandLayer::Specimen);
            assert_eq!(observed, "-x");
        }
    }
}

/// RED: a specimen whose bytes stop leading with a dash once the fill is
/// resolved. The emitted operand is `prefix-x`, which is not a leading-dash
/// operand at all, so the refusal the specimen stands for does not fire and
/// the claim is false.
#[test]
fn a_specimen_swallowed_by_its_fill_fails_at_the_specimen_layer() {
    let refused = fire(
        "command c(tail: String)\n  program \"git\" \"diff\"\n  refuses \"--\"\n  \
         specimen \"-x\"\n  operand r: String = \"prefix{{tail}}\"",
    );
    match refused {
        Ok(()) => unreachable!("a swallowed specimen must not pass"),
        Err((cause, _, observed)) => {
            assert_eq!(cause, CommandCause::SpecimenAccepted);
            assert_eq!(observed, "-x");
        }
    }
}

/// GREEN: the canonical refusing fixture's specimen fires and refuses, which
/// is the only shape that passes.
#[test]
fn the_canonical_refusing_fixtures_specimen_fires_and_refuses() {
    assert!(
        fire(DIFF_CENSUS).is_ok(),
        "the diff census declares that it refuses `--`, and firing its specimen must prove it"
    );
}

/// An ADMITTING invocation carries no specimen, so there is nothing to fire
/// and firing is trivially satisfied. Asserted rather than assumed, because
/// "nothing to do" and "did nothing" must not read alike.
#[test]
fn an_admitting_invocation_has_no_specimen_to_fire() {
    assert!(
        fire(GIT_TAG).is_ok(),
        "the admitting cure carries no specimen; firing must succeed vacuously"
    );
}

/// `specimen_wrong_cause` — SHADOWED BY CONSTRUCTION, recorded honestly.
///
/// The cause distinguishes "the specimen did not refuse" from "the specimen
/// refused for the wrong reason". Its red arm is UNREACHABLE in this cut, and
/// the reason is structural rather than an oversight: [`fire_specimen`] builds
/// a parameter set that binds every declared parameter with a value shaped to
/// its declared type, so the other three emit-layer causes
/// (`argument_missing`, `argument_undeclared`, `argument_type_mismatch`)
/// cannot arise during a firing, and `leading_dash_operand` is precisely the
/// cause the specimen stands for.
///
/// The code is kept rather than removed, because collapsing "did not refuse"
/// into "refused wrongly" is exactly the catch-all the layer-naming law
/// forbids, and the emit vocabulary is expected to grow. What is asserted
/// here is what CAN be asserted: the code is constructible, renders its own
/// wire string, and is filed at the specimen layer. This shadowing is
/// reported as a finding rather than hidden — the same honesty the rig's own
/// probe battery applies to its two unreachable arms.
#[test]
fn the_wrong_cause_code_is_filed_at_the_specimen_layer_though_its_red_arm_is_shadowed() {
    assert_eq!(
        CommandCause::SpecimenWrongCause.layer(),
        CommandLayer::Specimen
    );
    assert_eq!(
        CommandCause::SpecimenWrongCause.as_str(),
        "specimen_wrong_cause"
    );
    assert_eq!(
        CommandCause::from_code("specimen_wrong_cause"),
        Some(CommandCause::SpecimenWrongCause)
    );
}
