//! The sealed conclusion verifier (design §2.6, §6 clause 4).
//!
//! > A sealed verifier reads `.fleet/conclusions/<stage-id>` from the
//! > child's exact recorded pushed commit, never from a branch or worktree,
//! > and returns either a typed `stage_conclusion` result or a typed
//! > refusal.
//!
//! # The seal, and how it is enforced rather than asserted
//!
//! Design §6 names the violations verbatim: *"any implementation that
//! resolves that object with `git rev-parse`, reads a live branch ref or
//! worktree, greps prose for a conclusion, consults time or worker
//! availability, or reads the current deployed module to recover a prior
//! decision violates the design."*
//!
//! This module closes each one by construction:
//!
//! * **Never a name.** [`ExactCommit`] can only be built from exactly 40
//!   lowercase hex characters. A branch name, `HEAD`, `refs/heads/x`, an
//!   abbreviated sha, or an uppercase sha is refused BEFORE any git process
//!   starts. There is no other constructor.
//! * **Never `rev-parse`.** The only git invocations are `ls-tree` against
//!   an exact commit object id and `cat-file blob` against the exact blob
//!   object id that `ls-tree` returned. Neither reads a ref.
//! * **Never a worktree.** `ls-tree` reads the committed tree object. The
//!   working tree is never consulted; `--full-tree` makes the pathspec
//!   root-relative so the process cwd cannot change what is read.
//! * **Never prose.** The token is compared byte-for-byte against the
//!   stage's declared closed vocabulary. There is no search, no trim of the
//!   value, and no pattern.
//! * **Never time or availability.** This module reads no clock and asks
//!   nothing about workers.
//!
//! # The token contract (§2.1)
//!
//! > exactly one declared token, followed by one LF and no other bytes
//!
//! Enforced on the raw bytes: `value` + `0x0A`, and nothing after. No CRLF,
//! no missing LF, no second line, no trailing whitespace, no BOM.

use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::chain::records::{StageConclusion, StageConclusionRefused};
use crate::error::{ConclusionCause, VerifierError};
use crate::ids::{conclusion_path, is_valid_id};

/// An exact git object id: 40 lowercase hex characters, nothing else.
///
/// The type IS the seal. There is no `from_ref`, no `resolve`, and no
/// `Deref<Target = str>` that would let a name in through the side.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExactCommit(String);

impl ExactCommit {
    /// Build from a candidate, refusing anything that is not an exact
    /// object id.
    ///
    /// # Errors
    ///
    /// [`VerifierError::NotAnObjectId`] for a ref name, an abbreviated sha,
    /// an uppercase sha, or any other non-40-hex input.
    pub fn parse(candidate: &str) -> Result<Self, VerifierError> {
        let ok = candidate.len() == 40
            && candidate
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
        if ok {
            Ok(Self(candidate.to_owned()))
        } else {
            Err(VerifierError::NotAnObjectId {
                given: candidate.to_owned(),
            })
        }
    }

    /// The exact object id.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// What the verifier concluded. Both arms are typed records the chain
/// records verbatim (design §4).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "record", rename_all = "snake_case")]
pub enum Verdict {
    /// The token verified against the stage's declared closed vocabulary.
    Verified(StageConclusion),
    /// The token is absent, malformed, or undeclared. This is NEITHER
    /// `unplanned` NOR a silent stage failure (§2.6).
    Refused(StageConclusionRefused),
}

/// Everything one verification needs. Every field is already-recorded
/// history; the verifier resolves nothing.
#[derive(Clone, Debug)]
pub struct VerifyRequest<'a> {
    /// The stage being verified.
    pub stage_id: &'a str,
    /// The child whose result is being verified.
    pub child_workflow_id: &'a str,
    /// The child's EXACT recorded pushed commit.
    pub result_commit: &'a ExactCommit,
    /// The stage's declared closed vocabulary, from the normalized graph.
    pub vocabulary: &'a [String],
    /// The run root kept on a refusal.
    pub kept_root: &'a str,
    /// The last accepted commit, carried on a refusal.
    pub last_accepted_commit: &'a str,
}

