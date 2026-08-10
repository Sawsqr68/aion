use std::ffi::OsStr;
use std::io;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::filesystem::ConfinedDir;

#[derive(Debug, Serialize)]
pub struct DocumentEntry {
    pub path: String,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct DocumentResponse {
    pub source: String,
    pub content_hash: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateDocumentRequest {
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct CreateDocumentResponse {
    pub path: String,
    pub name: String,
    pub source: String,
    pub content_hash: String,
}

#[derive(Debug, Deserialize)]
pub struct PutDocumentRequest {
    pub source: String,
}

#[derive(Debug, thiserror::Error)]
pub enum DocumentError {
    #[error("invalid AWL document path: {0}")]
    InvalidPath(String),
    #[error("invalid AWL document name: {0}")]
    InvalidName(String),
    #[error("AWL document was not found: {0}")]
    NotFound(String),
    #[error("AWL document already exists: {0}")]
    Exists(String),
    #[error("AWL workspace is not configured")]
    WorkspaceUnconfigured,
    #[error("AWL workspace I/O failed: {0}")]
    Io(#[from] io::Error),
    /// A document was written but its first revision could not be stored, and
    /// the half-created document could not be removed again. Both causes are
    /// carried because the operator needs the second one to get unstuck: until
    /// `path` is deleted by hand, creating that document will keep failing as
    /// though it already exists.
    #[error(
        "AWL document `{path}` was created but its first revision could not be stored ({reason}), \
         and removing the half-created document then failed ({rollback}); \
         `{path}` is still in the workspace and must be deleted before it can be created again"
    )]
    CreateRollbackFailed {
        path: String,
        reason: String,
        rollback: io::Error,
    },
}

impl DocumentError {
    /// The stable wire `error_type` label for this failure class.
    ///
    /// One vocabulary in one place: the HTTP studio routes and the MCP
    /// authoring tools both label their refusals through this, so a caller
    /// branching on `error_type` sees the same word whichever surface it used.
    #[must_use]
    pub fn error_type(&self) -> &'static str {
        match self {
            Self::InvalidPath(_) => "InvalidDocumentPath",
            Self::InvalidName(_) => "InvalidDocumentName",
            Self::NotFound(_) => "DocumentNotFound",
            Self::Exists(_) => "DocumentExists",
            Self::WorkspaceUnconfigured => "AuthoringWorkspaceUnconfigured",
            Self::Io(_) => "DocumentIoError",
            Self::CreateRollbackFailed { .. } => "DocumentCreateRollbackFailed",
        }
    }

    /// The wire form of this failure: code class and `error_type` together.
    ///
    /// The class mapping lives HERE, next to the variants, so every surface
    /// that carries a document refusal — the run-loop deploy path and the MCP
    /// tools alike — answers with the same code for the same failure.
    #[must_use]
    pub fn to_wire_error(&self) -> aion_proto::WireError {
        let wire = match self {
            Self::NotFound(_) => aion_proto::WireError::not_found(self.to_string()),
            Self::InvalidPath(_) | Self::InvalidName(_) | Self::Exists(_) => {
                aion_proto::WireError::invalid_input(self.to_string())
            }
            Self::WorkspaceUnconfigured | Self::Io(_) | Self::CreateRollbackFailed { .. } => {
                aion_proto::WireError::backend(self.to_string())
            }
        };
        wire.with_error_type(self.error_type())
    }
}

pub async fn list(root: &Path) -> Result<Vec<DocumentEntry>, DocumentError> {
    let root = root.to_owned();
    blocking("document listing", move || {
        let workspace = match ConfinedDir::open(&root) {
            Ok(workspace) => workspace,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(DocumentError::Io(error)),
        };
        let mut entries: Vec<_> = workspace
            .list_awl()?
            .into_iter()
            .map(|path| DocumentEntry {
                name: path
                    .file_stem()
                    .unwrap_or(OsStr::new(""))
                    .to_string_lossy()
                    .into_owned(),
                path: path.to_string_lossy().replace('\\', "/"),
            })
            .collect();
        entries.sort_by(|left, right| left.path.cmp(&right.path));
        Ok(entries)
    })
    .await
}

pub async fn read(root: &Path, requested: &str) -> Result<DocumentResponse, DocumentError> {
    let relative = document_path(requested)?;
    let root = root.to_owned();
    let requested = requested.to_owned();
    let source = blocking("document read", move || {
        // A workspace root that does not exist yet is the module's normal
        // fresh-server state (nothing creates the configured directory until
        // the first write), and a document in a workspace that does not exist
        // is exactly as not-found as a document missing from one that does.
        // Only ENOENT takes this branch; EACCES/ELOOP/ENOTDIR still surface
        // as the I/O faults they are.
        let workspace = match ConfinedDir::open(&root) {
            Ok(workspace) => workspace,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(DocumentError::NotFound(requested));
            }
            Err(error) => return Err(DocumentError::Io(error)),
        };
        workspace.read_to_string(&relative).map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                DocumentError::NotFound(requested)
            } else {
                confinement_error(error)
            }
        })
    })
    .await?;
    Ok(DocumentResponse {
        content_hash: super::revisions::content_hash(&source),
        source,
    })
}

