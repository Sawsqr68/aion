//! The absorbed Just subset, and the forms refused beside it.
//!
//! # What "absorbed" means here
//!
//! AWL does not embed Just and does not depend on it. It has its OWN parser
//! over a SUBSET of Just's authoring shapes — the ones that appear in command
//! and parameter declarations — because those shapes are what an author who
//! has written a justfile already knows, and inventing a fourth spelling of
//! `param="default"` would cost them for nothing.
//!
//! The subset is exactly [`ABSORBED_PRODUCTIONS`] and nothing more. It is
//! authoring SHAPE only: none of Just's evaluation semantics is absorbed, and
//! everything in [`REFUSED_FORMS`] is refused by name, with its own cause and
//! its own reason.
//!
//! # Why the CC0 suite is carried as fixtures
//!
//! An own parser over someone else's shape drifts against author expectation
//! silently: the day `bar='baz'` stops meaning what a justfile author thinks
//! it means, nothing in this repository notices. So Just's own test suite —
//! CC0-1.0, so carrying its specimens is unencumbered — supplies the
//! conformance cases FOR EXACTLY THIS SUBSET. A drift becomes a red test
//! rather than a surprise.
//!
//! Suite cases that exercise a REFUSED form are carried too, as refusal
//! fixtures. Skipping them would leave the refused list asserted only in
//! prose, which is how a refused form quietly becomes an accepted one.
//!
//! Where the suite carries no specimen for an absorbed production — Just's
//! parameters are untyped, so no suite case can exercise a TYPED one — an
//! authored fixture satisfies the row and names its absence from the suite in
//! its own header. Completeness is a property of the question, and the
//! question must not be unanswerable on a technicality.

use super::cause::CommandCause;

/// The upstream suite every derived fixture cites, so provenance is a fact
/// about a commit rather than a memory of a file.
pub const SUITE_REPO: &str = "https://github.com/casey/just";
/// The commit the specimens were taken at.
pub const SUITE_COMMIT: &str = "b20386abdbae867a49cdff6c3c0f2b547faa9b23";
/// The suite's licence, which is what makes carrying its specimens free.
pub const SUITE_LICENSE: &str = "CC0-1.0";

/// One authoring shape this surface absorbed from Just.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbsorbedProduction {
    /// The production's stable identifier, which its fixtures name.
    pub id: &'static str,
    /// What the shape is, in one sentence.
    pub summary: &'static str,
    /// The AWL spelling of the shape.
    pub awl_form: &'static str,
    /// The Just spelling the shape was absorbed from.
    pub just_form: &'static str,
}

/// Exactly the absorbed subset. Adding a row is a design change.
///
/// The last three rows are the recipe half, and they are not in
/// contradiction with [`REFUSED_FORMS`]'s first three: a recipe body, a
/// dependency edge and a multi-step assembly are UNREPRESENTABLE inside a
/// gate-leg command declaration (amendment 1, structural) and representable
/// at the TOP LEVEL, where a reader of the document meets them. Same shapes,
/// two contexts, one fence between them — and each fixture header says which
/// context it is a fixture of.
pub const ABSORBED_PRODUCTIONS: [AbsorbedProduction; 7] = [
    AbsorbedProduction {
        id: "typed_parameter",
        summary: "A declaration takes named parameters, which are the only names its fills may \
                  interpolate.",
        awl_form: "command c(base: String)",
        just_form: "recipe base:",
    },
    AbsorbedProduction {
        id: "parameter_default",
        summary: "A parameter may carry a default, which may reference parameters declared \
                  before it and never after.",
        awl_form: "command c(a: String, b: String = \"{{a}}\")",
        just_form: "@foo a b=a:",
    },
    AbsorbedProduction {
        id: "variadic_tail",
        summary: "A list-typed parameter standing alone in an operand emits one argv element per \
                  item — the variadic tail, typed.",
        awl_form: "operand rest: [String] = \"{{args}}\"",
        just_form: "foo bar='baz' *bob:",
    },
    AbsorbedProduction {
        id: "interpolation",
        summary: "`{{name}}` interpolates one declared parameter into a fill.",
        awl_form: "operand range: String = \"{{base}}..HEAD\"",
        just_form: "echo {{ b }}",
    },
    AbsorbedProduction {
        id: "recipe_declaration",
        summary: "A recipe is declared at the top level, with typed parameters and an ordered \
                  body of steps. The SHAPE is absorbed; the body is not — a step names declared \
                  work, never a script line.",
        awl_form: "recipe foo",
        just_form: "foo bar='baz' *bob:",
    },
    AbsorbedProduction {
        id: "recipe_dependency",
        summary: "A step runs after another step of the same recipe. Absorbed as `after`, the \
                  word this language already uses for an ordering dependency — one word, one \
                  meaning, no synonym minted for the recipe layer.",
        awl_form: "step bar\n  after foo",
        just_form: "baz: foo::bar::baz",
    },
    AbsorbedProduction {
        id: "recipe_parameter_kind",
        summary: "A recipe parameter carries a declared field kind, and a `re_derive` one \
                  carries the source it is read from at the fire minute.",
        awl_form: "parameter control_expect: String re_derive derives \"origin/main:f\" extract \
                   \"line 22\"",
        just_form: "recipe param=\"value\"",
    },
];