/// Read and verify one stage's conclusion token from an exact commit.
///
/// # Errors
///
/// [`VerifierError`] only for seal breaches and git failures — being handed
/// a ref name, an unsafe stage id, or a git that cannot run. An artifact
/// that is merely absent, malformed, or undeclared is NOT an error: it is a
/// [`Verdict::Refused`], because design §2.6 requires those three to be a
/// typed chain outcome with a stable cause code.
pub fn verify_conclusion(
    repo: &Path,
    request: &VerifyRequest<'_>,
) -> Result<Verdict, VerifierError> {
    if !is_valid_id(request.stage_id) {
        return Err(VerifierError::UnsafeStageId {
            given: request.stage_id.to_owned(),
        });
    }
    let path = conclusion_path(request.stage_id);

    let Some(blob) = tree_blob(repo, request.result_commit, &path)? else {
        return Ok(refused(
            request,
            &path,
            ConclusionCause::ConclusionMissing,
            &[],
        ));
    };
    let bytes = cat_blob(repo, &blob)?;

    let Some(token) = single_token_line(&bytes) else {
        return Ok(refused(
            request,
            &path,
            ConclusionCause::ConclusionMalformed,
            &bytes,
        ));
    };
    if !request.vocabulary.iter().any(|v| v == &token) {
        return Ok(refused(
            request,
            &path,
            ConclusionCause::ConclusionUndeclared,
            &bytes,
        ));
    }

    Ok(Verdict::Verified(StageConclusion {
        stage_id: request.stage_id.to_owned(),
        child_workflow_id: request.child_workflow_id.to_owned(),
        result_commit: request.result_commit.as_str().to_owned(),
        conclusion_path: path,
        conclusion: token,
    }))
}

/// The token contract, on raw bytes: exactly one declared value, then
/// exactly one LF, then nothing.
///
/// Returns `None` — malformed — for: no LF; a byte after the LF (a second
/// line, or trailing garbage); an interior LF; an empty value; any ASCII
/// control or space inside the value; a CR (so CRLF is malformed); a
/// non-UTF-8 value.
fn single_token_line(bytes: &[u8]) -> Option<String> {
    let (last, body) = bytes.split_last()?;
    if *last != b'\n' {
        return None;
    }
    if body.is_empty() {
        return None;
    }
    if body
        .iter()
        .any(|b| *b == b'\n' || *b == b'\r' || *b == b' ' || b.is_ascii_control())
    {
        return None;
    }
    String::from_utf8(body.to_vec()).ok()
}

fn refused(
    request: &VerifyRequest<'_>,
    path: &str,
    cause_code: ConclusionCause,
    observed: &[u8],
) -> Verdict {
    Verdict::Refused(StageConclusionRefused {
        stage_id: request.stage_id.to_owned(),
        child_workflow_id: request.child_workflow_id.to_owned(),
        result_commit: request.result_commit.as_str().to_owned(),
        conclusion_path: path.to_owned(),
        cause_code,
        observed_bytes: render_observed(observed),
        kept_root: request.kept_root.to_owned(),
        last_accepted_commit: request.last_accepted_commit.to_owned(),
    })
}

/// Render observed bytes so a refusal can be read without guessing.
///
/// Every byte is shown: printable ASCII verbatim, everything else as
/// `\xNN`, with LF as `\n` and CR as `\r`. A refusal whose evidence is
/// "looks empty" is not evidence.
fn render_observed(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() + 8);
    for b in bytes {
        match b {
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            b'\t' => out.push_str("\\t"),
            b'\\' => out.push_str("\\\\"),
            0x20..=0x7e => out.push(*b as char),
            other => {
                out.push_str("\\x");
                out.push(hex_digit(other >> 4));
                out.push(hex_digit(other & 0x0f));
            }
        }
    }
    out
}

/// One lowercase hex digit for a nibble.
const fn hex_digit(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        _ => (b'a' + nibble - 10) as char,
    }
}

