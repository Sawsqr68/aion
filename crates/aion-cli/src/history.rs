//! Shared paged history reads for CLI commands that need the complete oplog.

use aion_client::Client;
use aion_core::{Event, WorkflowId};
use anyhow::{Context, Result};

/// Reads every history page in sequence without bounding the total answer.
pub(crate) async fn read_all(client: &Client, workflow_id: &WorkflowId) -> Result<Vec<Event>> {
    let mut events = Vec::new();
    let mut from_seq = Some(1);

    loop {
        let page = client
            .read_history(workflow_id, from_seq, None)
            .await
            .context("failed to read workflow history")?;
        events.extend(page.events);
        let Some(next_from_seq) = page.next_from_seq else {
            return Ok(events);
        };
        from_seq = Some(next_from_seq);
    }
}
