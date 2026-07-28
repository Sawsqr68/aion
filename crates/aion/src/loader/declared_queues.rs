//! What one read of the deployed contracts could — and could not — say about
//! declared task queues.
//!
//! A catalog can retain entries whose stored identity carries no decodable
//! contract at all: every package deployed before the `.v4` contract identity
//! reads that way. The handshake migration creates exactly that mixture —
//! old packages retained beside newly deployed `.v4` ones — so a bare set of
//! found queues is not an honest answer. Such a set reads as authoritative
//! while silently omitting whatever the undecodable entries declare, and a
//! caller that treats its absences as facts refuses dispatches nobody
//! contradicted.
//!
//! This type keeps both halves of the answer together, so a reader can tell
//! "no deployed contract declares this queue" apart from "this read could not
//! have seen it".

use std::collections::BTreeSet;

/// The queues one catalog read positively found, with the stored identities
/// that same read could not decode.
///
/// A positive find is a fact regardless of what the read missed. An ABSENCE is
/// only a fact when [`Self::covers_every_entry`] holds.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DeclaredQueues {
    declared: BTreeSet<String>,
    undecodable_identities: Vec<String>,
}

impl DeclaredQueues {
    /// Records one read: the queues found, and the stored identities that could
    /// not be decoded — empty exactly when the read covered every entry.
    #[must_use]
    pub fn new(declared: BTreeSet<String>, undecodable_identities: Vec<String>) -> Self {
        Self {
            declared,
            undecodable_identities,
        }
    }

    /// Whether a decoded contract declares `task_queue`.
    #[must_use]
    pub fn declares(&self, task_queue: &str) -> bool {
        self.declared.contains(task_queue)
    }

    /// Whether every retained entry decoded — the only condition under which
    /// this read's absences carry information.
    #[must_use]
    pub fn covers_every_entry(&self) -> bool {
        self.undecodable_identities.is_empty()
    }

    /// The stored identities this read could not decode, in catalog order.
    #[must_use]
    pub fn undecodable_identities(&self) -> &[String] {
        &self.undecodable_identities
    }

    /// Whether the read found no queue declaration at all. Such a catalog
    /// contradicts nothing, whether or not the read was complete.
    #[must_use]
    pub fn found_no_declaration(&self) -> bool {
        self.declared.is_empty()
    }

    /// Every queue this read found.
    #[must_use]
    pub fn declared(&self) -> &BTreeSet<String> {
        &self.declared
    }
}

#[cfg(test)]
#[path = "declared_queues_tests.rs"]
mod declared_queues_tests;
