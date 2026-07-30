//! Small snapshot and start-pin bookkeeping helpers for the workflow catalog.

use std::sync::PoisonError;

use aion_package::ContentHash;

use super::{CatalogEntry, CatalogSnapshot, StartPin};

impl Drop for StartPin {
    fn drop(&mut self) {
        let mut pins = self.pins.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(count) = pins.get_mut(&self.key) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                pins.remove(&self.key);
            }
        }
    }
}

impl CatalogSnapshot {
    pub(super) fn routed_entry(&self, workflow_type: &str) -> Option<&CatalogEntry> {
        let version = self.routed.get(workflow_type)?;
        self.by_version
            .get(&(workflow_type.to_owned(), version.clone()))
    }

    /// Every resident version of `workflow_type` that is NOT the route-active
    /// one, sorted. Empty when the type has only the version it is routed at.
    pub(super) fn superseded_versions_of(&self, workflow_type: &str) -> Vec<ContentHash> {
        let routed = self.routed.get(workflow_type);
        let mut versions: Vec<ContentHash> = self
            .by_version
            .keys()
            .filter(|(loaded_type, version)| {
                loaded_type == workflow_type && Some(version) != routed
            })
            .map(|(_, version)| version.clone())
            .collect();
        // `ContentHash` is not `Ord`; sorting by its textual form is the same
        // total order the wire and the CLI already present versions in.
        versions.sort_by_key(ContentHash::to_string);
        versions
    }

    pub(super) fn loaded_versions_of(&self, workflow_type: &str) -> String {
        let mut versions: Vec<String> = self
            .by_version
            .keys()
            .filter(|(loaded_type, _)| loaded_type == workflow_type)
            .map(|(_, version)| version.to_string())
            .collect();
        versions.sort();
        if versions.is_empty() {
            "none".to_owned()
        } else {
            versions.join(", ")
        }
    }
}
