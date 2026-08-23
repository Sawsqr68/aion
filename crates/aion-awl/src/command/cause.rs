//! The closed cause vocabulary of the command-declaration surface.
//!
//! Every refusal this surface produces names one of these codes, and the set
//! is CLOSED: adding a code is a design change, not an implementation
//! decision. The string each code renders is the contract — it is what a
//! diagnostic prints, what a fixture header names, and what a caller matches
//! on — so it is stated exactly once, here.
//!
//! # Layer naming
//!
//! The carrier-contract law: a parse failure, a check refusal, an emit
//! refusal, a compose refusal, a fire refusal and a specimen-fire failure are
//! DIFFERENT LAYERS with different causes. Never one catch-all. Each code belongs to exactly one
//! [`CommandLayer`], and the layer error types in [`super::error`] refuse at
//! construction to carry a code from another layer — so "a check error that
//! is really a parse error" is not a bug that can be written, it is a value
//! that cannot be built.
//!
//! The six layers answer six different questions, and conflating them is
//! the defect the law exists to prevent:
//!
//! * **Parse** — the bytes are not a command or recipe declaration of this
//!   language. Nothing downstream ran. A form refused here is
//!   UNREPRESENTABLE, not discouraged.
//! * **Check** — the declaration parses, and what it says is wrong: a
//!   dangling parameter, an unstated end-of-options property, a hand literal
//!   where a derivation exists, a dependency cycle.
//! * **Emit** — the declaration is sound and a particular parameter set is
//!   not, or the argv the set would produce is one the declaration forbids.
//! * **Compose** — an assembly act was composed and refused BEFORE anything
//!   ran: a slot nothing filled, a projection that would emit a term a round
//!   measured out. Nothing has been fired, and nothing will be.
//! * **Fire** — the moment of firing itself refused: a referenced artifact's
//!   deployed bytes no longer match the census, a declared precondition was
//!   never answered, a stage's typed comparison disagreed, an object moved
//!   between the write and the claim.
//! * **Specimen** — a declared refusal specimen was fired and did not refuse,
//!   or refused for the wrong reason. This layer is how a MANDATORY specimen
//!   proves it is load-bearing rather than decorative.
//!
//! Compose and Fire are this cut's additions, and they exist because the
//! assembly surface acts at two moments the command surface had no name for.
//! Filing a fire-time census refusal under `Emit` would say that emitting an
//! argv and measuring a deployed artifact are the same discipline; they are
//! not, and a reader who cannot tell "this was refused before anything ran"
//! from "this was refused at the moment of firing" cannot tell whether the
//! world was touched.

/// Which layer of the surface a refusal came from.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum CommandLayer {
    /// The bytes are not a command or recipe declaration of this language.
    Parse,
    /// The declaration parses, and what it says is wrong.
    Check,
    /// A parameter set, or the argv it would produce, is refused.
    Emit,
    /// An assembly act was composed and refused before anything ran.
    Compose,
    /// The moment of firing refused: census, precondition, comparison, claim.
    Fire,
    /// A declared refusal specimen did not refuse, or refused wrongly.
    Specimen,
}

impl CommandLayer {
    /// Every layer, in the order a declaration passes through them.
    pub const ALL: [Self; 6] = [
        Self::Parse,
        Self::Check,
        Self::Emit,
        Self::Compose,
        Self::Fire,
        Self::Specimen,
    ];

    /// The layer's stable wire rendering. This string is the contract.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Parse => "parse",
            Self::Check => "check",
            Self::Emit => "emit",
            Self::Compose => "compose",
            Self::Fire => "fire",
            Self::Specimen => "specimen",
        }
    }
}

