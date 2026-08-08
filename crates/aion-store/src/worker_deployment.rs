//! Durable worker-deployment records and persistence contract.

use std::collections::BTreeSet;

pub use aion_core::{DesiredState, PutOutcome};
use async_trait::async_trait;
use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};

use crate::StoreError;

/// The executable artifact named by a worker deployment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum WorkerArtifactRef {
    /// The server's own executable, invoked with this operator-declared argv tail.
    Builtin {
        /// Arguments passed after the server executable path.
        verb: Vec<String>,
    },
}

/// Content identity of the server binary named by a builtin deployment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeployedBinaryIdentity {
    /// Package version stamped into the binary.
    pub version: String,
    /// Source commit stamped into the binary.
    pub commit: String,
    /// Dirty-state token stamped into the binary.
    pub dirty: String,
    /// Lowercase hexadecimal SHA-256 digest of the executable bytes.
    pub content_hash: String,
}

/// One append-only deployment status transition.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusEntry {
    /// When the transition was recorded.
    pub at: DateTime<Utc>,
    /// Stable status token.
    pub status: String,
    /// Optional human-readable context.
    pub detail: Option<String>,
}

/// A durable operator-authored worker deployment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerDeployment {
    /// Operator-chosen primary key.
    pub name: String,
    /// Artifact the deployment names.
    pub artifact: WorkerArtifactRef,
    /// Identity captured from the named binary at deploy time.
    pub binary: DeployedBinaryIdentity,
    /// Identity used by the most recent spawn. Always `None` in W-0.
    pub last_spawn_binary: Option<DeployedBinaryIdentity>,
    /// Namespace routing set.
    pub namespaces: BTreeSet<String>,
    /// Task-queue routing axis.
    pub task_queue: String,
    /// Optional node/locality routing axis.
    pub node: Option<String>,
    /// Operator-requested state.
    pub desired: DesiredState,
    /// Append-only status transitions.
    pub status_history: Vec<StatusEntry>,
    /// Record creation instant.
    pub created_at: DateTime<Utc>,
    /// Most recent mutation instant.
    pub updated_at: DateTime<Utc>,
}

/// Validation failure for a worker deployment boundary.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum WorkerDeploymentValidationError {
    /// Deployment names are primary keys and may not be empty.
    #[error("worker deployment name must not be empty")]
    EmptyName,
}

/// Operator and deploy-time fields used to create a deployment record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewWorkerDeployment {
    /// Operator-chosen primary key.
    pub name: String,
    /// Artifact the deployment names.
    pub artifact: WorkerArtifactRef,
    /// Identity captured from the named binary at deploy time.
    pub binary: DeployedBinaryIdentity,
    /// Namespace routing set.
    pub namespaces: BTreeSet<String>,
    /// Task-queue routing axis.
    pub task_queue: String,
    /// Optional node/locality routing axis.
    pub node: Option<String>,
    /// Initial operator-requested state.
    pub desired: DesiredState,
}

impl WorkerDeployment {
    /// Stable token written when a record is created.
    pub const CREATED_STATUS: &'static str = "created";
    /// Stable token written when an existing record is replaced.
    pub const REPLACED_STATUS: &'static str = "replaced";
    /// Stable token written when desired state is set.
    pub const DESIRED_STATE_CHANGED_STATUS: &'static str = "desired-state-changed";

    /// Construct a new deployment and its initial `created` history entry.
    ///
    /// # Errors
    ///
    /// Returns [`WorkerDeploymentValidationError::EmptyName`] for an empty or
    /// whitespace-only primary key.
    pub fn new(
        input: NewWorkerDeployment,
        now: DateTime<Utc>,
    ) -> Result<Self, WorkerDeploymentValidationError> {
        validate_name(&input.name)?;
        Ok(Self {
            name: input.name,
            artifact: input.artifact,
            binary: input.binary,
            last_spawn_binary: None,
            namespaces: input.namespaces,
            task_queue: input.task_queue,
            node: input.node,
            desired: input.desired,
            status_history: vec![StatusEntry {
                at: now,
                status: Self::CREATED_STATUS.to_owned(),
                detail: None,
            }],
            created_at: now,
            updated_at: now,
        })
    }

    /// Merge the durable memory of `previous` into this replacement record.
    ///
    /// Operator-authored fields and binary identity come from `self`; creation
    /// time, append-only history, and the last-spawn binary survive from the
    /// previous record. The replacement itself is appended as a stable history
    /// entry at `now` and becomes the record's new update instant.
    #[must_use]
    pub fn preserving_previous(mut self, previous: &Self, now: DateTime<Utc>) -> Self {
        self.created_at = previous.created_at;
        self.status_history.clone_from(&previous.status_history);
        self.status_history.push(StatusEntry {
            at: now,
            status: Self::REPLACED_STATUS.to_owned(),
            detail: None,
        });
        self.last_spawn_binary
            .clone_from(&previous.last_spawn_binary);
        self.updated_at = now;
        self
    }

