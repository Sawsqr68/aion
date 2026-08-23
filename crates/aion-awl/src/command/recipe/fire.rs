//! The fire layer: the moment the assembly reaches for the world.
//!
//! Order is load-bearing and is NOT to be reordered:
//!
//! 1. **The receipt opens FIRST.** It names the tree it measured before
//!    anything is measured against that tree, so a refusal at any later
//!    point still has a receipt to be recorded in. "Receipt first — no
//!    receipt, no staging" is carried as law rather than as a habit
//!    (`.fleet/fixtures/corpus/speech1.STAGING.md` at `1150e466…`, step 1).
//! 2. **Preconditions.** A declared at-fire precondition that was never
//!    answered stops the fire. An unanswered precondition is not a met one —
//!    the three-arm re-check a checklist carries by hand (`:37–39`: idle
//!    enough, workers serving the queue, disk ≥ 40G) is declared, and the
//!    executing side answers each by name.
//! 3. **The reference census.** Every artifact the recipe reaches outside
//!    itself is measured against its declared digest before a single step
//!    runs, so a fire against a moved artifact stops here rather than
//!    reporting a green over the wrong bytes.
//! 4. **The steps, in dependency order**, each producing its outcome into
//!    the receipt whether it passed or refused.
//!
//! Re-derived parameters resolve HERE and nowhere earlier. That is the whole
//! of the fire-minute law: compose/fire drift is impossible rather than
//! checked-for, because there is no compose-time value to drift from.

use std::collections::BTreeMap;

use crate::Span;

use super::super::cause::CommandCause;
use super::super::check::check_command;
use super::super::decl::CommandDecl;
use super::super::emit::{ArgumentValue, Argv, ParameterSet, emit};
use super::super::error::CommandRefusal;
use super::census::{ArtifactSource, digest_of, measure};
use super::compare::{StageComparison, VerdictRecord, compare};
use super::compose::Composed;
use super::decl::{ArtifactDecl, RecipeDecl, StepBody};
use super::derivation::resolve;
use super::graph::topological_order;
use super::receipt::{MeasuredTree, Receipt, ReceiptError, StepOutcome};

/// The answers the executing side gives to a recipe's declared at-fire
/// preconditions.
///
/// Each declared precondition is answered by its own declared text. There is
/// no "assume met", and there is no default: a precondition nobody answered
/// is the shape a checklist step takes when it was skipped.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Preconditions {
    answered: BTreeMap<String, bool>,
}

impl Preconditions {
    /// No precondition answered yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Answer one declared precondition.
    #[must_use]
    pub fn answered(mut self, precondition: impl Into<String>, met: bool) -> Self {
        self.answered.insert(precondition.into(), met);
        self
    }

    /// Whether this precondition was answered, and how.
    #[must_use]
    pub fn answer(&self, precondition: &str) -> Option<bool> {
        self.answered.get(precondition).copied()
    }
}

/// What a fire produced: its receipt, and the refusal that stopped it if one
/// did.
///
/// Both, always. A refused fire that produced no receipt is a fire nobody
/// can audit, which is the failure receipt-first ordering exists to prevent.
#[derive(Debug, Clone)]
pub struct Fired {
    /// The generated receipt. Present whether the fire passed or refused.
    pub receipt: Receipt,
    /// The refusal that stopped the fire, if one did.
    pub refusal: Option<CommandRefusal>,
}

impl Fired {
    /// Whether the fire completed with no refusal.
    #[must_use]
    pub const fn passed(&self) -> bool {
        self.refusal.is_none()
    }
}

/// The three ports a fire runs against: a tree to read, a firer to run a
/// declared artifact, and a runner to run a declared command's argv.
///
/// Bundled because they are ONE seam, not three arguments that happen to
/// travel together: everything in here is something the language front
/// declares and deliberately does not implement, and everything outside it
/// is the front's own.
pub struct Ports<'a> {
    /// The tree the census and the derivations are read from.
    pub source: &'a dyn ArtifactSource,
    /// What runs a declared demonstration artifact.
    pub firer: &'a dyn SpecimenFirer,
    /// What runs a declared command's emitted argv.
    pub runner: &'a dyn CommandRunner,
}

/// What running a declared command answered.
///
/// The second port the front declares and does not implement. Emitting the
/// argv is the front's — cut-1's checker and emitter do it, so a recipe step
/// runs exactly the argv `aion awl check` proved sound — and SPAWNING it is
/// the executing side's, which is what keeps process machinery out of the
/// language front.
pub trait CommandRunner {
    /// Run `argv` as the declaration emitted it, and return its exit status.
    ///
    /// # Errors
    ///
    /// Returns the reason the argv could not be run at all, verbatim.
    fn run(&self, command: &Argv) -> Result<i32, String>;
}