impl std::fmt::Display for CommandLayer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// The stable cause codes of the command-declaration surface.
///
/// Closed by design and closed in fact: [`CommandCause::ALL`] carries every
/// variant, `every_cause_has_a_refusing_specimen` proves each one is
/// reachable by a real document rather than merely declared, and the wire
/// strings are pinned so a rename is a visible break.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum CommandCause {
    // ---------------------------------------------------------- parse layer
    /// A word opened a line in a command body that is not one of its clauses.
    ClauseUnknown,
    /// A command declaration stated nothing at all.
    BodyEmpty,
    /// A recipe, a dependency edge, or a multi-step assembly form appeared in
    /// a gate-leg command declaration. Structurally refused: a gate leg
    /// consumes the command half only, so that a reader sees the exact
    /// command that ran.
    RecipeFormRefused,
    /// An environment binding was written as a `K=V` string prefix instead of
    /// a structural name and value.
    EnvStringPrefix,
    /// A backtick appeared in a fill. Backtick substitution executes at
    /// evaluation time, which the determinism boundary does not admit.
    BacktickSubstitution,
    /// A fill opened `{{` and never closed it.
    InterpolationUnterminated,
    /// A fill wrote `{{}}` — an interpolation naming no parameter.
    InterpolationEmpty,
    /// A fill wrote `{{…}}` around something that is not a parameter name.
    InterpolationMalformed,
    /// An `admits` or `refuses` clause named a marker other than `--`.
    EndOfOptionsMarkerWrong,
    /// A `program` clause named no words. The clause is there and says
    /// nothing, which the grammar can see for itself.
    ProgramWordsEmpty,
    /// A recipe, wall, or artifact declaration stated nothing at all.
    /// Distinct from [`CommandCause::BodyEmpty`], which is the command
    /// declaration's own silence: the two constructs admit different
    /// clauses, so a shared code could not name the admitted set.
    RecipeBodyEmpty,
    /// A word opened a line in a recipe, wall, or artifact body that is not
    /// one of that body's clauses.
    AssemblyClauseUnknown,
    /// A recipe step's body was written as a shell string. A step runs a
    /// DECLARED command or captures a DECLARED artifact; a string a shell
    /// would re-split is unrepresentable, not discouraged.
    ShellStringBody,
    /// A `command` declaration was written inside a recipe body. The
    /// converse half of amendment 1: a command is declared at the top level
    /// and REFERENCED by a step, so that the command a reader sees is the
    /// command that ran.
    CommandDeclarationNested,
    /// A recipe parameter named a kind outside the closed taxonomy
    /// (`skeleton`, `per_item`, `both`, `re_derive`).
    ParameterKindUnknown,

    // ---------------------------------------------------------- check layer
    /// The declaration carries no `program` clause at all, so nothing says
    /// what runs. Distinct from [`CommandCause::ProgramWordsEmpty`]: that is
    /// a clause the grammar refused, this is a declaration the checker read
    /// whole and found silent.
    ProgramUnstated,
    /// The invocation stated neither `admits "--"` nor `refuses "--"`. No
    /// command may sit in an unstated middle.
    EndOfOptionsUnstated,
    /// The end-of-options property was stated more than once.
    EndOfOptionsRestated,
    /// An invocation declared NOT admitting `--` carries no leading-dash
    /// refusal specimen. The specimen is mandatory, not advisory.
    LeadingDashSpecimenMissing,
    /// A leading-dash specimen's bytes do not begin with `-`, so firing it
    /// would prove nothing about the family it stands for.
    LeadingDashSpecimenNotLeading,
    /// A fill referenced a parameter the command does not declare.
    ParameterUndeclared,
    /// A fill's type does not match the option or operand it fills.
    ParameterTypeMismatch,
    /// A hand literal stands in a fill position for which a declared
    /// derivation exists. The literal is carried as observed bytes.
    LiteralWhereDerived,
    /// A second `hardening` block appeared in one scope. Hardening has one
    /// home.
    HardeningRestated,
    /// A `hardening` block stated nothing, so it hardens nothing.
    HardeningEmpty,
    /// A `cwd` carried a brace placeholder other than `{workspace_root}`.
    CwdPlaceholderUnknown,
    /// Two parameters of one command share a name.
    ParameterRestated,
    /// Two operands of one command share a name.
    OperandRestated,
    /// Two options of one command share a flag.
    OptionRestated,
    /// Two environment bindings of one command share a name.
    EnvRestated,
    /// A declared timeout named no owner.
    TimeoutOwnerEmpty,
    /// An admitting invocation carried a leading-dash refusal specimen. The
    /// marker closes the family by construction, so there is nothing for a
    /// specimen to prove and nothing keeps it honest.
    SpecimenWithoutRefusal,
    /// A list-typed parameter was interpolated somewhere other than alone in
    /// an operand — the one position where a list has a rendering, as the
    /// variadic tail. Anywhere else it has no single argv element to be.
    ListInScalarPosition,
    /// Two command declarations in one document share a name.
    CommandRestated,
    /// The dependency edges between a recipe's steps close a cycle, so no
    /// order satisfies them and nothing could run first.
    DependencyCycle,
    /// A recipe named a step, command, wall, or artifact the document does
    /// not declare. Assembly is FROM declarations: a reference to something
    /// undeclared is an edge to nowhere, and the sweep's completeness claim
    /// rests on the edge set being closed.
    ReferenceUndeclared,
    /// A `re_derive` parameter carries no declared derivation, so nothing
    /// says what would be re-derived or from where.
    DerivationMissing,
    /// A derivation's extraction names no extraction this surface performs.
    ExtractionUnknown,
    /// A declaration carries the same truth twice — two parameters whose
    /// declared values restate one another. One source, one projection.
    DoubleCarry,
    /// A recipe declared a value the DISPATCH already declares. The
    /// one-truth fence: a second hand-copy of a truth the dispatch carries
    /// is the class the reference census exists to close.
    VenueValueRestated,
    /// A declared artifact's digest is not a content digest — 64 lowercase
    /// hexadecimal characters — so no measurement could ever match it.
    ArtifactDigestMalformed,
    /// A wall declaration allows nothing, so its enforcement rendering would
    /// admit no path and its prompt rendering would name no wall.
    WallAllowsNothing,
    /// Two steps of one recipe share a name.
    RecipeStepRestated,
    /// Two assembly declarations of one kind share a name.
    AssemblyDeclarationRestated,
    /// A recipe declares no step, so it assembles nothing.
    RecipeStatesNoStep,

    // ----------------------------------------------------------- emit layer
    /// A parameter set omitted a parameter the command declares and gives no
    /// default.
    ArgumentMissing,
    /// A parameter set named something the command does not declare.
    ArgumentUndeclared,
    /// A supplied value's type does not match the declared parameter type.
    ArgumentTypeMismatch,
    /// An invocation declared NOT admitting `--` would emit an operand
    /// beginning with `-`. There is no end-of-options marker to protect it,
    /// so the operand would be read as an option.
    LeadingDashOperand,

    // -------------------------------------------------------- compose layer
    /// A slot nothing filled: a declared parameter with no binding and no
    /// default, or a bound value still carrying the sentinel that says it
    /// was never filled. The slot's name and the bytes seen both ride in the
    /// refusal.
    SlotUnfilled,
    /// A projection would emit a term the wall declares excluded — a term a
    /// round measured out, declared beside the wall so the next round cannot
    /// reintroduce it by hand.
    ExcludedTerm,

    // ----------------------------------------------------------- fire layer
    /// A referenced artifact's deployed bytes do not hash to the digest the
    /// census declares. Declared and observed hashes both ride in the
    /// refusal.
    CensusMismatch,
    /// A referenced artifact could not be read at all, so the census could
    /// not be measured and the fire must not proceed on an unmeasured tree.
    ArtifactUnreadable,
    /// A restore step left the restored bytes different from the bytes it
    /// was declared to restore. Restoration is byte-compared, never assumed.
    RestoreInexact,
    /// A stage's typed comparison disagreed: the record the stage measured
    /// is not the record it declared it would measure. Both sides' bytes
    /// ride in the refusal.
    StageCompareMismatch,
    /// An object was re-read from the committed state before being claimed,
    /// and had moved. A claim about bytes that have changed is a claim about
    /// a tree that no longer exists.
    ObjectMovedBeforeClaim,
    /// A declared at-fire precondition was never answered. An unanswered
    /// precondition is not a met one.
    PreconditionUnmet,
    /// A step ran a declared command and the command did not succeed, or
    /// could not be emitted for the values this run composed. An assembly
    /// whose act failed has not assembled anything, and going on would build
    /// the next step on a stage that is not there.
    StepCommandFailed,

    // ------------------------------------------------------- specimen layer
    /// A declared refusal specimen was fired and the surface accepted it.
    SpecimenAccepted,
    /// A declared refusal specimen refused, but named a different cause than
    /// the specimen stands for.
    SpecimenWrongCause,
}

