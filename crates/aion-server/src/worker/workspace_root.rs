//! `{workspace_root}` expansion — for declared action bodies, and for the
//! path-valued settings of a worker document's `harness` section.
//!
//! The two consumers differ in one way that matters and are otherwise the same
//! act. A declared BODY is a command string that gets parsed into an argv, so
//! the splice must not change how it parses ([`WorkspaceRoot::expand`]). A
//! harness SETTING is one whole path that nothing parses, so those rules do not
//! apply to it and are not imposed on it ([`WorkspaceRoot::expand_setting`]);
//! what does apply is the root being resolvable, absolute, spellable and
//! spawnable. Both read the ONE resolved root, so a body and a setting in the
//! same document can never disagree about where this box's workspaces live.
//!
//! A declared body runs with its environment cleared to `PATH` only, so it
//! cannot expand `~` or read a variable to learn where session workspaces
//! live. The workflow's start inputs cannot carry the location either — the
//! operator's start contract is deliberately minimal, and the workspace root
//! is a property of the SERVER, not of any one run. The server therefore
//! states it: a declared command may carry the literal placeholder
//! [`WORKSPACE_ROOT_PLACEHOLDER`], and the dispatch path replaces every
//! occurrence with the server's own workspace root before the command is
//! parsed.
//!
//! The root is the aion home's `clones/` directory
//! ([`crate::config::aion_home`] → `<home>/clones`) — the same location the
//! crate worker's provision handlers established (#175): durable history
//! records workspace paths, so they must live somewhere that survives a host
//! reboot, never the OS temp dir. There is no separate configurable value;
//! the home is already the operator's one answer to "where does this
//! server's state live" (#113), and resolving a second answer here is how
//! two components come to disagree about one path.
//!
//! Resolution happens ONCE, at server-state construction, and every consumer
//! — the declared-body dispatcher, the startup banner — reads that one
//! value. A resolution failure is carried, not raised: boot proceeds, and
//! the failure surfaces as a terminal dispatch refusal when (and only when)
//! a placeholder-bearing body is dispatched. Bodies that do not use the
//! placeholder are untouched by resolution failure.
//!
//! Expansion happens on the raw command STRING, before
//! [`aion_worker::shell::ShellAction`] parses it into an argv. A root whose
//! text would change that parse — whitespace splits a word, `$` opens a
//! parameter reference, a quote opens a quoted region, NUL cannot cross
//! `execve` — is refused rather than spliced, because a silent reshape of
//! the declared command is exactly what the template layer exists to
//! prevent. The root is server-controlled, so the refusal is theoretical;
//! it is checked because "theoretical" is not "impossible".
//!
//! The COMMAND is held to the same standard: every placeholder occurrence
//! must stand alone as one whole, unquoted argv word of the command as the
//! template parses it. An occurrence inside a quoted region or glued to
//! adjacent text would make the spliced root a FRAGMENT of some larger
//! word, so what executes would not be the path the server resolved — the
//! dispatch is refused by name instead ([`WorkspaceRootError::PlaceholderMisplaced`]).

use std::path::{Path, PathBuf};

use thiserror::Error;

/// The literal placeholder a declared command carries where the server's
/// workspace root belongs.
///
/// Defined by the LANGUAGE crate, not here. It is a token of the documents
/// this server executes, and the AWL checker refuses a harness path setting by
/// the same spelling this module splices — one definition in the crate both
/// already depend on is that agreement, where a second copy would be the drift.
pub use aion_awl::WORKSPACE_ROOT_PLACEHOLDER;

/// The directory under the aion home where session workspaces live.
const CLONES_DIRECTORY: &str = "clones";