/// What a fired artifact answered.
///
/// The executing side runs the declared artifact and hands back what it
/// said; the front compares. Splitting it this way is what keeps the front
/// free of process machinery while the comparison stays the front's.
pub trait SpecimenFirer {
    /// Fire the artifact at `address` and return the record it produced.
    ///
    /// # Errors
    ///
    /// Returns the reason the artifact could not be fired, verbatim.
    fn fire(&self, address: &str) -> Result<VerdictRecord, String>;
}

/// Fire a checked recipe.
///
/// # Errors
///
/// Returns [`ReceiptError`] only when the receipt itself cannot be built —
/// a tree that cannot say what it measured. Every other outcome is carried
/// in the returned [`Fired`], because a fire that refuses still has a
/// receipt to hand back.
pub fn fire(
    recipe: &RecipeDecl,
    commands: &[CommandDecl],
    artifacts: &[ArtifactDecl],
    composed: &Composed,
    preconditions: &Preconditions,
    tree: MeasuredTree,
    ports: &Ports<'_>,
) -> Result<Fired, ReceiptError> {
    let source = ports.source;
    // 1. RECEIPT FIRST.
    let mut receipt = Receipt::new(tree, &recipe.name)?;

    // 2. Preconditions.
    if let Some(refusal) = unmet_precondition(recipe, preconditions) {
        return Ok(Fired {
            receipt,
            refusal: Some(refusal),
        });
    }

    // 3. The reference census, measured before anything runs.
    let (rows, census_refusal) = measure(recipe, artifacts, source);
    receipt.measured(rows);
    if let Some(refusal) = census_refusal {
        return Ok(Fired {
            receipt,
            refusal: Some(refusal),
        });
    }

    // 4. Re-derived parameters, read AT THIS MINUTE and no earlier.
    let values = match derive_deferred(recipe, composed, source, &mut receipt) {
        Ok(values) => values,
        Err(refusal) => {
            return Ok(Fired {
                receipt,
                refusal: Some(refusal),
            });
        }
    };

    // 5. The steps, in dependency order.
    let refusal = run_steps(recipe, commands, artifacts, &values, ports, &mut receipt);
    Ok(Fired { receipt, refusal })
}

/// Read every deferred parameter from its declared source, AT THIS MINUTE.
///
/// Each resolution is recorded in the receipt as it happens, so a run that
/// stops halfway still says which values it had already read.
///
/// # Errors
///
/// Returns the first refusal a derivation earned.
fn derive_deferred(
    recipe: &RecipeDecl,
    composed: &Composed,
    source: &dyn ArtifactSource,
    receipt: &mut Receipt,
) -> Result<BTreeMap<String, String>, CommandRefusal> {
    let mut values = composed.resolved.clone();
    for name in &composed.deferred {
        let Some(parameter) = recipe.parameters.iter().find(|item| item.name == *name) else {
            continue;
        };
        let Some(derivation) = &parameter.derivation else {
            continue;
        };
        match resolve(derivation, name, source) {
            Ok(value) => {
                receipt.stepped(StepOutcome {
                    step: format!("derive {name}"),
                    did: format!("re-derived from `{}` at the fire minute", derivation.source),
                    comparison: None,
                    refused: None,
                });
                values.insert(name.clone(), value);
            }
            Err(refusal) => {
                receipt.stepped(StepOutcome {
                    step: format!("derive {name}"),
                    did: format!("re-derive from `{}`", derivation.source),
                    comparison: None,
                    refused: Some(refusal.rendered()),
                });
                return Err(refusal);
            }
        }
    }
    Ok(values)
}

/// Run the steps in DEPENDENCY order, recording each outcome as it happens.
///
/// Returns the refusal that stopped the run, or `None` when every step ran.
fn run_steps(
    recipe: &RecipeDecl,
    commands: &[CommandDecl],
    artifacts: &[ArtifactDecl],
    values: &BTreeMap<String, String>,
    ports: &Ports<'_>,
    receipt: &mut Receipt,
) -> Option<CommandRefusal> {
    let Ok(ordered) = topological_order(recipe) else {
        // A cycle is a CHECK refusal and a checked recipe cannot carry one.
        // Reaching here means an unchecked recipe was fired; say so rather
        // than running an order that does not exist.
        return Some(CommandRefusal::refuse(
            CommandCause::DependencyCycle,
            recipe.span,
            &recipe.name,
            format!(
                "recipe `{}` was fired with a dependency cycle in it, which the check layer \
                 refuses; an unchecked recipe has no order to run in",
                recipe.name
            ),
        ));
    };

    for step in ordered {
        let Some(body) = &step.body else {
            continue;
        };
        let report = run_step(recipe, commands, artifacts, values, ports, step, body);
        receipt.stepped(report.outcome);
        if report.refusal.is_some() {
            return report.refusal;
        }
    }
    None
}