    /// Set the desired state and append the corresponding status entry.
    pub fn change_desired_state(&mut self, desired: DesiredState, now: DateTime<Utc>) {
        self.desired = desired;
        self.updated_at = now;
        self.status_history.push(StatusEntry {
            at: now,
            status: Self::DESIRED_STATE_CHANGED_STATUS.to_owned(),
            detail: Some(desired.token().to_owned()),
        });
    }

    /// Encode the stable backend-neutral on-disk representation.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Serialization`] when serialization fails.
    pub fn encode(&self) -> Result<Vec<u8>, StoreError> {
        let stored = StoredWorkerDeployment::from(self);
        serde_json::to_vec(&stored).map_err(|error| StoreError::Serialization(error.to_string()))
    }

    /// Decode and validate a stable on-disk representation.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Serialization`] for malformed records, unknown
    /// artifact tags, invalid instants, or an empty deployment name.
    pub fn decode(bytes: &[u8]) -> Result<Self, StoreError> {
        let stored: StoredWorkerDeployment = serde_json::from_slice(bytes)
            .map_err(|error| StoreError::Serialization(error.to_string()))?;
        let record = Self {
            name: stored.name,
            artifact: stored.artifact.into(),
            binary: stored.binary,
            last_spawn_binary: stored.last_spawn_binary,
            namespaces: stored.namespaces,
            task_queue: stored.task_queue,
            node: stored.node,
            desired: stored.desired,
            status_history: stored
                .status_history
                .into_iter()
                .map(StoredStatusEntry::decode)
                .collect::<Result<Vec<_>, _>>()?,
            created_at: decode_instant(&stored.created_at)?,
            updated_at: decode_instant(&stored.updated_at)?,
        };
        validate_name(&record.name)
            .map_err(|error| StoreError::Serialization(error.to_string()))?;
        Ok(record)
    }
}

/// A deployment row that was present but could not be decoded.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UndecodableWorkerDeployment {
    /// Primary-key name under which the poisoned row is stored.
    pub name: String,
    /// Decode failure rendered for operator diagnosis.
    pub error: String,
}

/// Complete worker-deployment listing, including poisoned-row visibility.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerDeploymentListing {
    /// Successfully decoded records, ordered by primary-key name.
    pub deployments: Vec<WorkerDeployment>,
    /// Present rows that could not be decoded, ordered by primary-key name.
    pub undecodable: Vec<UndecodableWorkerDeployment>,
}

/// Exact result of a create-or-replace operation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerDeploymentPutResult {
    /// Whether the persisted row was newly created or replaced an existing key.
    pub outcome: PutOutcome,
    /// The exact merged record persisted by the backend.
    pub deployment: WorkerDeployment,
}

/// Result of deleting a worker deployment by key.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerDeploymentDeleteOutcome {
    /// Whether any row existed and was removed.
    pub existed: bool,
    /// The removed record when it decoded successfully.
    pub deployment: Option<WorkerDeployment>,
}

