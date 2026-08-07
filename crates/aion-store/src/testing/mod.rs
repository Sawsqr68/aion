//! Test doubles over [`crate::memory::InMemoryStore`], for the two things a
//! correct store cannot be asked to do.
//!
//! [`ShardSeamSpy`] answers *"did the decorator forward this seam?"* — it records
//! every default-bodied seam call and returns sentinels distinct from the trait
//! defaults, so a dropped seam is caught by both the return value and the missing
//! call.
//!
//! [`StaleActiveListStore`] answers *"does the reader defend itself when the store
//! lies?"* — it forces chosen ids into `list_active` regardless of their projected
//! status. Every real backend filters that set to exactly `Running`, so a reader's
//! second line of defence behind that filter is unreachable through any honest
//! store and therefore untestable without this one.
//!
//! [`FencedHistoryStore`] answers *"does this caller carry a routable refusal
//! through, or flatten it?"* — its history reads and appends refuse with
//! `StoreError::NotOwner` once armed, and it disclaims every shard while they
//! do. An in-memory store owns every shard, so it can never produce that
//! refusal on its own, and a caller that reads the store directly has no engine
//! error wrapping it to inspect.

pub mod fenced_history;
pub mod shard_seam_spy;
pub mod stale_active_list;

pub use fenced_history::{FencedHistoryStore, REFUSED_SHARD};
pub use shard_seam_spy::ShardSeamSpy;
pub use stale_active_list::StaleActiveListStore;
