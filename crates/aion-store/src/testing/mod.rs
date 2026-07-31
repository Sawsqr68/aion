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

pub mod shard_seam_spy;
pub mod stale_active_list;

pub use shard_seam_spy::ShardSeamSpy;
pub use stale_active_list::StaleActiveListStore;
