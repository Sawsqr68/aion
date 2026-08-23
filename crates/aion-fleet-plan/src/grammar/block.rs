//! Extracting exactly one `fleet-stages-v1` block from a plan document.
//!
//! Design §5.1 puts the block in `.fleet/plan.md`, "a line-oriented fenced
//! block", because ordering is a property of the whole chain and the plan is
//! already the committed contract. §5.2 clause 1 requires "exactly one
//! supported block and version".

use crate::cause::CauseCode;
use crate::error::PlanRefusal;

/// The one block version this validator supports.
pub const SUPPORTED_VERSION: &str = "fleet-stages-v1";

/// The fence-info prefix that marks a line as claiming to be a staged-chain
/// block. Any `fleet-stages-*` fence is a CLAIM on this contract; only
/// [`SUPPORTED_VERSION`] is honoured, and any other version is refused by
/// name rather than ignored as prose.
const VERSION_PREFIX: &str = "fleet-stages-v";

/// What a scan of the plan document found.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BlockScan {
    /// No `fleet-stages-*` fence anywhere.
    ///
    /// Design §7 clause 5: a plan WITHOUT a block "continues to run only
    /// through the existing `fleet_dev` entry point". Absence is therefore
    /// NOT a refusal here — it is the legacy path, and the caller routes it
    /// to the pre-existing behaviour byte-identically.
    Absent,
    /// Exactly one supported block, with its stage lines in authored order.
    Present {
        /// The stage lines, comments and blank lines already dropped.
        lines: Vec<String>,
    },
}

/// Scan `plan_text` for the staged-chain block.
///
/// # Errors
///
/// Refuses with [`CauseCode::MultipleBlocks`] when more than one
/// `fleet-stages-*` fence is present, [`CauseCode::UnsupportedBlockVersion`]
/// when the single fence names a version this validator does not support,
/// and [`CauseCode::EmptyBlock`] when a supported block declares no stage.
pub fn extract_block(plan_text: &str) -> Result<BlockScan, PlanRefusal> {
    let mut blocks: Vec<(String, Vec<String>)> = Vec::new();
    let mut open: Option<(String, Vec<String>)> = None;

    for raw in plan_text.lines() {
        let trimmed = raw.trim_end_matches(['\r']);
        let fence = trimmed.trim_start();
        if let Some((version, body)) = open.as_mut() {
            if fence.starts_with("```") {
                blocks.push((std::mem::take(version), std::mem::take(body)));
                open = None;
            } else {
                body.push(trimmed.to_owned());
            }
            continue;
        }
        if let Some(info) = fence.strip_prefix("```") {
            let info = info.trim();
            if info.starts_with(VERSION_PREFIX) {
                open = Some((info.to_owned(), Vec::new()));
            }
        }
    }
    // An unterminated fence still declares a block; its body is what was
    // read. Refusing it as absent would let a truncated plan take the
    // legacy path, which is exactly the silent reinterpretation design §7
    // clause 5 forbids.
    if let Some((version, body)) = open {
        blocks.push((version, body));
    }

    match blocks.len() {
        0 => Ok(BlockScan::Absent),
        1 => {
            let Some((version, body)) = blocks.into_iter().next() else {
                // Unreachable: len() == 1. Expressed without `expect`.
                return Err(PlanRefusal::whole(
                    CauseCode::EmptyBlock,
                    "internal: a single-block scan yielded no block",
                ));
            };
            if version != SUPPORTED_VERSION {
                return Err(PlanRefusal::whole(
                    CauseCode::UnsupportedBlockVersion,
                    format!(
                        "the plan declares a `{version}` block; this validator supports \
                         exactly `{SUPPORTED_VERSION}`"
                    ),
                ));
            }
            let lines: Vec<String> = body
                .into_iter()
                .filter(|l| !l.trim().is_empty())
                .filter(|l| !l.trim_start().starts_with('#'))
                .collect();
            if lines.is_empty() {
                return Err(PlanRefusal::whole(
                    CauseCode::EmptyBlock,
                    "the `fleet-stages-v1` block declares no stage",
                ));
            }
            Ok(BlockScan::Present { lines })
        }
        n => Err(PlanRefusal::whole(
            CauseCode::MultipleBlocks,
            format!(
                "the plan declares {n} `fleet-stages-*` blocks; §5.2 clause 1 requires exactly one"
            ),
        )),
    }
}