/// Durable worker-deployment persistence contract.
#[async_trait]
pub trait WorkerDeploymentStore: Send + Sync + 'static {
    /// Create or replace a deployment record, returning the exact persisted value.
    async fn put_worker_deployment(
        &self,
        record: WorkerDeployment,
    ) -> Result<WorkerDeploymentPutResult, StoreError>;

    /// Look up one deployment by name.
    async fn get_worker_deployment(
        &self,
        name: &str,
    ) -> Result<Option<WorkerDeployment>, StoreError>;

    /// List decodable deployments and report every undecodable key, with both
    /// sets ordered by primary-key name.
    async fn list_worker_deployments(&self) -> Result<WorkerDeploymentListing, StoreError>;

    /// Set desired state on an existing record, returning `None` when absent.
    async fn set_desired_state(
        &self,
        name: &str,
        desired: DesiredState,
    ) -> Result<Option<WorkerDeployment>, StoreError>;

    /// Atomically remove a row by key without requiring it to decode.
    ///
    /// `existed` distinguishes absence from a poisoned row that was removed;
    /// `deployment` contains the removed value only when it decoded.
    async fn delete_worker_deployment(
        &self,
        name: &str,
    ) -> Result<WorkerDeploymentDeleteOutcome, StoreError>;
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredWorkerDeployment {
    name: String,
    artifact: StoredWorkerArtifactRef,
    binary: DeployedBinaryIdentity,
    last_spawn_binary: Option<DeployedBinaryIdentity>,
    namespaces: BTreeSet<String>,
    task_queue: String,
    node: Option<String>,
    desired: DesiredState,
    status_history: Vec<StoredStatusEntry>,
    created_at: String,
    updated_at: String,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
enum StoredWorkerArtifactRef {
    Builtin { verb: Vec<String> },
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredStatusEntry {
    at: String,
    status: String,
    detail: Option<String>,
}

impl From<&WorkerDeployment> for StoredWorkerDeployment {
    fn from(record: &WorkerDeployment) -> Self {
        Self {
            name: record.name.clone(),
            artifact: (&record.artifact).into(),
            binary: record.binary.clone(),
            last_spawn_binary: record.last_spawn_binary.clone(),
            namespaces: record.namespaces.clone(),
            task_queue: record.task_queue.clone(),
            node: record.node.clone(),
            desired: record.desired,
            status_history: record
                .status_history
                .iter()
                .map(StoredStatusEntry::from)
                .collect(),
            created_at: encode_instant(record.created_at),
            updated_at: encode_instant(record.updated_at),
        }
    }
}

impl From<&WorkerArtifactRef> for StoredWorkerArtifactRef {
    fn from(artifact: &WorkerArtifactRef) -> Self {
        match artifact {
            WorkerArtifactRef::Builtin { verb } => Self::Builtin { verb: verb.clone() },
        }
    }
}

impl From<StoredWorkerArtifactRef> for WorkerArtifactRef {
    fn from(artifact: StoredWorkerArtifactRef) -> Self {
        match artifact {
            StoredWorkerArtifactRef::Builtin { verb } => Self::Builtin { verb },
        }
    }
}

impl From<&StatusEntry> for StoredStatusEntry {
    fn from(entry: &StatusEntry) -> Self {
        Self {
            at: encode_instant(entry.at),
            status: entry.status.clone(),
            detail: entry.detail.clone(),
        }
    }
}

impl StoredStatusEntry {
    fn decode(self) -> Result<StatusEntry, StoreError> {
        Ok(StatusEntry {
            at: decode_instant(&self.at)?,
            status: self.status,
            detail: self.detail,
        })
    }
}

fn validate_name(name: &str) -> Result<(), WorkerDeploymentValidationError> {
    if name.trim().is_empty() {
        Err(WorkerDeploymentValidationError::EmptyName)
    } else {
        Ok(())
    }
}

fn encode_instant(instant: DateTime<Utc>) -> String {
    instant.to_rfc3339_opts(SecondsFormat::Nanos, true)
}

fn decode_instant(value: &str) -> Result<DateTime<Utc>, StoreError> {
    DateTime::parse_from_rfc3339(value)
        .map(|date_time| date_time.with_timezone(&Utc))
        .map_err(|error| StoreError::Serialization(error.to_string()))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use chrono::{TimeZone, Utc};

    use super::{
        DeployedBinaryIdentity, DesiredState, NewWorkerDeployment, WorkerArtifactRef,
        WorkerDeployment, WorkerDeploymentValidationError,
    };

    fn instant() -> Result<chrono::DateTime<Utc>, &'static str> {
        Utc.with_ymd_and_hms(2026, 8, 8, 1, 2, 3)
            .single()
            .ok_or("test instant must be valid")
    }

    fn record() -> Result<WorkerDeployment, Box<dyn std::error::Error>> {
        Ok(WorkerDeployment::new(
            NewWorkerDeployment {
                name: "shells".to_owned(),
                artifact: WorkerArtifactRef::Builtin {
                    verb: vec!["worker".to_owned(), "shell".to_owned()],
                },
                binary: DeployedBinaryIdentity {
                    version: "1.2.3".to_owned(),
                    commit: "abc".to_owned(),
                    dirty: "false".to_owned(),
                    content_hash: "0123".to_owned(),
                },
                namespaces: BTreeSet::from(["orders".to_owned()]),
                task_queue: "shell".to_owned(),
                node: Some("node-a".to_owned()),
                desired: DesiredState::Running,
            },
            instant()?,
        )?)
    }

    #[test]
    fn codec_round_trips_every_field_and_spawn_slot() -> Result<(), Box<dyn std::error::Error>> {
        let mut expected = record()?;
        expected.last_spawn_binary = Some(DeployedBinaryIdentity {
            version: "0.9.0".to_owned(),
            commit: "old".to_owned(),
            dirty: "true".to_owned(),
            content_hash: "feed".to_owned(),
        });
        expected.change_desired_state(DesiredState::Stopped, instant()?);
        assert_eq!(WorkerDeployment::decode(&expected.encode()?)?, expected);
        Ok(())
    }

    #[test]
    fn unknown_artifact_tag_is_a_typed_refusal() -> Result<(), Box<dyn std::error::Error>> {
        let encoded = record()?.encode()?;
        let mut value: serde_json::Value = serde_json::from_slice(&encoded)?;
        value["artifact"]["type"] = serde_json::Value::String("archive".to_owned());
        let error = WorkerDeployment::decode(&serde_json::to_vec(&value)?).err();
        assert!(matches!(error, Some(crate::StoreError::Serialization(_))));
        Ok(())
    }

    #[test]
    fn empty_name_is_refused() -> Result<(), Box<dyn std::error::Error>> {
        let mut invalid = record()?;
        invalid.name = "  ".to_owned();
        assert!(matches!(
            WorkerDeployment::new(
                NewWorkerDeployment {
                    name: invalid.name,
                    artifact: invalid.artifact,
                    binary: invalid.binary,
                    namespaces: invalid.namespaces,
                    task_queue: invalid.task_queue,
                    node: invalid.node,
                    desired: invalid.desired,
                },
                invalid.created_at,
            ),
            Err(WorkerDeploymentValidationError::EmptyName)
        ));
        Ok(())
    }
}
