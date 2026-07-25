//! Embedded assistant skill-resource materialization.

use std::path::Path;

use aion_worker::ActivityFailure;

/// The assistant's skill documents, embedded so every session gets them even
/// when no Aion repository is present.
pub(super) const SKILL_RESOURCES: [(&str, &str); 5] = [
    (
        "ENVIRONMENT.md",
        include_str!("../../../../assistant/resources/ENVIRONMENT.md"),
    ),
    (
        "AWL-AUTHORING.md",
        include_str!("../../../../assistant/resources/AWL-AUTHORING.md"),
    ),
    (
        "COMMANDS.md",
        include_str!("../../../../assistant/resources/COMMANDS.md"),
    ),
    (
        "AWL-REFERENCE.md",
        include_str!("../../../../assistant/resources/AWL-REFERENCE.md"),
    ),
    (
        "WORKERS.md",
        include_str!("../../../../assistant/resources/WORKERS.md"),
    ),
];

/// Write the embedded skill documents inside the confined workspace.
pub(super) fn materialize_skill_resources(workspace: &Path) -> Result<(), ActivityFailure> {
    let resources = workspace.join(".assistant").join("resources");
    std::fs::create_dir_all(&resources).map_err(|source| {
        ActivityFailure::terminal(format!(
            "cannot create skill-resource directory {}: {source}",
            resources.display()
        ))
    })?;
    for (name, contents) in SKILL_RESOURCES {
        std::fs::write(resources.join(name), contents).map_err(|source| {
            ActivityFailure::terminal(format!(
                "cannot write skill resource {name} into {}: {source}",
                resources.display()
            ))
        })?;
    }
    Ok(())
}