/// One form named in the design's refused list, with the cause that refuses
/// it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefusedForm {
    /// The form's stable identifier, which its refusal fixture names.
    pub id: &'static str,
    /// The Just spelling that is refused.
    pub just_form: &'static str,
    /// Why it is refused — the reason that rides in the diagnostic.
    pub reason: &'static str,
    /// The cause a refusal of this form names.
    pub cause: CommandCause,
}

/// Every refused form, each with a refusal test. The absence-of-escape
/// discipline: a form on this list is refused BY NAME, never by falling
/// through to a general "unknown" diagnostic.
pub const REFUSED_FORMS: [RefusedForm; 8] = [
    RefusedForm {
        id: "recipe_body",
        just_form: "foo:\\n  echo hello",
        reason: "a recipe body is a script, and a gate leg consumes the command half only, so \
                 that a reader sees the exact command that ran",
        cause: CommandCause::RecipeFormRefused,
    },
    RefusedForm {
        id: "dependency_edge",
        just_form: "baz: foo::bar::baz",
        reason: "a dependency edge orders work, which is assembly; a gate-leg declaration has no \
                 production for one",
        cause: CommandCause::RecipeFormRefused,
    },
    RefusedForm {
        id: "multi_step_assembly",
        just_form: "foo *args: (bar args)",
        reason: "assembling a command from parts at run time produces a command no reader saw",
        cause: CommandCause::RecipeFormRefused,
    },
    RefusedForm {
        id: "backtick_substitution",
        just_form: "foos := `print('foo' * 4)`",
        reason: "backtick substitution runs a shell at evaluation time, which the determinism \
                 boundary does not admit",
        cause: CommandCause::BacktickSubstitution,
    },
    RefusedForm {
        id: "expression_interpolation",
        just_form: "echo {{if 'a' == 'b' { 'c' } else { 'd' }}}",
        reason: "an expression inside `{{…}}` is a templating language; interpolation here is \
                 closed over declared parameters and is a construction surface, not an evaluator",
        cause: CommandCause::InterpolationMalformed,
    },
    RefusedForm {
        id: "string_prefix_environment",
        just_form: "export FOO=bar",
        reason: "a `K=V` string is a prefix some shell would have to split, and splitting is what \
                 this surface exists to remove",
        cause: CommandCause::EnvStringPrefix,
    },
    RefusedForm {
        id: "shell_string_body",
        just_form: "foo:\\n  echo hello",
        reason: "a recipe step runs DECLARED work; a shell string in a step body is the \
                 argv-re-split family one level up, and a command assembled from parts at run \
                 time is a command no reader saw",
        cause: CommandCause::ShellStringBody,
    },
    RefusedForm {
        id: "nested_command_declaration",
        just_form: "(no Just form: Just has no declaration nesting)",
        reason: "amendment 1's converse fence — a command is declared at the top level and \
                 referenced by a step, so nesting one inside a recipe would put a gate-leg \
                 declaration where no reader of the top level meets it",
        cause: CommandCause::CommandDeclarationNested,
    },
];
