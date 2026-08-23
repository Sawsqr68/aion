//! Every cause code in the closed vocabulary has a refusing specimen, and
//! every specimen refuses with the cause and the bytes it claims.
//!
//! This is the red-first duty, mechanised. A cause code is a promise that the
//! surface will refuse a particular thing; a promise with no specimen is a
//! promise nothing keeps. `every_cause_has_a_refusing_specimen` iterates the
//! ENUM — not a hand-kept list — so a cause added without a specimen fails
//! the build.

mod command;

use std::collections::BTreeSet;

use aion_awl::command::{CommandCause, CommandLayer};

use command::specimens::document;

/// One refusing specimen: the cause it must produce, and the document that
/// produces it.
pub struct Specimen {
    /// The cause the surface must name.
    pub cause: CommandCause,
    /// A short label naming what is wrong, for a failure message.
    pub what: &'static str,
    /// The command declaration text.
    pub command: &'static str,
    /// A byte run the refusal must carry as its observed bytes, or `None`
    /// when the specimen only pins the cause.
    pub observed: Option<&'static str>,
}

/// Every cause code's refusing specimen.
///
/// The list is checked for COVERAGE against `CommandCause::ALL`, so a cause
/// added to the vocabulary without a specimen fails the build rather than
/// going quietly untested.
pub const SPECIMENS: &[Specimen] = &[
    // ---------------------------------------------------------- parse layer
    Specimen {
        cause: CommandCause::ClauseUnknown,
        what: "a word that is not a clause",
        command: "command c\n  program \"git\"\n  admits \"--\"\n  wibble \"x\"",
        observed: Some("wibble"),
    },
    Specimen {
        cause: CommandCause::BodyEmpty,
        what: "a declaration that states nothing",
        command: "command c",
        observed: Some("c"),
    },
    Specimen {
        cause: CommandCause::RecipeFormRefused,
        what: "a recipe form in a gate-leg declaration",
        command: "command c\n  program \"git\"\n  admits \"--\"\n  recipe \"build\"",
        observed: Some("recipe"),
    },
    Specimen {
        cause: CommandCause::RecipeFormRefused,
        what: "a dependency edge spelled with a RESERVED word",
        command: "command c\n  program \"git\"\n  admits \"--\"\n  after \"other\"",
        observed: Some("after"),
    },
    Specimen {
        cause: CommandCause::EnvStringPrefix,
        what: "a K=V string-prefix environment binding",
        command: "command c\n  program \"git\"\n  admits \"--\"\n  env \"PAGER=cat\"",
        observed: Some("PAGER=cat"),
    },
    Specimen {
        cause: CommandCause::BacktickSubstitution,
        what: "backtick command substitution in a fill",
        command: "command c(a: String)\n  program \"git\"\n  admits \"--\"\n  \
                  operand x: String = \"`git rev-parse HEAD`\"",
        observed: Some("`git rev-parse HEAD`"),
    },
    Specimen {
        cause: CommandCause::InterpolationUnterminated,
        what: "an interpolation that never closes",
        command: "command c(a: String)\n  program \"git\"\n  admits \"--\"\n  \
                  operand x: String = \"{{a\"",
        observed: Some("{{a"),
    },
    Specimen {
        cause: CommandCause::InterpolationEmpty,
        what: "an interpolation naming nothing",
        command: "command c\n  program \"git\"\n  admits \"--\"\n  \
                  operand x: String = \"{{}}\"",
        observed: Some("{{}}"),
    },
    Specimen {
        cause: CommandCause::InterpolationMalformed,
        what: "an expression inside an interpolation",
        command: "command c\n  program \"git\"\n  admits \"--\"\n  \
                  operand x: String = \"{{if a == b}}\"",
        observed: Some("{{if a == b}}"),
    },
    Specimen {
        cause: CommandCause::EndOfOptionsMarkerWrong,
        what: "an end-of-options marker that is not `--`",
        command: "command c\n  program \"git\"\n  admits \"-\"",
        observed: Some("-"),
    },
    Specimen {
        cause: CommandCause::ProgramWordsEmpty,
        what: "a program clause naming no words",
        command: "command c\n  program\n  admits \"--\"",
        observed: Some("program"),
    },
    // ---------------------------------------------------------- check layer
    Specimen {
        cause: CommandCause::ProgramUnstated,
        what: "a declaration with no program clause",
        command: "command c\n  admits \"--\"",
        observed: Some("c"),
    },
    Specimen {
        cause: CommandCause::EndOfOptionsUnstated,
        what: "a declaration in the unstated middle",
        command: "command c\n  program \"git\"",
        observed: Some("c"),
    },
    Specimen {
        cause: CommandCause::EndOfOptionsRestated,
        what: "the end-of-options property stated twice",
        command: "command c\n  program \"git\"\n  admits \"--\"\n  refuses \"--\"",
        observed: Some("refuses"),
    },
    Specimen {
        cause: CommandCause::LeadingDashSpecimenMissing,
        what: "a refusing invocation with no specimen",
        command: "command c\n  program \"git\" \"diff\"\n  refuses \"--\"",
        observed: Some("refuses"),
    },
    Specimen {
        cause: CommandCause::LeadingDashSpecimenNotLeading,
        what: "a specimen that does not lead with a dash",
        command: "command c\n  program \"git\" \"diff\"\n  refuses \"--\"\n  specimen \"x\"",
        observed: Some("x"),
    },
    Specimen {
        cause: CommandCause::ParameterUndeclared,
        what: "an interpolation naming no declared parameter",
        command: "command c\n  program \"git\"\n  admits \"--\"\n  \
                  operand x: String = \"{{nowhere}}\"",
        observed: Some("{{nowhere}}"),
    },
    Specimen {
        cause: CommandCause::ParameterTypeMismatch,
        what: "an operand typed differently from the parameter filling it",
        command: "command c(n: Int)\n  program \"git\"\n  admits \"--\"\n  \
                  operand x: String = \"{{n}}\"",
        observed: Some("{{n}}"),
    },
    Specimen {
        cause: CommandCause::LiteralWhereDerived,
        what: "a hand-written base pin where a derivation exists",
        command: "command c\n  program \"git\" \"diff\"\n  refuses \"--\"\n  specimen \"-x\"\n  \
                  operand range: String = \"5e2fb5a3ed32b53d48dbc585aa914acd056458c6..HEAD\"",
        observed: Some("5e2fb5a3ed32b53d48dbc585aa914acd056458c6"),
    },
    Specimen {
        cause: CommandCause::HardeningRestated,
        what: "hardening stated twice in one scope",
        command: "command c\n  program \"git\"\n  admits \"--\"\n  hardening\n    open_files 1024\n  \
                  hardening\n    open_files 2048",
        observed: Some("hardening"),
    },
    Specimen {
        cause: CommandCause::HardeningEmpty,
        what: "a hardening block that hardens nothing",
        command: "command c\n  program \"git\"\n  admits \"--\"\n  hardening",
        observed: Some("hardening"),
    },
    Specimen {
        cause: CommandCause::CwdPlaceholderUnknown,
        what: "a working directory reaching for a second placeholder",
        command: "command c\n  program \"git\"\n  admits \"--\"\n  cwd \"{repo_root}/src\"",
        observed: Some("{repo_root}"),
    },
    Specimen {
        cause: CommandCause::ParameterRestated,
        what: "two parameters sharing a name",
        command: "command c(a: String, a: String)\n  program \"git\"\n  admits \"--\"",
        observed: Some("a"),
    },
    Specimen {
        cause: CommandCause::OperandRestated,
        what: "two operands sharing a name",
        command: "command c\n  program \"git\"\n  admits \"--\"\n  operand x: String = \"one\"\n  \
                  operand x: String = \"two\"",
        observed: Some("x"),
    },
    Specimen {
        cause: CommandCause::OptionRestated,
        what: "two options sharing a flag",
        command: "command c\n  program \"git\"\n  admits \"--\"\n  option \"--name-only\"\n  \
                  option \"--name-only\"",
        observed: Some("--name-only"),
    },
    Specimen {
        cause: CommandCause::EnvRestated,
        what: "two environment bindings sharing a name",
        command: "command c\n  program \"git\"\n  admits \"--\"\n  env PAGER = \"cat\"\n  \
                  env PAGER = \"less\"",
        observed: Some("PAGER"),
    },
    Specimen {
        cause: CommandCause::TimeoutOwnerEmpty,
        what: "a declared timeout nobody owns",
        command: "command c\n  program \"git\"\n  admits \"--\"\n  timeout 30s owner \"\"",
        observed: Some(""),
    },
    Specimen {
        cause: CommandCause::SpecimenWithoutRefusal,
        what: "a specimen on an admitting invocation",
        command: "command c\n  program \"git\" \"tag\"\n  admits \"--\"\n  specimen \"-x\"",
        observed: Some("-x"),
    },
    Specimen {
        cause: CommandCause::ListInScalarPosition,
        what: "a list parameter written into a larger word",
        command: "command c(items: [String])\n  program \"git\"\n  admits \"--\"\n  \
                  operand x: String = \"prefix-{{items}}\"",
        observed: Some("prefix-{{items}}"),
    },
    Specimen {
        cause: CommandCause::CommandRestated,
        what: "two command declarations sharing a name",
        command: "command c\n  program \"git\"\n  admits \"--\"\n\n\
                  command c\n  program \"git\"\n  admits \"--\"",
        observed: Some("c"),
    },
    // -------------------------------------------- parse layer (assembly)
    Specimen {
        cause: CommandCause::RecipeBodyEmpty,
        what: "a recipe that states nothing",
        command: "recipe stage",
        observed: Some("stage"),
    },
    Specimen {
        cause: CommandCause::AssemblyClauseUnknown,
        what: "a word that is not a recipe clause",
        command: "recipe stage\n  wibble \"x\"",
        observed: Some("wibble"),
    },
    Specimen {
        cause: CommandCause::ShellStringBody,
        what: "a recipe step body written as a shell string",
        command: "recipe stage\n  step build\n    runs \"cargo test --workspace\"",
        observed: Some("cargo test --workspace"),
    },
    Specimen {
        cause: CommandCause::CommandDeclarationNested,
        what: "a command declaration nested inside a recipe body",
        command: "recipe stage\n  command inner",
        observed: Some("command"),
    },
    Specimen {
        cause: CommandCause::ParameterKindUnknown,
        what: "a parameter kind outside the closed taxonomy",
        command: "recipe stage\n  parameter branch: String nonesuch",
        observed: Some("nonesuch"),
    },
    // -------------------------------------------- check layer (assembly)
    Specimen {
        cause: CommandCause::DependencyCycle,
        what: "dependency edges that close a cycle",
        command: "recipe stage\n  step first\n    after second\n    runs command push_branch\n  \
                  step second\n    after first\n    runs command push_branch",
        observed: Some("first -> second -> first"),
    },
    Specimen {
        cause: CommandCause::ReferenceUndeclared,
        what: "a dependency edge naming a step the recipe does not declare",
        command: "recipe stage\n  step push\n    after nowhere\n    runs command push_branch",
        observed: Some("nowhere"),
    },
    Specimen {
        cause: CommandCause::DerivationMissing,
        what: "a re-derive parameter with no declared derivation",
        command: "recipe stage\n  parameter control_expect: String re_derive\n  step push\n    \
                  runs command push_branch",
        observed: Some("re_derive"),
    },
    Specimen {
        cause: CommandCause::ExtractionUnknown,
        what: "a derivation naming an extraction this surface does not perform",
        command: "recipe stage\n  parameter control_expect: String re_derive derives \"origin/main:f\" \
                  extract \"the third paragraph\"\n  step push\n    runs command push_branch",
        observed: Some("the third paragraph"),
    },
    Specimen {
        cause: CommandCause::DoubleCarry,
        what: "two parameters declaring the same truth",
        command: "recipe stage\n  parameter leg: String = \"build\" skeleton\n  \
                  parameter leg_ids: String = \"build\" skeleton\n  step push\n    \
                  runs command push_branch",
        observed: Some("build"),
    },
    Specimen {
        cause: CommandCause::VenueValueRestated,
        what: "a recipe restating a value the dispatch already declares",
        command: "recipe stage\n  parameter workspace_root: String = \"/home/aion/fleet/runs\" \
                  skeleton\n  step push\n    runs command push_branch",
        observed: Some("/home/aion/fleet/runs"),
    },
    Specimen {
        cause: CommandCause::ArtifactDigestMalformed,
        what: "an artifact digest that is not a content digest",
        command: "artifact ground \"OPERATING-GROUND.md\"\n  digest \"not-a-digest\"",
        observed: Some("not-a-digest"),
    },
    Specimen {
        cause: CommandCause::WallAllowsNothing,
        what: "a wall that admits no path at all",
        command: "wall changed_paths\n  excludes \"mutation vocabulary\"",
        observed: Some("changed_paths"),
    },
    Specimen {
        cause: CommandCause::RecipeStepRestated,
        what: "two steps of one recipe sharing a name",
        command: "recipe stage\n  step push\n    runs command push_branch\n  step push\n    \
                  runs command push_branch",
        observed: Some("push"),
    },
    Specimen {
        cause: CommandCause::AssemblyDeclarationRestated,
        what: "two walls sharing a name",
        command: "wall changed_paths\n  allows \".fleet/\"\n\nwall changed_paths\n  \
                  allows \"crates/\"",
        observed: Some("changed_paths"),
    },
    Specimen {
        cause: CommandCause::RecipeStatesNoStep,
        what: "a recipe that assembles nothing",
        command: "recipe stage\n  parameter branch: String per_item",
        observed: Some("stage"),
    },
];

