//! The `fleet-stages-v1` plan-block grammar (design §2.1).

pub mod admit;
pub mod block;
pub mod line;

pub use admit::parse_admit;
pub use block::{BlockScan, SUPPORTED_VERSION, extract_block};
pub use line::{CANONICAL_KEYS, RawStage, parse_stage_line};