/// `git ls-tree` against an EXACT commit object id, returning the blob id
/// at `path`, or `None` when the path is absent from that tree.
fn tree_blob(
    repo: &Path,
    commit: &ExactCommit,
    path: &str,
) -> Result<Option<String>, VerifierError> {
    let argv = ["ls-tree", "-z", "--full-tree", commit.as_str(), "--", path];
    let out = run_git(repo, &argv)?;
    let text = String::from_utf8_lossy(&out);
    let Some(entry) = text.split('\0').find(|e| !e.is_empty()) else {
        return Ok(None);
    };
    // `<mode> SP <type> SP <object> TAB <path>`
    let Some((meta, _)) = entry.split_once('\t') else {
        return Err(VerifierError::UnparseableTreeEntry {
            line: entry.to_owned(),
        });
    };
    let fields: Vec<&str> = meta.split(' ').collect();
    let [_mode, kind, object] = fields.as_slice() else {
        return Err(VerifierError::UnparseableTreeEntry {
            line: entry.to_owned(),
        });
    };
    if *kind != "blob" {
        // A tree or a submodule at the conclusion path is not a token.
        return Ok(None);
    }
    Ok(Some((*object).to_owned()))
}

/// `git cat-file blob` against an exact blob object id.
fn cat_blob(repo: &Path, blob: &str) -> Result<Vec<u8>, VerifierError> {
    run_git(repo, &["cat-file", "blob", blob])
}

/// Run git, carrying its own stderr on refusal rather than swallowing it.
fn run_git(repo: &Path, argv: &[&str]) -> Result<Vec<u8>, VerifierError> {
    let rendered = argv.join(" ");
    let cwd = repo.display().to_string();
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(argv)
        .output()
        .map_err(|source| VerifierError::GitUnavailable {
            argv: rendered.clone(),
            cwd: cwd.clone(),
            source,
        })?;
    if output.status.success() {
        return Ok(output.stdout);
    }
    Err(VerifierError::GitRefused {
        argv: rendered,
        code: output.status.code().unwrap_or(-1),
        stderr: String::from_utf8_lossy(&output.stderr)
            .trim_end()
            .to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_an_exact_object_id_is_accepted() {
        let forty = "a".repeat(40);
        assert!(ExactCommit::parse(&forty).is_ok());
        // Every one of these is a NAME or an inexact id. Design §6 names
        // resolving one a violation, so the verifier refuses to be handed
        // one at all.
        for name in [
            "HEAD",
            "main",
            "origin/main",
            "refs/heads/main",
            "fleet/result",
            "v0.20.0",
            "HEAD~1",
            "HEAD^{commit}",
            "a1b2c3d",
            "",
            &"A".repeat(40),
            &"a".repeat(39),
            &"a".repeat(41),
            &format!("{}g", "a".repeat(39)),
        ] {
            assert!(
                ExactCommit::parse(name).is_err(),
                "{name:?} was accepted as an exact object id"
            );
        }
    }

    #[test]
    fn the_token_contract_is_bytes_not_prose() {
        assert_eq!(
            single_token_line(b"design-ready\n"),
            Some("design-ready".to_owned())
        );
        assert_eq!(
            single_token_line(b"measured-red\n"),
            Some("measured-red".to_owned())
        );
        for malformed in [
            &b"design-ready"[..],     // no LF
            b"design-ready\n\n",      // two lines
            b"design-ready\nextra\n", // second line
            b"design-ready \n",       // trailing space
            b"design-ready\r\n",      // CRLF
            b" design-ready\n",       // leading space
            b"\n",                    // empty value
            b"",                      // empty file
            b"design-ready\nx",       // trailing garbage after the LF
        ] {
            assert_eq!(
                single_token_line(malformed),
                None,
                "{:?} was accepted as a well-formed token",
                String::from_utf8_lossy(malformed)
            );
        }
    }

    #[test]
    fn observed_bytes_show_every_byte() {
        assert_eq!(render_observed(b"ok\n"), "ok\\n");
        assert_eq!(render_observed(b"\x00"), "\\x00");
        assert_eq!(render_observed(b"a\r\n"), "a\\r\\n");
    }
}