/// Why the workspace root could not be resolved or spliced into a declared
/// command.
///
/// Every variant is terminal at dispatch time: the root is a property of the
/// server's configuration and filesystem, so retrying the dispatch cannot
/// change it.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum WorkspaceRootError {
    /// The aion home itself could not be resolved, so there is no root to
    /// derive.
    #[error("the aion home cannot be resolved, so there is no workspace root: {reason}")]
    Unresolvable {
        /// The home resolution's own diagnosis.
        reason: String,
    },
    /// The resolved root is not an absolute path.
    ///
    /// A relative root resolves against the server's current directory, so
    /// the same recorded history would name a different location after a
    /// restart from elsewhere.
    #[error(
        "the workspace root `{path}` is not an absolute path; a relative root names a \
         different location after a restart from a different directory"
    )]
    NotAbsolute {
        /// The offending path, rendered for the refusal.
        path: String,
    },
    /// The resolved root is not valid UTF-8, so it has no faithful spelling
    /// inside a declared command string.
    #[error(
        "the workspace root `{path}` is not valid UTF-8, so it cannot be spliced into a \
         declared command"
    )]
    NotUnicode {
        /// The offending path, rendered lossily for the refusal.
        path: String,
    },
    /// The resolved root contains a character that would change how the
    /// declared command parses after splicing.
    #[error(
        "the workspace root `{path}` contains {character}, which would change the parsed \
         shape of the declared command it is spliced into"
    )]
    ShapeChanging {
        /// The offending path, rendered for the refusal.
        path: String,
        /// Which shape-changing character was found, named for the refusal.
        character: &'static str,
    },
    /// A placeholder occurrence in the declared command is not a whole,
    /// unquoted argv word, so the spliced root would become a fragment of
    /// some larger word instead of the path the server resolved.
    #[error(
        "the declared command carries {{workspace_root}} {placement}; every occurrence \
         must stand alone as one whole, unquoted argv word, because the root is spliced \
         into the command string before it is parsed"
    )]
    PlaceholderMisplaced {
        /// Where the offending occurrence sits, named for the refusal.
        placement: &'static str,
    },
    /// The resolved root contains a NUL byte, so no process can be launched in
    /// a path spliced with it.
    ///
    /// The command-splicing path refuses NUL under
    /// [`WorkspaceRootError::ShapeChanging`] alongside the characters that
    /// would reshape a parse. A harness SETTING is never parsed, so NUL is the
    /// only one of that set which still bites — and it bites for its own
    /// reason, one layer lower: `execve` takes NUL-terminated strings, so a
    /// path carrying one cannot be handed to the kernel at all.
    #[error(
        "the workspace root `{path}` contains a NUL byte, which cannot cross `execve`, so no \
         process can be launched in it"
    )]
    NotSpawnable {
        /// The offending path, rendered for the refusal.
        path: String,
    },
    /// The root directory does not exist and could not be created.
    #[error("the workspace root directory `{path}` could not be created: {error}")]
    CreationFailed {
        /// The directory that could not be created.
        path: String,
        /// The io error's own diagnosis.
        error: String,
    },
}

/// A declared command with its workspace-root placeholder expanded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpandedCommand {
    /// The command with every placeholder occurrence replaced by the root.
    pub command: String,
    /// The resolved root that was spliced in, for the dispatch log.
    pub workspace_root: String,
}

/// The server's one workspace root, resolved once and read everywhere.
///
/// Carries the RESULT of resolution rather than requiring it: a server whose
/// home cannot resolve must still boot (nothing else needs the root), so the
/// failure is held here and surfaces as a terminal refusal at the first
/// dispatch of a placeholder-bearing body.
#[derive(Debug, Clone)]
pub struct WorkspaceRoot {
    resolution: Result<PathBuf, WorkspaceRootError>,
}

impl WorkspaceRoot {
    /// Resolve the workspace root from the server's aion home:
    /// [`crate::config::aion_home`] → `<home>/clones`.
    ///
    /// This is the ONE derivation of the value. Everything downstream — the
    /// declared-body dispatcher, the startup banner, any composition point
    /// reading the banner — consumes the resolved value rather than
    /// re-deriving it, so two components can never disagree about where
    /// workspaces live.
    #[must_use]
    pub fn resolve() -> Self {
        let resolution = crate::config::aion_home()
            .map(|home| root_under(&home.path))
            .map_err(|error| WorkspaceRootError::Unresolvable {
                reason: error.to_string(),
            });
        Self { resolution }
    }

    /// Build a root from an already-decided resolution.
    ///
    /// For embedders and tests that need a known root (or a known failure)
    /// without touching the process environment the production
    /// [`WorkspaceRoot::resolve`] reads. Single derivation is a convention
    /// the production path upholds, not an enforced invariant: nothing stops
    /// a caller constructing a second, disagreeing root here.
    #[must_use]
    pub const fn from_resolution(resolution: Result<PathBuf, WorkspaceRootError>) -> Self {
        Self { resolution }
    }

    /// The one-line rendering of this root for the startup banner: the
    /// resolved path's display form, or `unresolvable: <reason>` when
    /// resolution failed — never a fabricated value.
    #[must_use]
    pub fn banner_value(&self) -> String {
        match self.resolved() {
            Ok(path) => path.display().to_string(),
            Err(error) => format!("unresolvable: {error}"),
        }
    }

    /// The resolved root, or why there is none.
    ///
    /// Read-only: reporting the value (the startup banner) must not create
    /// the directory. Creation happens at expansion time, where a failure
    /// has a dispatch to refuse.
    ///
    /// # Errors
    ///
    /// Returns the held [`WorkspaceRootError`] when resolution failed.
    pub fn resolved(&self) -> Result<&Path, &WorkspaceRootError> {
        match &self.resolution {
            Ok(path) => Ok(path.as_path()),
            Err(error) => Err(error),
        }
    }

