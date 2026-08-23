//! The shape of a declared recipe, a declared wall, and a declared artifact.
//!
//! # Why a recipe body is not a script
//!
//! A recipe is an ASSEMBLY act: it orders declared work. Its steps therefore
//! name work that was declared elsewhere — a [`CommandDecl`](crate::command::CommandDecl)
//! at the top level, an [`ArtifactDecl`] at the top level — and never carry a
//! string a shell would re-split. That is the same fence the command surface
//! draws around its argv, drawn one level up: a recipe assembled from strings
//! would produce commands no reader saw, which is precisely the review
//! property amendment 1 exists to protect.
//!
//! # Why the parameter's KIND is declared
//!
//! The field-kind taxonomy is read off the live composer corpus rather than
//! invented (`.fleet/fixtures/corpus/issue-investigations.README.md` at
//! `b1db6ae3…`, the "Which fields are skeleton, which are per-issue" table).
//! A field is `skeleton` (fixed across every dispatch of the rig),
//! `per_item` (supplied per dispatch), `both` (partly fixed, partly per
//! dispatch), or `re_derive` — a value that is READ AT THE DISPATCH MINUTE
//! from a named source and never copied. The fourth kind is the one that
//! carries a law: the README's own words are "at the dispatch minute (not the
//! compose minute — they drift)", and a declared derivation is how compose /
//! fire drift becomes impossible rather than checked-for.
//!
//! # Why a wall carries its exclusions
//!
//! A wall declaration is projected — into prompt content and into mechanical
//! enforcement — from ONE source, so the two renderings cannot disagree. The
//! exclusions beside it are the negative half: terms a round measured out,
//! declared where the next projection will meet them, so reintroducing one by
//! hand is a refusal rather than a review's luck.

use crate::Span;
use crate::ast::{Comment, DocLine, Lead, TypeRef};

/// One declared recipe: typed parameters with declared kinds, and the steps
/// those parameters feed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecipeDecl {
    /// Span covering the declaration's header line.
    pub span: Span,
    /// Leading trivia before the declaration.
    pub lead: Vec<Lead>,
    /// `///` documentation lines attached to the declaration.
    pub docs: Vec<DocLine>,
    /// Same-line comment on the header line.
    pub trailing: Option<Comment>,
    /// The recipe's name.
    pub name: String,
    /// Source span of the name.
    pub name_span: Span,
    /// Declared parameters, in source order.
    pub parameters: Vec<RecipeParam>,
    /// Declared steps, in source order. The ORDER a document happens to
    /// write them in is not the order they run: the dependency edges are.
    pub steps: Vec<RecipeStep>,
}

/// One declared recipe parameter: a name, a type, a kind, an optional
/// default, and — for a `re_derive` parameter — its declared derivation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecipeParam {
    /// Span covering the clause.
    pub span: Span,
    /// The parameter's name.
    pub name: String,
    /// Source span of the name.
    pub name_span: Span,
    /// Declared type.
    pub ty: TypeRef,
    /// The declared kind.
    pub kind: ParameterKind,
    /// Source span of the kind word.
    pub kind_span: Span,
    /// The value this parameter takes when a composition omits it, written
    /// as a literal. ABSENCE IS A LEGAL DECLARED STATE: a parameter with no
    /// default is one a composition must fill, and an unfilled one refuses.
    pub default: Option<ParamLiteral>,
    /// The declared derivation, which exactly the `re_derive` kind carries.
    pub derivation: Option<Derivation>,
}

/// A literal value written into a declaration, with the span it was read at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParamLiteral {
    /// The value's decoded text, verbatim — the bytes a refusal observes.
    pub text: String,
    /// Source span of the literal.
    pub span: Span,
}

/// Which of the four declared kinds a parameter is.
///
/// A closed taxonomy: adding a kind is a design change, and the words are
/// read against this set by name so an unknown kind is refused with the set
/// printed rather than falling through to a general "unexpected word".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterKind {
    /// Fixed across every dispatch of the rig.
    Skeleton,
    /// Supplied per dispatch.
    PerItem,
    /// Partly fixed, partly per dispatch.
    Both,
    /// Read at the FIRE minute from a declared source, never copied.
    ReDerive,
}

impl ParameterKind {
    /// Every kind, in the order the taxonomy reads them.
    pub const ALL: [Self; 4] = [Self::Skeleton, Self::PerItem, Self::Both, Self::ReDerive];

    /// The kind's stable spelling, exactly as an author writes it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Skeleton => "skeleton",
            Self::PerItem => "per_item",
            Self::Both => "both",
            Self::ReDerive => "re_derive",
        }
    }

    /// The kind named by its spelling, or `None` when the word names no kind
    /// of this closed taxonomy — never a nearest match.
    #[must_use]
    pub fn from_word(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == word)
    }

    /// The admitted set, rendered for a diagnostic.
    #[must_use]
    pub fn admitted() -> String {
        Self::ALL
            .into_iter()
            .map(|kind| format!("`{}`", kind.as_str()))
            .collect::<Vec<String>>()
            .join(", ")
    }
}