/// The causes whose refusals are raised by the EMIT, COMPOSE, FIRE and
/// SPECIMEN layers, which take a parameter set, a composition, or a tree
/// rather than a document, and are therefore specimened directly in
/// `command_emit.rs`, `command_specimen.rs`, `recipe_compose.rs`,
/// `recipe_wall.rs` and `recipe_fire.rs` rather than through a document here.
///
/// `specimen_wrong_cause` is among them and is additionally SHADOWED: its red
/// arm is unreachable in this cut's emit vocabulary, which
/// `command_specimen.rs` records in full rather than hiding.
pub const NON_DOCUMENT_CAUSES: [CommandCause; 15] = [
    CommandCause::ArgumentMissing,
    CommandCause::ArgumentUndeclared,
    CommandCause::ArgumentTypeMismatch,
    CommandCause::LeadingDashOperand,
    CommandCause::SlotUnfilled,
    CommandCause::ExcludedTerm,
    CommandCause::CensusMismatch,
    CommandCause::ArtifactUnreadable,
    CommandCause::RestoreInexact,
    CommandCause::StageCompareMismatch,
    CommandCause::ObjectMovedBeforeClaim,
    CommandCause::PreconditionUnmet,
    CommandCause::StepCommandFailed,
    CommandCause::SpecimenAccepted,
    CommandCause::SpecimenWrongCause,
];