pub async fn create(
    root: &Path,
    request: CreateDocumentRequest,
) -> Result<CreateDocumentResponse, DocumentError> {
    validate_document_name(&request.name)?;
    let source = new_document_source(&request.name)?;
    let path = format!("{}.awl", request.name);
    let root_owned = root.to_owned();
    let path_owned = PathBuf::from(&path);
    let source_owned = source.clone();
    blocking("document create", move || {
        let workspace = ConfinedDir::open_or_create(&root_owned).map_err(confinement_error)?;
        workspace
            .create_new(&path_owned, source_owned.as_bytes())
            .map_err(|error| {
                if error.kind() == io::ErrorKind::AlreadyExists {
                    DocumentError::Exists(path_owned.to_string_lossy().into_owned())
                } else {
                    confinement_error(error)
                }
            })
    })
    .await?;
    let revision = match super::revisions::store(root, &source).await {
        Ok(revision) => revision,
        Err(error) => {
            // The document file is already on disk but has no first revision,
            // so it must come back off. If the rollback ITSELF fails the
            // operator has to hear about it: the orphan stays in the
            // workspace, and every retry of this create will now fail with
            // `Exists` for a document that was never successfully created.
            // Reporting only the revision error would send them chasing that
            // instead of the file they actually have to delete.
            if let Err(rollback) = rollback_created_document(root, &path).await {
                return Err(DocumentError::CreateRollbackFailed {
                    path: path.clone(),
                    reason: error.to_string(),
                    rollback,
                });
            }
            return Err(revision_io(&error));
        }
    };
    Ok(CreateDocumentResponse {
        path,
        name: request.name,
        source,
        content_hash: revision.content_hash,
    })
}

pub async fn write(
    root: &Path,
    requested: &str,
    request: PutDocumentRequest,
) -> Result<DocumentResponse, DocumentError> {
    let relative = document_path(requested)?;
    let root_owned = root.to_owned();
    let source_owned = request.source.clone();
    blocking("document write", move || {
        let workspace = ConfinedDir::open_or_create(&root_owned).map_err(confinement_error)?;
        workspace
            .atomic_write(&relative, source_owned.as_bytes())
            .map_err(confinement_error)
    })
    .await?;
    let revision = super::revisions::store(root, &request.source)
        .await
        .map_err(|error| revision_io(&error))?;
    Ok(DocumentResponse {
        source: request.source,
        content_hash: revision.content_hash,
    })
}

pub(crate) fn document_path(requested: &str) -> Result<PathBuf, DocumentError> {
    let path = Path::new(requested);
    if requested.is_empty()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(DocumentError::InvalidPath(
            "path must be non-empty, relative, and contain no `..` components".to_owned(),
        ));
    }
    if path.extension() != Some(OsStr::new("awl")) {
        return Err(DocumentError::InvalidPath(
            "document path must end in `.awl`".to_owned(),
        ));
    }
    Ok(path.to_owned())
}

/// Remove a document that was created but never got a first revision.
///
/// Runs on the blocking pool like every other filesystem call in this module —
/// the original rollback ran `ConfinedDir::open` and `remove_file` inline on
/// the async runtime, which blocks a reactor thread on real disk I/O.
///
/// A workspace that has already vanished is treated as a successful rollback:
/// the document it held is gone too, which is the outcome this is for.
async fn rollback_created_document(root: &Path, path: &str) -> Result<(), io::Error> {
    let root_owned = root.to_owned();
    let path_owned = PathBuf::from(path);
    tokio::task::spawn_blocking(move || {
        let workspace = match ConfinedDir::open(&root_owned) {
            Ok(workspace) => workspace,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error),
        };
        match workspace.remove_file(&path_owned) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    })
    .await
    .map_err(|error| io::Error::other(format!("document rollback task failed: {error}")))?
}

fn confinement_error(error: io::Error) -> DocumentError {
    if matches!(
        error.kind(),
        io::ErrorKind::InvalidInput | io::ErrorKind::NotADirectory
    ) {
        DocumentError::InvalidPath(format!(
            "workspace paths must contain only real directories and files: {error}"
        ))
    } else {
        DocumentError::Io(error)
    }
}

async fn blocking<T: Send + 'static>(
    operation: &'static str,
    work: impl FnOnce() -> Result<T, DocumentError> + Send + 'static,
) -> Result<T, DocumentError> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| io::Error::other(format!("{operation} task failed: {error}")))?
}

