//! The `{workspace_root}` placeholder, and the two forms a harness path may take.
//!
//! # Why a document may not simply write the path
//!
//! A `harness` section's path-valued settings — an ACP agent's `command`, either kind's
//! `cwd`, a Norn `binary` — are absolute by rule, because a worker is started from
//! whatever directory a supervisor or a shell happens to be in and a relative path names
//! a different place on every launch.
//!
//! That rule is right for a document an operator writes for THEIR box, and impossible
//! for a document that SHIPS. `crates/aion-server/assistant-embed/assistant.awl` is
//! compiled into the server binary and must be correct on every machine that runs it;
//! there is no absolute literal that is right on more than one. Held to absolute-only,
//! such a document cannot be written at all.
//!
//! So a second accepted form exists, and exactly one: a value that LEADS with the
//! literal [`WORKSPACE_ROOT_PLACEHOLDER`]. The document states the shape of the path and
//! the box supplies its root, which is the same division of labour the server already
//! uses for declared action bodies (`aion_server::worker::WorkspaceRoot`).
//!
//! # Where each end of that division is enforced
//!
//! This module is the ACCEPT-SET only: it says which of the two forms a value is, and
//! refuses everything else with a reason. It deliberately cannot expand anything —
//! resolving the root is a property of the box, which the language layer has no business
//! knowing.
//!
//! The composition root that launches the harness expands the placeholder against the
//! box's own root and THEN demands the result be absolute. Both ends therefore hold:
//! nothing about the absolute requirement is softened, and a placeholder that expands to
//! a non-absolute path — or one on a box whose root cannot be resolved at all — is
//! refused there, loudly, before anything is spawned.
//!
//! # Why the placeholder's spelling lives HERE
//!
//! It is a token of the language documents are written in, and the server's declared-body
//! expansion, the checker, and the compiler's backstop must all agree on it to the byte.
//! One definition in the crate every one of them already depends on is that agreement;
//! a second copy anywhere is the drift.

/// The literal a harness path setting carries where the box's workspace root belongs.
///
/// Braces are literal text to the AWL lexer and to the worker SDK's command template
/// (`$name`/`${name}` are its only parameter forms), so a value carrying this checks
/// clean and — if it ever reached a launch unexpanded — would fail loudly on a
/// nonexistent `{workspace_root}` path rather than silently launching somewhere else.
pub const WORKSPACE_ROOT_PLACEHOLDER: &str = "{workspace_root}";

/// Which of the two accepted forms a harness path setting is written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HarnessPathForm {
    /// An absolute path, usable exactly as written.
    Absolute,
    /// A path leading with [`WORKSPACE_ROOT_PLACEHOLDER`], absolute only once the
    /// composition root has expanded it against the box's own workspace root.
    WorkspaceRooted,
}

/// Why a harness path setting is neither accepted form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HarnessPathRefusal {
    /// The value is empty or nothing but whitespace.
    Empty,
    /// The value is a relative path and carries no placeholder to make it otherwise.
    Relative,
    /// The value carries the placeholder, but not at its start — so however the box
    /// resolves, the expansion cannot produce an absolute path.
    PlaceholderNotLeading,
}

impl HarnessPathRefusal {
    /// The refusal's reason, a self-contained sentence opening with "it".
    ///
    /// One phrasing per refusal, read by both the checker's operator-facing message and
    /// the compiler's backstop, so the two cannot come to describe the same fault
    /// differently. It opens with "it" because both slots introduce the offending setting
    /// first and the reason second.
    #[must_use]
    pub const fn reason(self) -> &'static str {
        match self {
            Self::Empty => {
                "it is empty; a path setting names an absolute path, or leads with \
                 `{workspace_root}` for the launching box to expand"
            }
            Self::Relative => {
                "it must be an ABSOLUTE path. A worker is started from whatever directory a \
                 supervisor or a shell happens to be in, so a relative path names a different \
                 place on every launch. A document that must be correct on more than one box \
                 leads the value with `{workspace_root}` instead, which the launching worker \
                 expands against that box's own workspace root."
            }
            Self::PlaceholderNotLeading => {
                "it carries `{workspace_root}` somewhere other than its start, so expanding it \
                 cannot produce an absolute path. The placeholder stands at the beginning of \
                 the value or not at all."
            }
        }
    }
}

/// Which accepted form `value` is written in, or why it is neither.
///
/// The accept-set is EXACTLY two forms and is stated only here: the checker refuses a
/// document by this rule, and the compiler's backstop refuses a value that reached it
/// by any other route by the same one.
///
/// # Errors
///
/// Returns the [`HarnessPathRefusal`] naming which of the three ways the value failed to
/// be either accepted form.
pub fn harness_path_form(value: &str) -> Result<HarnessPathForm, HarnessPathRefusal> {
    if value.trim().is_empty() {
        return Err(HarnessPathRefusal::Empty);
    }
    if value.starts_with(WORKSPACE_ROOT_PLACEHOLDER) {
        return Ok(HarnessPathForm::WorkspaceRooted);
    }
    if value.contains(WORKSPACE_ROOT_PLACEHOLDER) {
        return Err(HarnessPathRefusal::PlaceholderNotLeading);
    }
    if std::path::Path::new(value).is_absolute() {
        return Ok(HarnessPathForm::Absolute);
    }
    Err(HarnessPathRefusal::Relative)
}

#[cfg(test)]
mod tests {
    use super::{
        HarnessPathForm, HarnessPathRefusal, WORKSPACE_ROOT_PLACEHOLDER, harness_path_form,
    };

    #[test]
    fn an_absolute_path_is_the_first_accepted_form() {
        assert_eq!(
            harness_path_form("/srv/agents/workspace"),
            Ok(HarnessPathForm::Absolute)
        );
    }

    #[test]
    fn a_leading_placeholder_is_the_second_accepted_form() {
        assert_eq!(
            harness_path_form(WORKSPACE_ROOT_PLACEHOLDER),
            Ok(HarnessPathForm::WorkspaceRooted)
        );
        assert_eq!(
            harness_path_form("{workspace_root}/assistant"),
            Ok(HarnessPathForm::WorkspaceRooted)
        );
    }

    #[test]
    fn a_relative_path_is_refused_as_it_always_was() {
        assert_eq!(
            harness_path_form("workspace"),
            Err(HarnessPathRefusal::Relative)
        );
        assert_eq!(
            harness_path_form("./workspace"),
            Err(HarnessPathRefusal::Relative)
        );
        assert_eq!(
            harness_path_form("../workspace"),
            Err(HarnessPathRefusal::Relative)
        );
    }

    #[test]
    fn the_new_form_did_not_widen_the_old_refusal() {
        // The placeholder buys a document ONE shape: the root at the front. A value that
        // merely mentions it is still refused, because no root the box resolves can make
        // `agents/{workspace_root}` absolute — accepting it here would hand the launch a
        // value the composition root must refuse, which is the checker-green/launch-red
        // split the checker exists to prevent.
        assert_eq!(
            harness_path_form("agents/{workspace_root}"),
            Err(HarnessPathRefusal::PlaceholderNotLeading)
        );
        assert_eq!(
            harness_path_form("/srv/{workspace_root}/agents"),
            Err(HarnessPathRefusal::PlaceholderNotLeading),
            "an absolute path carrying the placeholder is still refused: the expansion \
             would splice a second root into the middle of it"
        );
    }

    #[test]
    fn an_empty_value_is_refused_before_either_form_is_considered() {
        assert_eq!(harness_path_form(""), Err(HarnessPathRefusal::Empty));
        assert_eq!(harness_path_form("   "), Err(HarnessPathRefusal::Empty));
    }
}