    /// Expand every [`WORKSPACE_ROOT_PLACEHOLDER`] occurrence in `command`
    /// with the resolved root.
    ///
    /// A command without the placeholder is untouched — `Ok(None)`, no
    /// resolution requirement, no filesystem side effect — so an unresolved
    /// root never affects a body that does not use it. A command WITH the
    /// placeholder requires the full chain: every occurrence standing alone
    /// as one whole, unquoted argv word, then a resolved, absolute, UTF-8
    /// root with no shape-changing characters, and an existing directory
    /// (created here, idempotently and owner-only `0700`, if missing — an
    /// already-existing root keeps whatever permissions the operator gave
    /// it; they are the operator's own).
    ///
    /// # Errors
    ///
    /// Returns [`WorkspaceRootError`] when a placeholder occurrence is not a
    /// whole unquoted argv word, when the root is unresolved, not absolute,
    /// not valid UTF-8, contains a character that would change the command's
    /// parsed shape, or does not exist and cannot be created.
    pub fn expand(&self, command: &str) -> Result<Option<ExpandedCommand>, WorkspaceRootError> {
        if !command.contains(WORKSPACE_ROOT_PLACEHOLDER) {
            return Ok(None);
        }
        if let Some(placement) = misplaced_placeholder(command) {
            return Err(WorkspaceRootError::PlaceholderMisplaced { placement });
        }
        let (root, root_text) = self.spliceable_root()?;
        if let Some(character) = shape_changing_character(root_text) {
            return Err(WorkspaceRootError::ShapeChanging {
                path: root_text.to_owned(),
                character,
            });
        }
        create_root_directory(root).map_err(|error| WorkspaceRootError::CreationFailed {
            path: root_text.to_owned(),
            error: error.to_string(),
        })?;
        Ok(Some(ExpandedCommand {
            command: command.replace(WORKSPACE_ROOT_PLACEHOLDER, root_text),
            workspace_root: root_text.to_owned(),
        }))
    }

    /// Expand every [`WORKSPACE_ROOT_PLACEHOLDER`] occurrence in a harness
    /// section's PATH-VALUED setting — an ACP `command`, either kind's `cwd`, a
    /// Norn `binary` — with the resolved root.
    ///
    /// A value without the placeholder is untouched: `Ok(None)`, no resolution
    /// requirement, no filesystem side effect. A document that names an
    /// absolute path outright is therefore unaffected by an unresolvable root,
    /// exactly as it is unaffected by one today.
    ///
    /// The rules that govern [`WorkspaceRoot::expand`]'s COMMAND splice are
    /// deliberately NOT applied here, because a setting is not a command:
    ///
    /// - No placeholder-placement rule. A command is split into argv words, so
    ///   an occurrence glued to other text would splice a fragment of some
    ///   larger word. A setting is one whole value that nothing splits, so
    ///   `{workspace_root}/agents` is the ordinary way to write it. Where the
    ///   placeholder may stand is the LANGUAGE's rule, refused at
    ///   `aion_awl::harness_path_form` — it must lead the value — and this
    ///   layer does not restate it.
    /// - No shape-changing-character rule, save NUL. Whitespace, `$` and quotes
    ///   change how a command PARSES; a directory named `/Users/a b` is simply
    ///   a directory, and refusing it here would refuse a legitimate path for a
    ///   reason that belongs to a different consumer. NUL survives the cut on
    ///   its own footing: `execve` cannot carry it.
    ///
    /// The ROOT directory is created (idempotently, owner-only `0700`) exactly
    /// as the command path creates it, and for the harder reason: a process
    /// cannot be started in a directory that does not exist. Only the root —
    /// a value naming something BENEATH it (`{workspace_root}/assistant`) is
    /// the document's claim about a tree that should already be there, and
    /// conjuring an empty one would turn a mistyped path into an agent working
    /// against nothing. The adapter refuses a missing working directory by
    /// name, before it spawns.
    ///
    /// # Errors
    ///
    /// Returns [`WorkspaceRootError`] when the root is unresolved, not
    /// absolute, not valid UTF-8, carries a NUL byte, or does not exist and
    /// cannot be created. Every one of them is terminal — the root is a
    /// property of this box's configuration and filesystem, so a second attempt
    /// reads the same answer.
    pub fn expand_setting(&self, value: &str) -> Result<Option<String>, WorkspaceRootError> {
        if !value.contains(WORKSPACE_ROOT_PLACEHOLDER) {
            return Ok(None);
        }
        let (root, root_text) = self.spliceable_root()?;
        if root_text.contains('\0') {
            return Err(WorkspaceRootError::NotSpawnable {
                path: root_text.to_owned(),
            });
        }
        create_root_directory(root).map_err(|error| WorkspaceRootError::CreationFailed {
            path: root_text.to_owned(),
            error: error.to_string(),
        })?;
        Ok(Some(value.replace(WORKSPACE_ROOT_PLACEHOLDER, root_text)))
    }