impl CommandCause {
    /// Every cause code, grouped by layer in layer order.
    ///
    /// Rust cannot enumerate an enum's variants, so consumers that need the
    /// whole vocabulary — the specimen coverage gate above all — would
    /// otherwise hand-copy it and fall silently behind. `cause_inventory_is_
    /// complete` pins this list against the layer partition, so a variant
    /// added without an entry here fails a test rather than going missing.
    pub const ALL: [Self; 60] = [
        // parse
        Self::ClauseUnknown,
        Self::BodyEmpty,
        Self::RecipeFormRefused,
        Self::EnvStringPrefix,
        Self::BacktickSubstitution,
        Self::InterpolationUnterminated,
        Self::InterpolationEmpty,
        Self::InterpolationMalformed,
        Self::EndOfOptionsMarkerWrong,
        Self::ProgramWordsEmpty,
        Self::RecipeBodyEmpty,
        Self::AssemblyClauseUnknown,
        Self::ShellStringBody,
        Self::CommandDeclarationNested,
        Self::ParameterKindUnknown,
        // check
        Self::ProgramUnstated,
        Self::EndOfOptionsUnstated,
        Self::EndOfOptionsRestated,
        Self::LeadingDashSpecimenMissing,
        Self::LeadingDashSpecimenNotLeading,
        Self::ParameterUndeclared,
        Self::ParameterTypeMismatch,
        Self::LiteralWhereDerived,
        Self::HardeningRestated,
        Self::HardeningEmpty,
        Self::CwdPlaceholderUnknown,
        Self::ParameterRestated,
        Self::OperandRestated,
        Self::OptionRestated,
        Self::EnvRestated,
        Self::TimeoutOwnerEmpty,
        Self::SpecimenWithoutRefusal,
        Self::ListInScalarPosition,
        Self::CommandRestated,
        Self::DependencyCycle,
        Self::ReferenceUndeclared,
        Self::DerivationMissing,
        Self::ExtractionUnknown,
        Self::DoubleCarry,
        Self::VenueValueRestated,
        Self::ArtifactDigestMalformed,
        Self::WallAllowsNothing,
        Self::RecipeStepRestated,
        Self::AssemblyDeclarationRestated,
        Self::RecipeStatesNoStep,
        // emit
        Self::ArgumentMissing,
        Self::ArgumentUndeclared,
        Self::ArgumentTypeMismatch,
        Self::LeadingDashOperand,
        // compose
        Self::SlotUnfilled,
        Self::ExcludedTerm,
        // fire
        Self::CensusMismatch,
        Self::ArtifactUnreadable,
        Self::RestoreInexact,
        Self::StageCompareMismatch,
        Self::ObjectMovedBeforeClaim,
        Self::PreconditionUnmet,
        Self::StepCommandFailed,
        // specimen
        Self::SpecimenAccepted,
        Self::SpecimenWrongCause,
    ];