impl std::fmt::Display for ParameterKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// A declared derivation: WHERE the value is read from, and WHAT is taken
/// out of it.
///
/// Two declared slots, never one string carrying both. An address that
/// encoded its own extraction would be the same defect the one-truth fence
/// closes, written smaller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Derivation {
    /// Span covering the `derives … extract …` tail.
    pub span: Span,
    /// The source address, verbatim.
    pub source: String,
    /// Source span of the address literal.
    pub source_span: Span,
    /// The extraction, verbatim, as the author wrote it.
    pub extraction: String,
    /// Source span of the extraction literal.
    pub extraction_span: Span,
}

/// One declared step of a recipe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecipeStep {
    /// Span covering the step's header line.
    pub span: Span,
    /// The step's name.
    pub name: String,
    /// Source span of the name.
    pub name_span: Span,
    /// The steps this one runs after, in source order. These are the
    /// dependency edges, and they must form a DAG.
    pub after: Vec<StepReference>,
    /// The at-fire preconditions this step declares, in source order.
    pub requires: Vec<ParamLiteral>,
    /// What the step actually does.
    ///
    /// `None` when the step's body was never stated. The AST is the faithful
    /// parse product; that a step MUST say what it does is a check-layer
    /// ruling, not something the grammar can put right.
    pub body: Option<StepBody>,
}

/// One `after <step>` edge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepReference {
    /// Span covering the clause.
    pub span: Span,
    /// The step named.
    pub name: String,
    /// Source span of the name.
    pub name_span: Span,
}

/// What a recipe step does. Exactly three forms, and none of them is a
/// string a shell would re-split.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepBody {
    /// `runs command <name>` — the step's act is one DECLARED command.
    RunsCommand {
        /// Span covering the clause.
        span: Span,
        /// The declared command's name.
        command: String,
        /// Source span of the name.
        command_span: Span,
    },
    /// `fires artifact <name> expecting "<known answer>"` — the step fires a
    /// declared demonstration artifact and compares against a known answer.
    FiresArtifact {
        /// Span covering the clause.
        span: Span,
        /// The declared artifact's name.
        artifact: String,
        /// Source span of the name.
        artifact_span: Span,
        /// The known answer the fire is compared against.
        expecting: String,
        /// Source span of the known-answer literal.
        expecting_span: Span,
    },
    /// `captures <name>: <Type> of artifact <name>` — the step reads a
    /// declared artifact into a typed result.
    CapturesArtifact {
        /// Span covering the clause.
        span: Span,
        /// The capture's name.
        capture: String,
        /// Source span of the capture's name.
        capture_span: Span,
        /// The capture's declared type.
        ty: TypeRef,
        /// The declared artifact's name.
        artifact: String,
        /// Source span of the artifact's name.
        artifact_span: Span,
    },
}

impl StepBody {
    /// The clause's span, whichever form the body is.
    #[must_use]
    pub const fn span(&self) -> Span {
        match self {
            Self::RunsCommand { span, .. }
            | Self::FiresArtifact { span, .. }
            | Self::CapturesArtifact { span, .. } => *span,
        }
    }
}

/// One declared wall: the path patterns it admits and the terms it excludes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WallDecl {
    /// Span covering the declaration's header line.
    pub span: Span,
    /// Leading trivia before the declaration.
    pub lead: Vec<Lead>,
    /// `///` documentation lines attached to the declaration.
    pub docs: Vec<DocLine>,
    /// Same-line comment on the header line.
    pub trailing: Option<Comment>,
    /// The wall's name.
    pub name: String,
    /// Source span of the name.
    pub name_span: Span,
    /// The path patterns the wall admits, in source order.
    pub allows: Vec<ParamLiteral>,
    /// The terms a projection of this wall must never emit, in source order.
    pub excludes: Vec<ParamLiteral>,
}

/// One declared artifact: an address outside the document, its content
/// digest, and what firing it must leave behind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactDecl {
    /// Span covering the declaration's header line.
    pub span: Span,
    /// Leading trivia before the declaration.
    pub lead: Vec<Lead>,
    /// `///` documentation lines attached to the declaration.
    pub docs: Vec<DocLine>,
    /// Same-line comment on the header line.
    pub trailing: Option<Comment>,
    /// The artifact's name, as a step references it.
    pub name: String,
    /// Source span of the name.
    pub name_span: Span,
    /// The address the artifact's bytes live at.
    pub address: String,
    /// Source span of the address literal.
    pub address_span: Span,
    /// The declared content digest, if the declaration states one.
    ///
    /// ABSENCE IS A LEGAL DECLARED STATE at parse; the census is what makes
    /// a digest load-bearing, and a recipe that fires an artifact without
    /// one is refused where the census is measured, not here.
    pub digest: Option<ParamLiteral>,
    /// The path this artifact must restore byte-exactly, if it is a restore
    /// artifact.
    pub restores: Option<ParamLiteral>,
    /// The sentinels whose presence in the artifact's bytes means it was
    /// never filled, in source order.
    pub excludes: Vec<ParamLiteral>,
}
