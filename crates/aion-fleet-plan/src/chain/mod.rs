//! The chain's records, its recorded ledger, and its decision function.

pub mod apply;
pub mod decide;
pub mod history;
pub mod records;
pub mod report;

pub use apply::{External, apply};
pub use decide::{ChainAction, ChainOutcome, next_action};
pub use history::{CarryForward, ChainLedger, Recorded};
pub use records::{
    AcceptanceDecision, HoldDecision, SkipRecord, StageConclusion, StageConclusionRefused,
    StageFailed, StageReceipt,
};
pub use report::{ChainReport, SkipLine, StageLine};