    /// The layer this cause belongs to.
    ///
    /// An exhaustive match with no wildcard: a cause added to the language
    /// fails to compile here until someone decides which layer it refuses at,
    /// rather than defaulting into one.
    #[must_use]
    pub const fn layer(self) -> CommandLayer {
        match self {
            Self::ClauseUnknown
            | Self::BodyEmpty
            | Self::RecipeFormRefused
            | Self::EnvStringPrefix
            | Self::BacktickSubstitution
            | Self::InterpolationUnterminated
            | Self::InterpolationEmpty
            | Self::InterpolationMalformed
            | Self::EndOfOptionsMarkerWrong
            | Self::ProgramWordsEmpty
            | Self::RecipeBodyEmpty
            | Self::AssemblyClauseUnknown
            | Self::ShellStringBody
            | Self::CommandDeclarationNested
            | Self::ParameterKindUnknown => CommandLayer::Parse,
            Self::ProgramUnstated
            | Self::EndOfOptionsUnstated
            | Self::EndOfOptionsRestated
            | Self::LeadingDashSpecimenMissing
            | Self::LeadingDashSpecimenNotLeading
            | Self::ParameterUndeclared
            | Self::ParameterTypeMismatch
            | Self::LiteralWhereDerived
            | Self::HardeningRestated
            | Self::HardeningEmpty
            | Self::CwdPlaceholderUnknown
            | Self::ParameterRestated
            | Self::OperandRestated
            | Self::OptionRestated
            | Self::EnvRestated
            | Self::TimeoutOwnerEmpty
            | Self::SpecimenWithoutRefusal
            | Self::ListInScalarPosition
            | Self::CommandRestated
            | Self::DependencyCycle
            | Self::ReferenceUndeclared
            | Self::DerivationMissing
            | Self::ExtractionUnknown
            | Self::DoubleCarry
            | Self::VenueValueRestated
            | Self::ArtifactDigestMalformed
            | Self::WallAllowsNothing
            | Self::RecipeStepRestated
            | Self::AssemblyDeclarationRestated
            | Self::RecipeStatesNoStep => CommandLayer::Check,
            Self::ArgumentMissing
            | Self::ArgumentUndeclared
            | Self::ArgumentTypeMismatch
            | Self::LeadingDashOperand => CommandLayer::Emit,
            Self::SlotUnfilled | Self::ExcludedTerm => CommandLayer::Compose,
            Self::CensusMismatch
            | Self::ArtifactUnreadable
            | Self::RestoreInexact
            | Self::StageCompareMismatch
            | Self::ObjectMovedBeforeClaim
            | Self::PreconditionUnmet
            | Self::StepCommandFailed => CommandLayer::Fire,
            Self::SpecimenAccepted | Self::SpecimenWrongCause => CommandLayer::Specimen,
        }
    }

