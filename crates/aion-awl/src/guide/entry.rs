//! The shape of one glossary entry, and the reference sections entries cite.
//!
//! An entry is deliberately small: the word, what kind of word it is, ONE
//! sentence of meaning, and the section of the language reference that
//! explains it in full. The sentence is short enough to sit in a hover
//! tooltip and in `--help` output without either surface paraphrasing it —
//! paraphrase is how the same rule comes to be written three times and mean
//! three things.

/// What kind of word a glossary entry describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WordKind {
    /// A reserved word. The lexer produces a [`crate::Keyword`] token for it,
    /// so it can never be an identifier and its meaning never depends on
    /// where it stands.
    Reserved,
    /// A positional word. It lexes as an ordinary identifier and is
    /// recognised only in the one grammar position where nothing else may
    /// stand, so that documents carrying it as a name keep working.
    Positional,
}

impl WordKind {
    /// The noun a surface prints for this kind.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Reserved => "keyword",
            Self::Positional => "word",
        }
    }
}

/// A numbered section of the AWL language reference.
///
/// The number and the title are carried together on purpose. A number alone
/// silently survives a document being re-ordered; the pair does not, because
/// `guide_reference_gate` reads the reference and refuses any entry whose
/// number and title do not both match a heading that is actually there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Section {
    number: u8,
    title: &'static str,
}

impl Section {
    /// Names a section by its number and its exact heading text.
    const fn new(number: u8, title: &'static str) -> Self {
        Self { number, title }
    }

    /// The section number, as the reference prints it.
    #[must_use]
    pub const fn number(self) -> u8 {
        self.number
    }

    /// The section heading, exactly as the reference writes it.
    #[must_use]
    pub const fn title(self) -> &'static str {
        self.title
    }
}

/// The language reference every entry cites, relative to the repository root.
///
/// It lives beside the bundled assistant's other resources because it is the
/// same document the assistant is given. One copy that cannot drift beats two
/// that can.
pub const REFERENCE_PATH: &str = "examples/assistant/resources/AWL-REFERENCE.md";

/// The authoring guide that introduces the language, relative to the
/// repository root.
pub const GUIDE_PATH: &str = "docs/authoring/AWL.md";

/// The practical objective-to-checked-document walkthrough.
pub const AUTHORING_PATH: &str = "examples/assistant/resources/AWL-AUTHORING.md";

/// What a `worker` block declares, and what actually serves it.
pub const WORKERS_PATH: &str = "examples/assistant/resources/WORKERS.md";

/// Every CLI command in the AWL loop, with its honest failure modes.
pub const COMMANDS_PATH: &str = "examples/assistant/resources/COMMANDS.md";

/// What a declared `run "…"` body can do. There is no sandbox.
pub const DECLARED_COMMANDS_PATH: &str = "docs/workers/DECLARED-COMMANDS.md";

/// §1 of the reference.
pub(crate) const DOCUMENT_STRUCTURE: Section = Section::new(1, "Document structure");
/// §2 of the reference.
pub(crate) const VALUES_AND_TYPES: Section = Section::new(2, "Values and types");
/// §3 of the reference.
pub(crate) const WORKFLOW_HEADER: Section = Section::new(3, "Workflow header");
/// §4 of the reference.
pub(crate) const WORKERS_AND_ACTIONS: Section = Section::new(4, "Workers, actions, and calls");
/// §5 of the reference.
pub(crate) const CHILDREN_AND_SUBFLOWS: Section = Section::new(5, "Children and subflows");
/// §6 of the reference.
pub(crate) const STEPS_AND_CALLS: Section = Section::new(6, "Steps, dependencies, and calls");
/// §7 of the reference.
pub(crate) const OUTCOMES_AND_ROUTES: Section = Section::new(7, "Selection, outcomes, and routes");
/// §8 of the reference.
pub(crate) const PARALLEL_WORK: Section = Section::new(8, "Parallel and sequential work");
/// §9 of the reference.
pub(crate) const BOUNDED_ITERATION: Section = Section::new(9, "Bounded iteration");
/// §10 of the reference.
pub(crate) const WAITS_AND_TIMERS: Section =
    Section::new(10, "Waits, timeouts, signals, and sleep");
/// §11 of the reference.
pub(crate) const FAILURE_HANDLING: Section = Section::new(11, "Failure handling");
/// §12 of the reference.
pub(crate) const SENDING_SIGNALS: Section = Section::new(12, "Sending signals to other workflows");
/// §13 of the reference.
pub(crate) const QUERY_SURFACE: Section = Section::new(13, "The read-only query surface");

/// One word of AWL, as the language reference explains it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuideEntry {
    word: &'static str,
    kind: WordKind,
    summary: &'static str,
    section: Section,
}

impl GuideEntry {
    /// Records one word's meaning and where its full treatment lives.
    pub(crate) const fn new(
        word: &'static str,
        kind: WordKind,
        summary: &'static str,
        section: Section,
    ) -> Self {
        Self {
            word,
            kind,
            summary,
            section,
        }
    }

    /// The word itself, exactly as an author types it.
    #[must_use]
    pub const fn word(self) -> &'static str {
        self.word
    }

    /// Whether the word is reserved or recognised positionally.
    #[must_use]
    pub const fn kind(self) -> WordKind {
        self.kind
    }

    /// One sentence of meaning.
    #[must_use]
    pub const fn summary(self) -> &'static str {
        self.summary
    }

    /// The reference section that explains the word in full.
    #[must_use]
    pub const fn section(self) -> Section {
        self.section
    }

    /// The citation a surface prints under the summary.
    ///
    /// Every surface renders the same string, so an author who reads the
    /// hover and an author who reads `aion awl guide` are sent to the same
    /// place by the same words.
    #[must_use]
    pub fn citation(self) -> String {
        format!(
            "{REFERENCE_PATH} §{} — {}",
            self.section.number(),
            self.section.title()
        )
    }
}