fn revision_io(error: &super::revisions::RevisionError) -> DocumentError {
    DocumentError::Io(io::Error::other(error.to_string()))
}

fn validate_document_name(name: &str) -> Result<(), DocumentError> {
    let mut characters = name.chars();
    let starts_validly = characters
        .next()
        .is_some_and(|character| character.is_ascii_lowercase() || character == '_');
    if !starts_validly
        || !characters.all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
    {
        return Err(DocumentError::InvalidName(
            "use a lowercase AWL identifier (letters, digits, and underscores)".to_owned(),
        ));
    }
    Ok(())
}

fn new_document_source(name: &str) -> Result<String, DocumentError> {
    let source = format!(
        "//! {name} workflow.\nworkflow {name}\n  outcome done: type Placeholder, route success\n\ntype Placeholder {{ value: String }}\n"
    );
    let document =
        aion_awl::parse(&source).map_err(|error| DocumentError::InvalidName(error.message))?;
    Ok(aion_awl::print(&document))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn missing_workspace_lists_empty_without_materializing_it()
    -> Result<(), Box<dyn std::error::Error>> {
        let parent = crate::test_support::private_tempdir()?;
        let workspace = parent.path().join("aion-authoring");
        assert!(list(&workspace).await?.is_empty());
        assert!(!workspace.exists());
        Ok(())
    }

    /// The read twin of the listing test above: on a fresh server the
    /// configured workspace directory does not exist until the first write,
    /// and a read against that absence is `NotFound` — the same refusal, with
    /// the same guidance downstream, as a document missing from a workspace
    /// that does exist — never a raw ENOENT dressed as an I/O fault. And the
    /// read must not materialize the directory as a side effect.
    #[tokio::test]
    async fn missing_workspace_reads_not_found_without_materializing_it()
    -> Result<(), Box<dyn std::error::Error>> {
        let parent = crate::test_support::private_tempdir()?;
        let workspace = parent.path().join("never-created");
        let refusal = read(&workspace, "any.awl").await;
        assert!(
            matches!(refusal, Err(DocumentError::NotFound(ref path)) if path == "any.awl"),
            "expected NotFound(any.awl), got {refusal:?}"
        );
        assert!(!workspace.exists(), "the read materialized the workspace");
        Ok(())
    }

    #[tokio::test]
    async fn workspace_round_trip_rejects_traversal() -> Result<(), Box<dyn std::error::Error>> {
        let workspace = crate::test_support::private_tempdir()?;
        write(
            workspace.path(),
            "nested/example.awl",
            PutDocumentRequest {
                source: "workflow example\n".to_owned(),
            },
        )
        .await?;
        let entries = list(workspace.path()).await?;
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, "nested/example.awl");
        assert_eq!(
            read(workspace.path(), "nested/example.awl").await?.source,
            "workflow example\n"
        );
        assert!(matches!(
            write(
                workspace.path(),
                "../outside.awl",
                PutDocumentRequest {
                    source: String::new()
                }
            )
            .await,
            Err(DocumentError::InvalidPath(_))
        ));
        assert!(matches!(
            read(workspace.path(), "/tmp/outside.awl").await,
            Err(DocumentError::InvalidPath(_))
        ));
        Ok(())
    }

    /// A create whose first revision cannot be stored must leave NOTHING
    /// behind. The rollback used to be skippable in silence, and the cost of
    /// skipping it is not a stray file — it is that every retry of the same
    /// create then fails with `Exists` for a document the operator never
    /// managed to create, with nothing anywhere saying why.
    #[tokio::test]
    async fn a_create_whose_revision_fails_leaves_no_document_behind()
    -> Result<(), Box<dyn std::error::Error>> {
        let workspace = crate::test_support::private_tempdir()?;
        // `.aion-authoring` is where revisions live. As a regular file it
        // cannot be opened as a directory, so storing the first revision
        // fails after the document itself is already on disk — exactly the
        // window the rollback exists to close.
        std::fs::write(workspace.path().join(".aion-authoring"), b"not a directory")?;

        let failure = create(
            workspace.path(),
            CreateDocumentRequest {
                name: "rollback_probe".to_owned(),
            },
        )
        .await;
        assert!(
            failure.is_err(),
            "a create whose revision cannot be stored must not report success"
        );
        assert!(
            !workspace.path().join("rollback_probe.awl").exists(),
            "the half-created document was left in the workspace"
        );
        assert!(
            list(workspace.path()).await?.is_empty(),
            "the half-created document is still listed"
        );

        // With the revision store repaired, the same name must be creatable —
        // proving the failed attempt did not poison it.
        std::fs::remove_file(workspace.path().join(".aion-authoring"))?;
        let created = create(
            workspace.path(),
            CreateDocumentRequest {
                name: "rollback_probe".to_owned(),
            },
        )
        .await?;
        assert_eq!(created.path, "rollback_probe.awl");
        Ok(())
    }

    /// A rollback that cannot do its job must SAY SO. This is the exact
    /// defect: the old code wrote `if let Ok(workspace) = …` and
    /// `let _ = workspace.remove_file(…)`, so a rollback that failed was
    /// indistinguishable from one that worked, and the operator was told only
    /// about the revision error while the orphan sat in the workspace
    /// poisoning every retry.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_rollback_that_cannot_remove_the_document_reports_it()
    -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::PermissionsExt;

        // Root bypasses directory permissions, so the sabotage below would not
        // sabotage anything and the assertion would pass vacuously. Skip
        // loudly at runtime rather than hiding the test behind `#[ignore]`.
        if rustix::process::geteuid().is_root() {
            tracing::info!(
                "skipping rollback-failure pin: running as root, which bypasses the \
                 directory permissions this test depends on"
            );
            return Ok(());
        }

        let workspace = crate::test_support::private_tempdir()?;
        let document = workspace.path().join("stuck.awl");
        std::fs::write(&document, b"workflow stuck\n")?;

        // Owner read+execute only: the document can still be seen but not
        // unlinked, because unlinking needs write on the DIRECTORY.
        std::fs::set_permissions(workspace.path(), std::fs::Permissions::from_mode(0o500))?;
        let outcome = rollback_created_document(workspace.path(), "stuck.awl").await;
        std::fs::set_permissions(workspace.path(), std::fs::Permissions::from_mode(0o700))?;

        let error = match outcome {
            Ok(()) => {
                return Err(
                    "a rollback that could not remove the document reported success — \
                            this is the swallowed error the fix exists to stop"
                        .into(),
                );
            }
            Err(error) => error,
        };
        assert_eq!(
            error.kind(),
            io::ErrorKind::PermissionDenied,
            "the rollback failure must carry its real cause, got: {error}"
        );
        assert!(document.exists(), "the test did not actually block removal");
        Ok(())
    }

    /// The rollback must not invent a failure when there is nothing to undo.
    #[tokio::test]
    async fn rolling_back_a_vanished_workspace_is_not_a_failure()
    -> Result<(), Box<dyn std::error::Error>> {
        let parent = crate::test_support::private_tempdir()?;
        let absent = parent.path().join("never-created");
        rollback_created_document(&absent, "anything.awl").await?;

        let workspace = crate::test_support::private_tempdir()?;
        rollback_created_document(workspace.path(), "never-written.awl").await?;
        Ok(())
    }

    #[tokio::test]
    async fn create_is_atomic_typed_and_private() -> Result<(), Box<dyn std::error::Error>> {
        let workspace = crate::test_support::private_tempdir()?;
        let created = create(
            workspace.path(),
            CreateDocumentRequest {
                name: "first_workflow".to_owned(),
            },
        )
        .await?;
        assert_eq!(created.path, "first_workflow.awl");
        assert_eq!(
            aion_awl::print(&aion_awl::parse(&created.source)?),
            created.source
        );
        assert!(matches!(
            create(
                workspace.path(),
                CreateDocumentRequest {
                    name: "first_workflow".to_owned()
                }
            )
            .await,
            Err(DocumentError::Exists(_))
        ));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(workspace.path().join(&created.path))?
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn root_parent_and_dangling_temp_links_cannot_escape()
    -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::symlink;

        let sandbox = crate::test_support::private_tempdir()?;
        let outside = sandbox.path().join("outside");
        std::fs::create_dir(&outside)?;
        let root_link = sandbox.path().join("root-link");
        symlink(&outside, &root_link)?;
        assert!(
            write(
                &root_link,
                "escape.awl",
                PutDocumentRequest {
                    source: "escaped".to_owned()
                }
            )
            .await
            .is_err()
        );
        assert!(!outside.join("escape.awl").exists());

        let workspace = sandbox.path().join("workspace");
        std::fs::create_dir(&workspace)?;
        crate::test_support::make_private(&workspace)?;
        symlink(&outside, workspace.join("linked"))?;
        assert!(
            write(
                &workspace,
                "linked/escape.awl",
                PutDocumentRequest {
                    source: "escaped".to_owned()
                }
            )
            .await
            .is_err()
        );
        assert!(!outside.join("escape.awl").exists());

        let victim = outside.join("victim");
        symlink(&victim, workspace.join(".victim.awl.aion-awl.tmp"))?;
        write(
            &workspace,
            "victim.awl",
            PutDocumentRequest {
                source: "safe".to_owned(),
            },
        )
        .await?;
        assert!(
            !victim.exists(),
            "predictable dangling temp link was followed"
        );
        Ok(())
    }
}
