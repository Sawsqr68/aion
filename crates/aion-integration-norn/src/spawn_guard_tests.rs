use aion_core::{ActivityId, ContentType, Payload, RunId, WorkflowId};
use tempfile::tempdir;

use super::*;

fn spec() -> AgentRunSpec {
    AgentRunSpec::new(
        WorkflowId::new_v4(),
        RunId::new_v4(),
        ActivityId::from_sequence_position(1),
        1,
        "dev",
        Payload::new(ContentType::Json, b"run".to_vec()),
    )
}

fn marker_path(tree: &Path, state: &Path) -> Result<PathBuf, HarnessError> {
    Ok(claim_area::for_tree_under(tree, state)?
        .directory
        .join(MARKER_NAME))
}

fn install_marker(
    tree: &Path,
    state: &Path,
    marker_tree: &str,
    holder: &AgentRunSpec,
    pid: u32,
    started: &str,
    command: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let path = marker_path(tree, state)?;
    std::fs::create_dir_all(path.parent().ok_or("marker parent absent")?)?;
    let value = json!({
        "tree": marker_tree,
        "workflow_id": holder.workflow_id.to_string(),
        "activity_id": holder.activity_id.to_string(),
        "pid": pid,
        "process_started": started,
        "process_command": command,
    });
    std::fs::write(&path, serde_json::to_vec(&value)?)?;
    Ok(path)
}

#[test]
fn live_exclusion_drop_release_and_per_tree_scope_are_preserved()
-> Result<(), Box<dyn std::error::Error>> {
    // Fails if relocation changes what is excluded, makes the guard global, or
    // leaves an armed pre-spawn reservation behind on drop.
    let scratch = tempdir()?;
    let state = scratch.path().join("state");
    let tree_one = scratch.path().join("one");
    let tree_two = scratch.path().join("two");
    std::fs::create_dir_all(&tree_one)?;
    std::fs::create_dir_all(&tree_two)?;
    let first_spec = spec();
    let first = SpawnClaim::acquire_under(&tree_one, &state, &first_spec)?;
    let path = marker_path(&tree_one, &state)?;
    assert!(path.exists());
    assert!(!tree_one.join(MARKER_NAME).exists());

    let refused = SpawnClaim::acquire_under(&tree_one, &state, &spec());
    assert!(refused.is_err());
    let other_tree = SpawnClaim::acquire_under(&tree_two, &state, &spec())?;
    assert!(marker_path(&tree_two, &state)?.exists());
    drop(other_tree);

    drop(first);
    assert!(!path.exists());
    let replacement = SpawnClaim::acquire_under(&tree_one, &state, &spec())?;
    drop(replacement);
    Ok(())
}

#[test]
fn same_execution_reattaches_without_deleting_the_live_marker()
-> Result<(), Box<dyn std::error::Error>> {
    // Fails if the existing same-execution reattach arm is turned into refusal
    // or cleanup during the location-only refactor.
    let scratch = tempdir()?;
    let tree = scratch.path().join("tree");
    let state = scratch.path().join("state");
    std::fs::create_dir_all(&tree)?;
    let run = spec();
    let first = SpawnClaim::acquire_under(&tree, &state, &run)?;
    let path = marker_path(&tree, &state)?;
    let reattached = SpawnClaim::acquire_under(&tree, &state, &run)?;
    drop(reattached);
    assert!(path.exists());
    drop(first);
    assert!(!path.exists());
    Ok(())
}

#[test]
fn dead_and_pid_reused_incarnations_are_cleaned_at_the_new_location()
-> Result<(), Box<dyn std::error::Error>> {
    // Fails if stale cleanup is lost, or if liveness is weakened to PID/signal-0
    // and a different live process incarnation wedges the tree forever.
    let scratch = tempdir()?;
    let tree = scratch.path().join("tree");
    let state = scratch.path().join("state");
    std::fs::create_dir_all(&tree)?;
    let canonical = std::fs::canonicalize(&tree)?;
    let tree_text = canonical.to_str().ok_or("tree not UTF-8")?;

    let dead_path = install_marker(&tree, &state, tree_text, &spec(), u32::MAX, "gone", "gone")?;
    let after_dead = SpawnClaim::acquire_under(&tree, &state, &spec())?;
    assert!(dead_path.exists());
    drop(after_dead);

    let pid = std::process::id();
    let identity = process_identity(pid)?;
    let reused_path = install_marker(
        &tree,
        &state,
        tree_text,
        &spec(),
        pid,
        &identity.started,
        "different process incarnation",
    )?;
    let after_reuse = SpawnClaim::acquire_under(&tree, &state, &spec())?;
    assert!(
        signal_zero(pid),
        "the unrelated live process must remain alive"
    );
    let replacement = read_marker(&reused_path)?;
    assert_eq!(replacement.process.command, identity.command);
    drop(after_reuse);
    Ok(())
}

#[test]
fn matching_live_incarnation_refuses_and_tree_mismatch_is_loud_and_preserved()
-> Result<(), Box<dyn std::error::Error>> {
    // Fails open if a matching live holder is cleaned; fails silently if a key
    // collision/wrong-tree marker is trusted or deleted.
    let scratch = tempdir()?;
    let tree = scratch.path().join("tree");
    let state = scratch.path().join("state");
    std::fs::create_dir_all(&tree)?;
    let canonical = std::fs::canonicalize(&tree)?;
    let tree_text = canonical.to_str().ok_or("tree not UTF-8")?;
    let pid = std::process::id();
    let identity = process_identity(pid)?;
    let live_path = install_marker(
        &tree,
        &state,
        tree_text,
        &spec(),
        pid,
        &identity.started,
        &identity.command,
    )?;
    assert!(SpawnClaim::acquire_under(&tree, &state, &spec()).is_err());
    assert!(live_path.exists());

    std::fs::remove_file(&live_path)?;
    let mismatch = install_marker(
        &tree,
        &state,
        "/different/canonical/tree",
        &spec(),
        u32::MAX,
        "gone",
        "gone",
    )?;
    assert!(SpawnClaim::acquire_under(&tree, &state, &spec()).is_err());
    assert!(mismatch.exists());
    Ok(())
}
