//! Native extraction of structured findings from each leg's own stream.
//!
//! No `jq` on the gate host: every leg's machine-readable output is decoded here
//! with `serde_json` (or, for `cargo fmt`, with its documented check output).

pub mod cargo_stream;
pub mod fmt_stream;
pub mod nextest_stream;
pub mod report;

pub use report::StreamReport;