    /// The cause's stable wire rendering. This string is the contract.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ClauseUnknown => "clause_unknown",
            Self::BodyEmpty => "body_empty",
            Self::RecipeFormRefused => "recipe_form_refused",
            Self::EnvStringPrefix => "env_string_prefix",
            Self::BacktickSubstitution => "backtick_substitution",
            Self::InterpolationUnterminated => "interpolation_unterminated",
            Self::InterpolationEmpty => "interpolation_empty",
            Self::InterpolationMalformed => "interpolation_malformed",
            Self::EndOfOptionsMarkerWrong => "end_of_options_marker_wrong",
            Self::ProgramWordsEmpty => "program_words_empty",
            Self::RecipeBodyEmpty => "recipe_body_empty",
            Self::AssemblyClauseUnknown => "assembly_clause_unknown",
            Self::ShellStringBody => "shell_string_body",
            Self::CommandDeclarationNested => "command_declaration_nested",
            Self::ParameterKindUnknown => "parameter_kind_unknown",
            Self::ProgramUnstated => "program_unstated",
            Self::EndOfOptionsUnstated => "end_of_options_unstated",
            Self::EndOfOptionsRestated => "end_of_options_restated",
            Self::LeadingDashSpecimenMissing => "leading_dash_specimen_missing",
            Self::LeadingDashSpecimenNotLeading => "leading_dash_specimen_not_leading",
            Self::ParameterUndeclared => "parameter_undeclared",
            Self::ParameterTypeMismatch => "parameter_type_mismatch",
            Self::LiteralWhereDerived => "literal_where_derived",
            Self::HardeningRestated => "hardening_restated",
            Self::HardeningEmpty => "hardening_empty",
            Self::CwdPlaceholderUnknown => "cwd_placeholder_unknown",
            Self::ParameterRestated => "parameter_restated",
            Self::OperandRestated => "operand_restated",
            Self::OptionRestated => "option_restated",
            Self::EnvRestated => "env_restated",
            Self::TimeoutOwnerEmpty => "timeout_owner_empty",
            Self::SpecimenWithoutRefusal => "specimen_without_refusal",
            Self::ListInScalarPosition => "list_in_scalar_position",
            Self::CommandRestated => "command_restated",
            Self::DependencyCycle => "dependency_cycle",
            Self::ReferenceUndeclared => "reference_undeclared",
            Self::DerivationMissing => "derivation_missing",
            Self::ExtractionUnknown => "extraction_unknown",
            Self::DoubleCarry => "double_carry",
            Self::VenueValueRestated => "venue_value_restated",
            Self::ArtifactDigestMalformed => "artifact_digest_malformed",
            Self::WallAllowsNothing => "wall_allows_nothing",
            Self::RecipeStepRestated => "recipe_step_restated",
            Self::AssemblyDeclarationRestated => "assembly_declaration_restated",
            Self::RecipeStatesNoStep => "recipe_states_no_step",
            Self::ArgumentMissing => "argument_missing",
            Self::ArgumentUndeclared => "argument_undeclared",
            Self::ArgumentTypeMismatch => "argument_type_mismatch",
            Self::LeadingDashOperand => "leading_dash_operand",
            Self::SlotUnfilled => "slot_unfilled",
            Self::ExcludedTerm => "excluded_term",
            Self::CensusMismatch => "census_mismatch",
            Self::ArtifactUnreadable => "artifact_unreadable",
            Self::RestoreInexact => "restore_inexact",
            Self::StageCompareMismatch => "stage_compare_mismatch",
            Self::ObjectMovedBeforeClaim => "object_moved_before_claim",
            Self::PreconditionUnmet => "precondition_unmet",
            Self::StepCommandFailed => "step_command_failed",
            Self::SpecimenAccepted => "specimen_accepted",
            Self::SpecimenWrongCause => "specimen_wrong_cause",
        }
    }

    /// The cause named by its wire string, or `None` when the string names no
    /// cause of this closed set.
    ///
    /// Fixtures declare the cause they expect by its wire string, so this is
    /// the door a fixture header comes in through — and an unknown string
    /// must be nothing, never a nearest match.
    #[must_use]
    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|cause| cause.as_str() == code)
    }
}

impl std::fmt::Display for CommandCause {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[cfg(test)]
#[path = "cause_tests.rs"]
mod cause_tests;