/// What one step produced: the outcome the receipt records — ALWAYS — and
/// the refusal that stopped it, if one did.
///
/// A struct rather than a `Result`, because a step ALWAYS produces an
/// outcome. Modelling the refusing arm as an error would make "the step
/// refused and recorded nothing" a state the type admits, and that state is
/// the one receipt-first ordering exists to remove.
pub struct StepReport {
    /// What the receipt records for this step.
    pub outcome: StepOutcome,
    /// The refusal that stopped it, if one did.
    pub refusal: Option<CommandRefusal>,
}

fn run_step(
    recipe: &RecipeDecl,
    commands: &[CommandDecl],
    artifacts: &[ArtifactDecl],
    values: &BTreeMap<String, String>,
    ports: &Ports<'_>,
    step: &super::decl::RecipeStep,
    body: &StepBody,
) -> StepReport {
    let named = step.name.clone();
    match body {
        StepBody::RunsCommand {
            command,
            command_span,
            ..
        } => match run_command(commands, command, *command_span, values, ports.runner) {
            Ok(did) => StepReport {
                outcome: StepOutcome {
                    step: named,
                    did,
                    comparison: None,
                    refused: None,
                },
                refusal: None,
            },
            Err(refusal) => StepReport {
                outcome: StepOutcome {
                    step: named,
                    did: format!("run declared command `{command}`"),
                    comparison: None,
                    refused: Some(refusal.rendered()),
                },
                refusal: Some(refusal),
            },
        },
        StepBody::CapturesArtifact {
            artifact,
            artifact_span,
            capture,
            ..
        } => match capture_artifact(artifacts, artifact, *artifact_span, ports.source) {
            Ok(digest) => StepReport {
                outcome: StepOutcome {
                    step: named,
                    did: format!("captured `{capture}` of artifact `{artifact}` ({digest})"),
                    comparison: None,
                    refused: None,
                },
                refusal: None,
            },
            Err(refusal) => StepReport {
                outcome: StepOutcome {
                    step: named,
                    did: format!("capture `{capture}` of artifact `{artifact}`"),
                    comparison: None,
                    refused: Some(refusal.rendered()),
                },
                refusal: Some(refusal),
            },
        },
        StepBody::FiresArtifact {
            artifact,
            expecting,
            expecting_span,
            ..
        } => {
            let fired = fire_specimen(
                recipe,
                artifacts,
                artifact,
                expecting,
                *expecting_span,
                ports,
            );
            let did = match &fired.refusal {
                None => format!("fired artifact `{artifact}`{}", fired.restored),
                Some(_) => format!("fired artifact `{artifact}`"),
            };
            StepReport {
                outcome: StepOutcome {
                    step: named,
                    did,
                    comparison: fired.comparison,
                    refused: fired.refusal.as_ref().map(CommandRefusal::rendered),
                },
                refusal: fired.refusal,
            }
        }
    }
}

/// A declared precondition nobody answered, or answered "not met".
fn unmet_precondition(
    recipe: &RecipeDecl,
    preconditions: &Preconditions,
) -> Option<CommandRefusal> {
    for step in &recipe.steps {
        for declared in &step.requires {
            let said = preconditions.answer(&declared.text);
            if said == Some(true) {
                continue;
            }
            let how = if said.is_none() {
                "was never answered"
            } else {
                "was answered NOT MET"
            };
            return Some(CommandRefusal::refuse(
                CommandCause::PreconditionUnmet,
                declared.span,
                &declared.text,
                format!(
                    "recipe `{}`'s step `{}` declares the at-fire precondition `{}`, which {how}. \
                     A precondition is re-checked at the fire minute because the box's occupancy \
                     changes between staging and firing, and an unanswered precondition is not a \
                     met one",
                    recipe.name, step.name, declared.text
                ),
            ));
        }
    }
    None
}

