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
//! refusal, and a specimen-fire failure are DIFFERENT LAYERS with different
//! causes. Never one catch-all. Each code belongs to exactly one
//! [`CommandLayer`], and the layer error types in [`super::error`] refuse at
//! construction to carry a code from another layer — so "a check error that
//! is really a parse error" is not a bug that can be written, it is a value
//! that cannot be built.
//!
//! The four layers answer four different questions, and conflating them is
//! the defect the law exists to prevent:
//!
//! * **Parse** — the bytes are not a command declaration of this language.
//!   Nothing downstream ran. A form refused here is UNREPRESENTABLE, not
//!   discouraged.
//! * **Check** — the declaration parses, and what it says is wrong: a
//!   dangling parameter, an unstated end-of-options property, a hand literal
//!   where a derivation exists.
//! * **Emit** — the declaration is sound and a particular parameter set is
//!   not, or the argv the set would produce is one the declaration forbids.
//! * **Specimen** — a declared refusal specimen was fired and did not refuse,
//!   or refused for the wrong reason. This layer is how a MANDATORY specimen
//!   proves it is load-bearing rather than decorative.

/// Which layer of the surface a refusal came from.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum CommandLayer {
    /// The bytes are not a command declaration of this language.
    Parse,
    /// The declaration parses, and what it says is wrong.
    Check,
    /// A parameter set, or the argv it would produce, is refused.
    Emit,
    /// A declared refusal specimen did not refuse, or refused wrongly.
    Specimen,
}

impl CommandLayer {
    /// Every layer, in the order a declaration passes through them.
    pub const ALL: [Self; 4] = [Self::Parse, Self::Check, Self::Emit, Self::Specimen];

    /// The layer's stable wire rendering. This string is the contract.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Parse => "parse",
            Self::Check => "check",
            Self::Emit => "emit",
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
    pub const ALL: [Self; 35] = [
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
        // emit
        Self::ArgumentMissing,
        Self::ArgumentUndeclared,
        Self::ArgumentTypeMismatch,
        Self::LeadingDashOperand,
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
            | Self::ProgramWordsEmpty => CommandLayer::Parse,
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
            | Self::CommandRestated => CommandLayer::Check,
            Self::ArgumentMissing
            | Self::ArgumentUndeclared
            | Self::ArgumentTypeMismatch
            | Self::LeadingDashOperand => CommandLayer::Emit,
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
            Self::ArgumentMissing => "argument_missing",
            Self::ArgumentUndeclared => "argument_undeclared",
            Self::ArgumentTypeMismatch => "argument_type_mismatch",
            Self::LeadingDashOperand => "leading_dash_operand",
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