    /// The resolved root as both a path and its text, or why it cannot be
    /// spliced into anything at all.
    ///
    /// The three requirements every consumer shares, stated once: resolution
    /// succeeded, the root is absolute (a relative one would name a different
    /// place after a restart from elsewhere), and it has a faithful UTF-8
    /// spelling to splice.
    fn spliceable_root(&self) -> Result<(&Path, &str), WorkspaceRootError> {
        let root = self.resolution.as_ref().map_err(Clone::clone)?;
        if !root.is_absolute() {
            return Err(WorkspaceRootError::NotAbsolute {
                path: root.to_string_lossy().into_owned(),
            });
        }
        let root_text = root
            .to_str()
            .ok_or_else(|| WorkspaceRootError::NotUnicode {
                path: root.to_string_lossy().into_owned(),
            })?;
        Ok((root.as_path(), root_text))
    }
}

/// Derive the workspace root from the aion home: `<home>/clones`.
///
/// The ONE derivation rule, factored pure so a test can pin it without
/// resolving a real home. The location is the crate worker's established one
/// (#175): durable history records workspace paths, so they live under the
/// home, never the OS temp dir.
fn root_under(home: &Path) -> PathBuf {
    home.join(CLONES_DIRECTORY)
}

/// Create the root directory (and any missing ancestors), owner-only.
///
/// New directories are minted `0700`: the root holds session workspaces —
/// operator repositories and agent working trees — so nothing else on the
/// host gets a default read into them. An ALREADY-existing root is left
/// exactly as found; its permissions are the operator's own configuration,
/// not this function's to correct.
fn create_root_directory(root: &Path) -> std::io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    // The crate's portability gate is target-cfg (see `[target.'cfg(unix)']`
    // in Cargo.toml); the mode is a unix concept, so it is gated the same way.
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt as _;
        builder.mode(0o700);
    }
    builder.create(root)
}

/// How a stretch of the raw command string is quoted, by the SAME rules the
/// worker SDK's command template parses with
/// (`aion_worker::shell::template::CommandTemplate::parse`): single quotes
/// are literal until the closing single quote, double quotes likewise, and
/// each region is entered only from unquoted text.
#[derive(Clone, Copy, PartialEq, Eq)]
enum QuoteContext {
    /// Plain command text: whitespace here splits words.
    Unquoted,
    /// Inside a `'…'` region.
    Single,
    /// Inside a `"…"` region.
    Double,
}

/// The template-rule quoting context at byte `position` of `command`.
///
/// An unterminated quote leaves the tail of the command inside the region,
/// which is also how the template treats it (it refuses the parse) — so a
/// placeholder after an unclosed quote reads as quoted here and is refused.
fn quote_context_at(command: &str, position: usize) -> QuoteContext {
    let mut context = QuoteContext::Unquoted;
    for (index, character) in command.char_indices() {
        if index >= position {
            break;
        }
        context = match (context, character) {
            (QuoteContext::Unquoted, '\'') => QuoteContext::Single,
            (QuoteContext::Unquoted, '"') => QuoteContext::Double,
            (QuoteContext::Single, '\'') | (QuoteContext::Double, '"') => QuoteContext::Unquoted,
            (current, _) => current,
        };
    }
    context
}

/// Where the first misplaced placeholder occurrence sits, named for the
/// refusal — or `None` when EVERY occurrence stands alone as one whole,
/// unquoted argv word of the command as the template parses it.
///
/// Walked with the template's own quoting rules rather than a regex guess:
/// a word boundary is unquoted whitespace (or an end of the command), and a
/// neighbouring quote character glues the occurrence into a larger word just
/// as any other adjacent text does.
fn misplaced_placeholder(command: &str) -> Option<&'static str> {
    for (start, _) in command.match_indices(WORKSPACE_ROOT_PLACEHOLDER) {
        let end = start + WORKSPACE_ROOT_PLACEHOLDER.len();
        // The placeholder contains no quote characters, so the context of its
        // first byte is the context of the whole occurrence.
        match quote_context_at(command, start) {
            QuoteContext::Single => return Some("inside single quotes"),
            QuoteContext::Double => return Some("inside double quotes"),
            QuoteContext::Unquoted => {}
        }
        // An unquoted position's immediate neighbours are unquoted too (a
        // quoted region can only end at its own closing quote, which is never
        // whitespace), so plain whitespace checks are the template's word
        // boundaries here.
        let starts_a_word = command[..start]
            .chars()
            .next_back()
            .is_none_or(char::is_whitespace);
        let ends_a_word = command[end..]
            .chars()
            .next()
            .is_none_or(char::is_whitespace);
        if !starts_a_word || !ends_a_word {
            return Some("glued to adjacent text");
        }
    }
    None
}