/// Emit a declared command's argv from the values this run resolved, run it,
/// and say what happened — never that it ran when it did not.
///
/// The argv is emitted through cut-1's own checker and emitter, so a step
/// runs exactly the argv `aion awl check` proved sound. Every parameter the
/// declaration takes is bound from the composition; a parameter the
/// composition has no value for is left to the declaration's own default,
/// and the emitter refuses if there is none.
fn run_command(
    commands: &[CommandDecl],
    name: &str,
    span: Span,
    values: &BTreeMap<String, String>,
    runner: &dyn CommandRunner,
) -> Result<String, CommandRefusal> {
    let Some(decl) = commands.iter().find(|item| item.name == name) else {
        return Err(CommandRefusal::refuse(
            CommandCause::ReferenceUndeclared,
            span,
            name,
            format!("command `{name}` is not declared, so there is nothing to run"),
        ));
    };
    let checked = check_command(decl).map_err(|mut refusals| {
        refusals.pop().unwrap_or_else(|| {
            CommandRefusal::refuse(
                CommandCause::StepCommandFailed,
                span,
                name,
                format!("command `{name}` did not check, and said nothing about why"),
            )
        })
    })?;
    let mut parameters = ParameterSet::new();
    for parameter in &decl.parameters {
        if let Some(value) = values.get(&parameter.name) {
            parameters = parameters.with(&parameter.name, ArgumentValue::scalar(value));
        }
    }
    let argv = emit(checked, &parameters)?;
    let rendered = argv.argv.join(" ");
    let status = runner.run(&argv).map_err(|detail| {
        CommandRefusal::refuse(
            CommandCause::StepCommandFailed,
            span,
            &rendered,
            format!("command `{name}` could not be run: {detail}"),
        )
    })?;
    if status != 0 {
        return Err(CommandRefusal::refuse(
            CommandCause::StepCommandFailed,
            span,
            &rendered,
            format!(
                "command `{name}` ran and exited {status}. An assembly whose act failed has not \
                 assembled anything, so the stage stops here rather than building the next step \
                 on a stage that is not there"
            ),
        ));
    }
    Ok(format!(
        "ran declared command `{name}` as {rendered:?}, exit 0"
    ))
}

/// Read a declared artifact's bytes for a capture, and refuse a sentinel
/// still standing in them.
fn capture_artifact(
    artifacts: &[ArtifactDecl],
    name: &str,
    span: Span,
    source: &dyn ArtifactSource,
) -> Result<String, CommandRefusal> {
    let Some(artifact) = artifacts.iter().find(|item| item.name == name) else {
        return Err(CommandRefusal::refuse(
            CommandCause::ReferenceUndeclared,
            span,
            name,
            format!("artifact `{name}` is not declared, so there is nothing to capture"),
        ));
    };
    let bytes = source.read(&artifact.address).map_err(|unreadable| {
        CommandRefusal::refuse(
            CommandCause::ArtifactUnreadable,
            artifact.address_span,
            &artifact.address,
            format!("artifact `{name}` could not be read at the fire minute: {unreadable}"),
        )
    })?;
    let text = String::from_utf8_lossy(&bytes);
    for sentinel in &artifact.excludes {
        if text.contains(&sentinel.text) {
            return Err(CommandRefusal::refuse(
                CommandCause::SlotUnfilled,
                sentinel.span,
                &sentinel.text,
                format!(
                    "artifact `{name}` at `{}` still carries `{}`, the sentinel that says it was \
                     never filled. The ground must state its MEASURED truth before anything \
                     composes from it — a document that predicts where it could cite is the fault \
                     this refusal exists to prevent",
                    artifact.address, sentinel.text
                ),
            ));
        }
    }
    Ok(digest_of(&bytes))
}

/// What firing one declared demonstration artifact produced.
///
/// A struct rather than a `Result`, for the same reason [`StepReport`] is:
/// the comparison must reach the receipt whether the stage passed or was
/// stopped by it, and an error type that swallowed the comparison would make
/// "refused, and nobody can see what was compared" representable.
struct FireReport {
    /// The typed comparison, when the stage got far enough to make one.
    comparison: Option<StageComparison>,
    /// What the receipt says the fire also did — a byte-exact restoration.
    restored: String,
    /// The refusal that stopped the stage, if one did.
    refusal: Option<CommandRefusal>,
}

impl FireReport {
    /// A stage stopped before it could compare anything.
    fn stopped(refusal: CommandRefusal) -> Self {
        Self {
            comparison: None,
            restored: String::new(),
            refusal: Some(refusal),
        }
    }
}