/// What the surface said about a document: the first refusal's rendered text,
/// from whichever layer produced it.
fn refusal_text(source: &str) -> Option<String> {
    match aion_awl::parse(source) {
        Err(error) => Some(error.message),
        Ok(parsed) => {
            let root = std::env::temp_dir();
            aion_awl::check_in(&parsed, &root)
                .into_iter()
                .map(|error| error.message)
                .next()
        }
    }
}

/// Every refusal the surface produced for a document, rendered.
fn all_refusals(source: &str) -> Vec<String> {
    match aion_awl::parse(source) {
        Err(error) => vec![error.message],
        Ok(parsed) => {
            let root = std::env::temp_dir();
            aion_awl::check_in(&parsed, &root)
                .into_iter()
                .map(|error| error.message)
                .collect()
        }
    }
}

/// COVERAGE. Every cause in the closed vocabulary is specimened somewhere:
/// by a document here, or — for the emit and specimen layers, which take a
/// parameter set rather than a document — by its own family's file.
#[test]
fn every_cause_has_a_refusing_specimen() {
    let documented: BTreeSet<CommandCause> =
        SPECIMENS.iter().map(|specimen| specimen.cause).collect();
    let elsewhere: BTreeSet<CommandCause> = NON_DOCUMENT_CAUSES.into_iter().collect();

    let mut uncovered = Vec::new();
    for cause in CommandCause::ALL {
        if !documented.contains(&cause) && !elsewhere.contains(&cause) {
            uncovered.push(cause.as_str());
        }
    }
    assert!(
        uncovered.is_empty(),
        "these cause codes have no refusing specimen, so nothing proves the surface can produce \
         them: {uncovered:?}"
    );

    let overlap: Vec<&str> = documented
        .intersection(&elsewhere)
        .map(|cause| cause.as_str())
        .collect();
    assert!(
        overlap.is_empty(),
        "these causes claim to be specimened in two places, so one list is stale: {overlap:?}"
    );
}

