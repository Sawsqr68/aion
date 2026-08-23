//! The shape of one glossary entry, and the reference sections entries cite.
//!
//! An entry is deliberately small: the word, what kind of word it is, ONE
//! sentence of meaning, its complete example documents, and the section of
//! the language reference that explains it in full. The sentence is short
//! enough to sit in a hover tooltip and in `--help` output without either
//! surface paraphrasing it — paraphrase is how the same rule comes to be
//! written three times and mean three things. Each example is a whole `.awl`
//! document rather than a fragment, because a fragment teaches syntax
//! without teaching where it stands; every example passes the crate's own
//! parse and check pipeline, gated by
//! `every_entry_example_passes_the_real_checker`. The example texts live in
//! [`super::examples`], one file per vocabulary cluster.

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

/// The command that prints the full embedded reference from the installed
/// binary.
///
/// The citation names the reference by section, not by repository path — an
/// author who installed `aion` from crates.io has no repository to resolve a
/// path against. This is the companion every surface may print so that author
/// still has somewhere real to go.
pub const REFERENCE_COMMAND: &str = "aion awl guide --reference";

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
/// §17 of the reference.
pub(crate) const COMMAND_DECLARATIONS: Section = Section::new(17, "Command declarations");

/// One word of AWL, as the language reference explains it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuideEntry {
    word: &'static str,
    kind: WordKind,
    summary: &'static str,
    section: Section,
    examples: &'static [&'static str],
}

impl GuideEntry {
    /// Records one word's meaning, where its full treatment lives, and the
    /// complete documents that demonstrate it.
    pub(crate) const fn new(
        word: &'static str,
        kind: WordKind,
        summary: &'static str,
        section: Section,
        examples: &'static [&'static str],
    ) -> Self {
        Self {
            word,
            kind,
            summary,
            section,
            examples,
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

    /// The complete, minimal `.awl` documents demonstrating the word in
    /// realistic use — one per honest form or context the word has.
    ///
    /// Whole documents rather than fragments, so an author can paste any one
    /// into a file and run `aion awl check` on it unchanged. A word with
    /// several genuinely distinct uses (a declaration form and a call form, a
    /// workflow-document form and a worker-document form) carries one example
    /// per use; a word with exactly one honest form carries one — a contrived
    /// near-duplicate teaches less than a single good document. Every example
    /// is run through the crate's real parse and check pipeline by the guide
    /// tests, so a syntax change that broke one fails the build instead of
    /// teaching dead syntax.
    #[must_use]
    pub const fn examples(self) -> &'static [&'static str] {
        self.examples
    }

    /// The citation a surface prints under the summary.
    ///
    /// Every surface renders the same string, so an author who reads the
    /// hover and an author who reads `aion awl guide` are sent to the same
    /// place by the same words. The reference is named by section rather than
    /// by repository path — the installed binary carries the full text, and
    /// [`REFERENCE_COMMAND`] is how a surface says where to open it.
    #[must_use]
    pub fn citation(self) -> String {
        format!(
            "AWL reference §{} — {}",
            self.section.number(),
            self.section.title()
        )
    }
}