/// Fire one declared demonstration artifact and compare TYPED.
fn fire_specimen(
    recipe: &RecipeDecl,
    artifacts: &[ArtifactDecl],
    name: &str,
    expecting: &str,
    expecting_span: Span,
    ports: &Ports<'_>,
) -> FireReport {
    let Some(artifact) = artifacts.iter().find(|item| item.name == name) else {
        return FireReport::stopped(CommandRefusal::refuse(
            CommandCause::ReferenceUndeclared,
            expecting_span,
            name,
            format!("artifact `{name}` is not declared, so there is nothing to fire"),
        ));
    };
    // Read what the restore target says BEFORE the fire, since after it is
    // too late to know what it was.
    let before = artifact
        .restores
        .as_ref()
        .map(|restores| (restores, ports.source.read(&restores.text)));

    let observed = match ports.firer.fire(&artifact.address) {
        Ok(observed) => observed,
        Err(detail) => {
            return FireReport::stopped(CommandRefusal::refuse(
                CommandCause::ArtifactUnreadable,
                artifact.address_span,
                &artifact.address,
                format!(
                    "recipe `{}` fires artifact `{name}` at `{}`, which could not be fired: \
                     {detail}",
                    recipe.name, artifact.address
                ),
            ));
        }
    };

    let expected = match VerdictRecord::new(expecting, observed.layer(), observed.observed_bytes())
    {
        Ok(expected) => expected,
        Err(error) => {
            return FireReport::stopped(CommandRefusal::refuse(
                CommandCause::StageCompareMismatch,
                expecting_span,
                expecting,
                format!(
                    "recipe `{}`'s known answer for artifact `{name}` is not a record: {error}",
                    recipe.name
                ),
            ));
        }
    };
    let comparison = compare(&expected, &observed);
    if !comparison.matched() {
        let refusal = CommandRefusal::refuse(
            CommandCause::StageCompareMismatch,
            expecting_span,
            observed.code(),
            format!(
                "recipe `{}` fired artifact `{name}` and the typed comparison disagreed — {}. A \
                 known-answer red proves the instrument only when the answer it gives is the \
                 answer it was declared to give",
                recipe.name,
                comparison.rendered()
            ),
        );
        return FireReport {
            comparison: Some(comparison),
            restored: String::new(),
            refusal: Some(refusal),
        };
    }

    match verify_restoration(name, before, ports) {
        Ok(restored) => FireReport {
            comparison: Some(comparison),
            restored,
            refusal: None,
        },
        Err(refusal) => FireReport {
            comparison: Some(comparison),
            restored: String::new(),
            refusal: Some(refusal),
        },
    }
}

/// The bytes a restore artifact was declared to restore, BYTE-COMPARED after
/// the fire.
///
/// # Errors
///
/// Returns [`CommandCause::RestoreInexact`] when the bytes differ, and
/// [`CommandCause::ArtifactUnreadable`] when either side could not be read —
/// an unverifiable restore is not a verified one.
type BeforeBytes<'a> = Option<(
    &'a super::decl::ParamLiteral,
    Result<Vec<u8>, super::census::SourceUnreadable>,
)>;

fn verify_restoration(
    name: &str,
    before: BeforeBytes<'_>,
    ports: &Ports<'_>,
) -> Result<String, CommandRefusal> {
    let Some((restores, original)) = before else {
        return Ok(String::new());
    };
    let original = original.map_err(|unreadable| {
        CommandRefusal::refuse(
            CommandCause::ArtifactUnreadable,
            restores.span,
            &restores.text,
            format!(
                "artifact `{name}` restores `{}`, which could not be read BEFORE the fire: \
                 {unreadable}. Without the original bytes there is nothing to byte-compare the \
                 restoration against, and an unverifiable restore is not a verified one",
                restores.text
            ),
        )
    })?;
    let after = ports.source.read(&restores.text).map_err(|unreadable| {
        CommandRefusal::refuse(
            CommandCause::ArtifactUnreadable,
            restores.span,
            &restores.text,
            format!(
                "artifact `{name}` restores `{}`, which could not be re-read to verify the \
                 restoration: {unreadable}",
                restores.text
            ),
        )
    })?;
    if after == original {
        return Ok(format!(" and restored `{}` byte-exactly", restores.text));
    }
    Err(CommandRefusal::refuse(
        CommandCause::RestoreInexact,
        restores.span,
        digest_of(&after),
        format!(
            "artifact `{name}` restores `{}`, and the bytes there hashed to `{}` before the fire \
             and `{}` after it. A restore is BYTE-COMPARED, never assumed: an inexact restoration \
             leaves a tree nobody declared",
            restores.text,
            digest_of(&original),
            digest_of(&after)
        ),
    ))
}