/// The parse and check layers, both directions: each specimen refuses, and
/// refuses with the cause and the bytes it claims.
#[test]
fn every_document_specimen_refuses_with_its_own_cause_and_bytes() {
    let mut failures = Vec::new();
    for Specimen {
        cause,
        what,
        command,
        observed,
    } in SPECIMENS
    {
        let source = document(command);
        let refusals = all_refusals(&source);
        if refusals.is_empty() {
            failures.push(format!(
                "`{}` ({what}) was ACCEPTED; it must be refused\n---\n{source}---",
                cause.as_str()
            ));
            continue;
        }
        let marker = format!("named cause: {}", cause.as_str());
        let Some(matched) = refusals.iter().find(|text| text.contains(&marker)) else {
            failures.push(format!(
                "`{}` ({what}) was refused, but as: {refusals:#?}",
                cause.as_str()
            ));
            continue;
        };
        if let Some(bytes) = observed {
            let expected = format!("observed: {bytes:?}");
            assert_or_record(
                matched.contains(&expected),
                &mut failures,
                format!(
                    "`{}` ({what}) must carry {expected} in its refusal; it rendered:\n{matched}",
                    cause.as_str()
                ),
            );
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// Every specimen's cause is filed at the layer the vocabulary says it is, so
/// a refusal cannot claim one layer's discipline while doing another's work.
#[test]
fn every_specimen_refuses_at_the_layer_its_cause_names() {
    let mut failures = Vec::new();
    for specimen in SPECIMENS {
        let source = document(specimen.command);
        let parsed = aion_awl::parse(&source);
        let refused_at_parse = parsed.is_err();
        let expected_parse = specimen.cause.layer() == CommandLayer::Parse;
        if refused_at_parse != expected_parse {
            failures.push(format!(
                "`{}` is a {} cause but the document was {} at the parse layer",
                specimen.cause.as_str(),
                specimen.cause.layer(),
                if refused_at_parse {
                    "refused"
                } else {
                    "accepted"
                }
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// THE CONTROL. The specimen machinery must be able to say "accepted", or a
/// green run above would mean only that every document failed somehow.
#[test]
fn the_two_canonical_shapes_are_accepted() {
    for (label, command) in [
        ("the admitting cure", command::specimens::GIT_TAG),
        ("the diff census", command::specimens::DIFF_CENSUS),
    ] {
        let source = document(command);
        assert_eq!(
            refusal_text(&source),
            None,
            "{label} must be accepted; source:\n{source}"
        );
    }
}

fn assert_or_record(condition: bool, failures: &mut Vec<String>, message: String) {
    if !condition {
        failures.push(message);
    }
}