/// The first character in `root` that would change a declared command's
/// parsed shape, named for the refusal — or `None` when the root splices
/// cleanly.
///
/// The set mirrors what the worker SDK's command template gives meaning to:
/// whitespace splits words, `$` opens a parameter reference, `'` and `"`
/// open quoted regions, and NUL cannot cross `execve` at all.
fn shape_changing_character(root: &str) -> Option<&'static str> {
    for character in root.chars() {
        if character.is_whitespace() {
            return Some("whitespace");
        }
        match character {
            '$' => return Some("`$`"),
            '\'' => return Some("a single quote"),
            '"' => return Some("a double quote"),
            '\0' => return Some("a NUL byte"),
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use std::path::Path;

    use super::{
        ExpandedCommand, WORKSPACE_ROOT_PLACEHOLDER, WorkspaceRoot, WorkspaceRootError, root_under,
        shape_changing_character,
    };

    /// What a test returns. Every fallible step is carried rather than
    /// unwrapped, because the workspace denies panicking accessors in test
    /// code as firmly as in library code.
    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn unresolved() -> WorkspaceRoot {
        WorkspaceRoot::from_resolution(Err(WorkspaceRootError::Unresolvable {
            reason: "AION_HOME must not be empty".to_owned(),
        }))
    }

    #[test]
    fn expansion_replaces_every_occurrence() -> TestResult {
        let scratch = tempfile::tempdir()?;
        let root = scratch.path().join("clones");
        let workspace = WorkspaceRoot::from_resolution(Ok(root.clone()));
        let root_text = root.to_string_lossy().into_owned();
        let expanded = workspace
            .expand("sh -c 'x' -- {workspace_root} $run_id {workspace_root}")?
            .ok_or("a placeholder-bearing command must expand")?;
        assert_eq!(
            expanded,
            ExpandedCommand {
                command: format!("sh -c 'x' -- {root_text} $run_id {root_text}"),
                workspace_root: root_text,
            }
        );
        Ok(())
    }

    #[test]
    fn a_command_without_the_placeholder_is_not_expanded() -> TestResult {
        let scratch = tempfile::tempdir()?;
        let resolved = WorkspaceRoot::from_resolution(Ok(scratch.path().join("clones")));
        assert_eq!(resolved.expand("echo $greeting")?, None);
        // Resolution failure must not touch a body that does not use the
        // placeholder — the refusal is scoped to bodies that need the root.
        assert_eq!(unresolved().expand("echo $greeting")?, None);
        Ok(())
    }

    #[test]
    fn a_setting_expands_to_the_root_and_the_root_directory_is_made() -> TestResult {
        let scratch = tempfile::tempdir()?;
        let root = scratch.path().join("clones");
        let workspace = WorkspaceRoot::from_resolution(Ok(root.clone()));
        let root_text = root.to_string_lossy().into_owned();
        assert!(
            !root.exists(),
            "the fixture must start with the root ABSENT, or the creation claim is vacuous"
        );

        assert_eq!(
            workspace.expand_setting(WORKSPACE_ROOT_PLACEHOLDER)?,
            Some(root_text.clone()),
            "a bare placeholder is the root itself"
        );
        assert_eq!(
            workspace.expand_setting("{workspace_root}/assistant")?,
            Some(format!("{root_text}/assistant")),
            "a setting is one whole value: the placeholder leads it and the rest rides along"
        );
        assert!(
            root.is_dir(),
            "a setting the harness will spawn in must exist by the time it is handed over"
        );
        Ok(())
    }

    #[test]
    fn a_setting_without_the_placeholder_is_not_expanded() -> TestResult {
        let scratch = tempfile::tempdir()?;
        let resolved = WorkspaceRoot::from_resolution(Ok(scratch.path().join("clones")));
        assert_eq!(resolved.expand_setting("/srv/agents/workspace")?, None);
        // The document that names its own absolute path is exactly as launchable
        // on a box with no resolvable home as it was before expansion existed.
        assert_eq!(unresolved().expand_setting("/srv/agents/workspace")?, None);
        Ok(())
    }

    #[test]
    // No `TestResult`: every arm here is an assertion on a refusal, so there is no `?` to
    // propagate and a wrapper return would be a shape this test never uses.
    fn a_setting_carrying_the_placeholder_needs_a_usable_root() {
        assert!(
            matches!(
                unresolved().expand_setting("{workspace_root}/assistant"),
                Err(WorkspaceRootError::Unresolvable { .. })
            ),
            "a box with no resolvable home refuses the launch by name"
        );
        assert!(
            matches!(
                WorkspaceRoot::from_resolution(Ok(PathBuf::from("relative/clones")))
                    .expand_setting("{workspace_root}/assistant"),
                Err(WorkspaceRootError::NotAbsolute { .. })
            ),
            "a relative root cannot make an absolute setting"
        );
        assert!(
            matches!(
                WorkspaceRoot::from_resolution(Ok(PathBuf::from("/srv/clo\0nes")))
                    .expand_setting("{workspace_root}")
                    .as_ref(),
                Err(WorkspaceRootError::NotSpawnable { .. })
            ),
            "a NUL cannot cross execve, so no process could ever be started there"
        );
    }

    #[test]
    fn a_setting_accepts_a_root_the_command_splice_refuses() -> TestResult {
        // The one place the two consumers deliberately diverge, pinned WITH its
        // control so the pin cannot pass by accident: a root containing a space
        // would change how a declared command parses and is refused there, while
        // a directory with a space in its name is just a directory and must be
        // usable as a working directory. If a future edit routes settings through
        // the command rules, the first assertion breaks; if it drops the command
        // rules, the second breaks.
        let scratch = tempfile::tempdir()?;
        let root = scratch.path().join("my clones");
        let workspace = WorkspaceRoot::from_resolution(Ok(root.clone()));
        assert!(
            matches!(
                workspace.expand("ls {workspace_root}"),
                Err(WorkspaceRootError::ShapeChanging {
                    character: "whitespace",
                    ..
                })
            ),
            "the command splice still refuses a root that would reshape the parse"
        );
        assert_eq!(
            workspace.expand_setting("{workspace_root}")?,
            Some(root.to_string_lossy().into_owned()),
            "the same root is a perfectly good directory to stand in"
        );
        Ok(())
    }

    #[test]
    fn the_root_is_derived_as_the_homes_clones_directory() {
        // Pins the derivation rule AND the directory name: renaming
        // `CLONES_DIRECTORY` (or changing the rule) must break here, because
        // durable history already records paths under this exact location.
        assert_eq!(
            root_under(Path::new("/x")),
            std::path::PathBuf::from("/x/clones")
        );
        assert_eq!(
            root_under(Path::new("/Users/operator/.aion")),
            std::path::PathBuf::from("/Users/operator/.aion/clones")
        );
    }

    #[test]
    fn the_banner_value_reports_the_path_or_the_failure() -> TestResult {
        let scratch = tempfile::tempdir()?;
        let root = scratch.path().join("clones");
        let resolved = WorkspaceRoot::from_resolution(Ok(root.clone()));
        assert_eq!(resolved.banner_value(), root.display().to_string());
        let failed = unresolved().banner_value();
        assert!(
            failed.starts_with("unresolvable: "),
            "an unresolved root must be reported as exactly that: {failed}"
        );
        assert!(
            failed.contains("AION_HOME must not be empty"),
            "the banner must carry the resolution failure's own reason: {failed}"
        );
        Ok(())
    }

    #[test]
    fn a_placeholder_that_is_a_whole_bare_word_is_accepted() -> TestResult {
        let scratch = tempfile::tempdir()?;
        let workspace = WorkspaceRoot::from_resolution(Ok(scratch.path().join("clones")));
        assert!(
            workspace
                .expand("sh -c 'x' -- {workspace_root} $run_id")?
                .is_some(),
            "a bare-word placeholder must expand"
        );
        Ok(())
    }

    #[test]
    fn a_misplaced_placeholder_is_refused_naming_the_placement() -> TestResult {
        let scratch = tempfile::tempdir()?;
        let root = scratch.path().join("clones");
        for (command, placement) in [
            // Inside a single-quoted region the template takes it literally,
            // so the spliced root would hide inside one quoted word.
            ("sh -c '{workspace_root}'", "inside single quotes"),
            // Likewise inside double quotes.
            ("echo \"{workspace_root}\"", "inside double quotes"),
            // Glued to preceding text.
            ("echo x{workspace_root}", "glued to adjacent text"),
            // Glued to following text.
            ("echo {workspace_root}/sub", "glued to adjacent text"),
            // Glued on both sides at once.
            ("echo x{workspace_root}/sub", "glued to adjacent text"),
            // Glued to a quoted region: adjacency, not quoting, is the defect.
            ("echo ''{workspace_root}", "glued to adjacent text"),
            // A `$` prefix would make the template read `${workspace_root}` as
            // a parameter reference, not the placeholder at all.
            ("echo ${workspace_root}", "glued to adjacent text"),
            // ONE misplaced occurrence refuses even when another is bare.
            (
                "echo {workspace_root} x{workspace_root}",
                "glued to adjacent text",
            ),
        ] {
            let workspace = WorkspaceRoot::from_resolution(Ok(root.clone()));
            let Err(error) = workspace.expand(command) else {
                return Err(format!("command {command:?} must be refused").into());
            };
            assert_eq!(
                error,
                WorkspaceRootError::PlaceholderMisplaced { placement },
                "command {command:?} must be refused as {placement}"
            );
        }
        assert!(
            !root.exists(),
            "a refused command must not create the root directory"
        );
        Ok(())
    }

    #[test]
    fn the_shipped_provision_commands_pass_the_placement_guard() -> TestResult {
        // The standing guard for the two live documents: their `run` bodies
        // carry the placeholder as `-- {workspace_root}`, a whole bare word,
        // and must keep expanding. Derived from the documents themselves so
        // an edit to either body is held here, not at the first dispatch.
        let scratch = tempfile::tempdir()?;
        let workspace = WorkspaceRoot::from_resolution(Ok(scratch.path().join("clones")));
        for (name, source) in [
            (
                "assistant.awl",
                crate::assistant::EMBEDDED_ASSISTANT_DOCUMENT,
            ),
            (
                "assistant_spike.awl",
                include_str!("../../../../examples/assistant/awl/assistant_spike.awl"),
            ),
        ] {
            let compiled = aion_awl::compile(source, Path::new("."))
                .map_err(|error| format!("{name} must compile: {error}"))?;
            let command = compiled
                .contract
                .workers
                .iter()
                .flat_map(|worker| &worker.actions)
                .find_map(|action| match &action.body {
                    Some(aion_package::ActionBodyContract::Run { command })
                        if action.name == "assistant_provision" =>
                    {
                        Some(command.clone())
                    }
                    _ => None,
                })
                .ok_or_else(|| format!("{name} must declare a bodied assistant_provision"))?;
            assert!(
                workspace.expand(&command)?.is_some(),
                "{name}'s provision body must pass the placement guard and expand"
            );
        }
        Ok(())
    }

    #[tokio::test]
    async fn every_accepted_printable_ascii_root_survives_the_real_parser() -> TestResult {
        // The refusal set is tied to the REAL parser, not to an enumeration:
        // for every printable ASCII character, either `expand` refuses the
        // root outright, or the expanded probe command — the placeholder as a
        // bare argv word — executes through the worker SDK's own
        // `ShellAction` and observes EXACTLY the root as its argument.
        let scratch = tempfile::tempdir()?;
        let mut executed = 0usize;
        for code in 0x20u8..=0x7Eu8 {
            let character = char::from(code);
            let root = scratch.path().join(format!("with{character}char"));
            let workspace = WorkspaceRoot::from_resolution(Ok(root.clone()));
            let probe = format!("printf %s {WORKSPACE_ROOT_PLACEHOLDER}");
            match workspace.expand(&probe) {
                Err(_) => {}
                Ok(expanded) => {
                    let expanded =
                        expanded.ok_or("a placeholder-bearing probe must expand or refuse")?;
                    let action = aion_worker::shell::ShellAction::new(&expanded.command).map_err(
                        |error| format!("accepted root {root:?} failed to parse: {error}"),
                    )?;
                    let (context, _cancellation) = aion_worker::ActivityContext::new(
                        aion_core::WorkflowId::new_v4(),
                        aion_core::RunId::new_v4(),
                        aion_core::ActivityId::from_sequence_position(1),
                        1,
                    );
                    let outcome = action
                        .run(&std::collections::BTreeMap::new(), &context)
                        .await
                        .map_err(|error| {
                            format!("accepted root {root:?} failed to execute: {error}")
                        })?;
                    assert_eq!(
                        outcome.stdout, expanded.workspace_root,
                        "the command must observe exactly the accepted root {root:?}"
                    );
                    executed += 1;
                }
            }
        }
        assert!(
            executed > 0,
            "at least one printable root must be accepted, or this test refused everything \
             and proved nothing"
        );
        Ok(())
    }

    #[test]
    fn an_unresolved_root_refuses_a_placeholder_bearing_command_by_name() -> TestResult {
        let command = format!("echo {WORKSPACE_ROOT_PLACEHOLDER}");
        let Err(error) = unresolved().expand(&command) else {
            return Err("an unresolved root must refuse expansion".into());
        };
        assert_eq!(
            error,
            WorkspaceRootError::Unresolvable {
                reason: "AION_HOME must not be empty".to_owned(),
            }
        );
        assert!(
            error.to_string().contains("AION_HOME must not be empty"),
            "the refusal must carry the resolution failure's own reason: {error}"
        );
        Ok(())
    }

    #[test]
    fn a_relative_root_is_refused() -> TestResult {
        let workspace = WorkspaceRoot::from_resolution(Ok(PathBuf::from("relative/clones")));
        let command = format!("echo {WORKSPACE_ROOT_PLACEHOLDER}");
        let Err(error) = workspace.expand(&command) else {
            return Err("a relative root must be refused".into());
        };
        assert_eq!(
            error,
            WorkspaceRootError::NotAbsolute {
                path: "relative/clones".to_owned(),
            }
        );
        Ok(())
    }

    #[test]
    fn a_shape_changing_root_is_refused_naming_the_character() -> TestResult {
        for (fragment, character) in [
            ("with space", "whitespace"),
            ("with\ttab", "whitespace"),
            ("with\nnewline", "whitespace"),
            ("with$dollar", "`$`"),
            ("with'single", "a single quote"),
            ("with\"double", "a double quote"),
            ("with\0nul", "a NUL byte"),
        ] {
            let root = PathBuf::from(format!("/absolute/{fragment}"));
            let workspace = WorkspaceRoot::from_resolution(Ok(root.clone()));
            let command = format!("echo {WORKSPACE_ROOT_PLACEHOLDER}");
            let Err(error) = workspace.expand(&command) else {
                return Err(format!("root {root:?} must be refused as shape-changing").into());
            };
            assert_eq!(
                error,
                WorkspaceRootError::ShapeChanging {
                    path: root.to_string_lossy().into_owned(),
                    character,
                },
                "root {root:?} must be refused naming {character}"
            );
        }
        Ok(())
    }

    #[test]
    fn a_clean_root_has_no_shape_changing_character() {
        assert_eq!(
            shape_changing_character("/Users/operator/.aion/clones"),
            None
        );
    }

    #[test]
    fn the_directory_is_created_when_missing() -> TestResult {
        let scratch = tempfile::tempdir()?;
        let root = scratch.path().join("nested").join("clones");
        assert!(!root.exists(), "the root must start absent");
        let workspace = WorkspaceRoot::from_resolution(Ok(root.clone()));
        let command = format!("echo {WORKSPACE_ROOT_PLACEHOLDER}");
        let first = workspace.expand(&command)?;
        assert!(first.is_some(), "expansion must succeed");
        assert!(root.is_dir(), "expansion must create the missing root");
        // A created root is owner-only: it holds session workspaces, so
        // nothing else on the host gets a default read into it.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(&root)?.permissions().mode() & 0o777;
            assert_eq!(
                mode, 0o700,
                "a created root must be mode 0700, got {mode:o}"
            );
        }
        // Idempotent: a second expansion over the now-existing directory
        // succeeds identically.
        assert_eq!(workspace.expand(&command)?, first);
        Ok(())
    }

    #[test]
    fn a_root_that_cannot_be_created_is_refused_naming_the_io_error() -> TestResult {
        let scratch = tempfile::tempdir()?;
        // A ROOT beneath a regular file cannot be created by any retry.
        let file = scratch.path().join("occupied");
        std::fs::write(&file, b"not a directory")?;
        let root = file.join("clones");
        let workspace = WorkspaceRoot::from_resolution(Ok(root.clone()));
        let command = format!("echo {WORKSPACE_ROOT_PLACEHOLDER}");
        let Err(error) = workspace.expand(&command) else {
            return Err("creation beneath a regular file must fail".into());
        };
        let WorkspaceRootError::CreationFailed { path, error: io } = &error else {
            return Err(format!("expected CreationFailed, got: {error}").into());
        };
        assert_eq!(path, &root.to_string_lossy().into_owned());
        assert!(!io.is_empty(), "the io error's own words must be carried");
        Ok(())
    }

    #[test]
    fn resolved_reports_without_creating() -> TestResult {
        let scratch = tempfile::tempdir()?;
        let root = scratch.path().join("clones");
        let workspace = WorkspaceRoot::from_resolution(Ok(root.clone()));
        assert_eq!(workspace.resolved(), Ok(root.as_path()));
        assert!(
            !root.exists(),
            "reporting the root must not create the directory"
        );
        let failed = unresolved();
        let Err(error) = failed.resolved() else {
            return Err("an unresolved root must report its failure".into());
        };
        assert!(matches!(error, WorkspaceRootError::Unresolvable { .. }));
        Ok(())
    }
}
