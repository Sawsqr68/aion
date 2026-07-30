//! Durable worker-contract records and their canonical identity encoding.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::Manifest;

/// Refusal returned when a stored package predates contract-bound identity.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ContractIdentityError {
    /// The package is integrity-valid but its identity commits to no contract.
    #[error(
        "package identity `{stored_version}` predates `.v4` worker-contract commitment; re-deploy this package under `.v4`"
    )]
    RedeployRequired {
        /// Stored pre-`.v4` package identity.
        stored_version: String,
    },
}

/// The complete contract surface committed into a package's `.v4` identity.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageContract {
    /// Primary workflow input schema.
    #[serde(serialize_with = "crate::canonical::serialize_value")]
    pub input_schema: Value,
    /// Primary workflow output schema.
    #[serde(serialize_with = "crate::canonical::serialize_value")]
    pub output_schema: Value,
    /// Queue-scoped activity declarations.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub workers: Vec<WorkerContract>,
    /// Declared child workflow callables.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<ChildContract>,
    /// Declared signal payloads.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub signals: Vec<SignalContract>,
    /// Additional workflow entry schemas in this package.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub additional_workflows: Vec<AdditionalWorkflowContract>,
    /// Activity names retained by legacy project manifests that cannot declare a
    /// queue or typed activity surface. They remain identity-bound but do not
    /// create a queue declaration.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unscoped_activities: Vec<String>,
}

impl PackageContract {
    /// Produces the most precise contract available from a legacy manifest.
    ///
    /// This never invents a queue or schemas for bare activity names. Such names
    /// are committed as unscoped records and therefore cannot satisfy a
    /// structural queue-service check.
    #[must_use]
    pub fn from_manifest(manifest: &Manifest) -> Self {
        Self {
            input_schema: manifest.input_schema.clone(),
            output_schema: manifest.output_schema.clone(),
            workers: Vec::new(),
            children: Vec::new(),
            signals: Vec::new(),
            additional_workflows: manifest
                .additional_workflows
                .iter()
                .map(|entry| AdditionalWorkflowContract {
                    workflow_type: entry.workflow_type.clone(),
                    input_schema: entry.input_schema.clone(),
                    output_schema: entry.output_schema.clone(),
                })
                .collect(),
            unscoped_activities: manifest
                .activities
                .iter()
                .map(|activity| activity.activity_type.clone())
                .collect(),
        }
    }

    /// Returns the deterministic binary encoding committed by the `.v4` hash.
    ///
    /// Declaration vectors and JSON object keys are sorted before encoding.
    /// JSON whitespace and source map insertion order therefore cannot affect
    /// package identity.
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        encode_json(&mut bytes, None, &self.input_schema);
        encode_json(&mut bytes, None, &self.output_schema);

        let mut workers = self
            .workers
            .iter()
            .map(WorkerContract::canonical_bytes)
            .collect::<Vec<_>>();
        workers.sort();
        encode_len(&mut bytes, workers.len());
        for worker in workers {
            update_record(&mut bytes, &worker);
        }

        let mut children = self
            .children
            .iter()
            .map(ChildContract::canonical_bytes)
            .collect::<Vec<_>>();
        children.sort();
        encode_len(&mut bytes, children.len());
        for child in children {
            update_record(&mut bytes, &child);
        }

        let mut signals = self.signals.iter().collect::<Vec<_>>();
        signals.sort_by(|left, right| left.name.cmp(&right.name));
        encode_len(&mut bytes, signals.len());
        for signal in signals {
            encode_text(&mut bytes, &signal.name);
            encode_json(&mut bytes, None, &signal.input_schema);
        }

        let mut additional = self.additional_workflows.iter().collect::<Vec<_>>();
        additional.sort_by(|left, right| left.workflow_type.cmp(&right.workflow_type));
        encode_len(&mut bytes, additional.len());
        for workflow in additional {
            encode_text(&mut bytes, &workflow.workflow_type);
            encode_json(&mut bytes, None, &workflow.input_schema);
            encode_json(&mut bytes, None, &workflow.output_schema);
        }

        let mut unscoped = self.unscoped_activities.iter().collect::<Vec<_>>();
        unscoped.sort();
        encode_len(&mut bytes, unscoped.len());
        for activity in unscoped {
            encode_text(&mut bytes, activity);
        }
        bytes
    }
}

/// The actions a worker queue must serve.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerContract {
    /// Declared task queue.
    pub task_queue: String,
    /// Typed activities on the queue.
    pub actions: Vec<ActionContract>,
}

impl WorkerContract {
    fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        encode_text(&mut bytes, &self.task_queue);
        let mut actions = self
            .actions
            .iter()
            .map(ActionContract::canonical_bytes)
            .collect::<Vec<_>>();
        actions.sort();
        encode_len(&mut bytes, actions.len());
        for action in actions {
            update_record(&mut bytes, &action);
        }
        bytes
    }
}

/// Typed activity surface advertised by a concrete worker build.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivityDescriptor {
    /// Activity type.
    pub name: String,
    /// Schema accepted by the worker.
    #[serde(serialize_with = "crate::canonical::serialize_value")]
    pub input_schema: Value,
    /// Schema produced by the worker.
    #[serde(serialize_with = "crate::canonical::serialize_value")]
    pub output_schema: Value,
}

/// One typed activity declaration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionContract {
    /// Activity type.
    pub name: String,
    /// Object schema over the activity parameters.
    #[serde(serialize_with = "crate::canonical::serialize_value")]
    pub input_schema: Value,
    /// Activity result schema.
    #[serde(serialize_with = "crate::canonical::serialize_value")]
    pub output_schema: Value,
    /// Declared node selector.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node: Option<String>,
    /// Declared schedule-to-close timeout.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<Duration>,
    /// Declaration-owned retry envelope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry: Option<RetryContract>,
    /// Whether the declaration classes this activity as ADVISORY: a side
    /// channel whose failure warns on the run and never faults the calling
    /// step (RUNTIME-OPERATIONS.md R5). Identity-bound — flipping it changes
    /// what the package promises a caller.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub advisory: bool,
    /// The declarative body the action carries, when it declares one.
    ///
    /// `None` is a requirement on an out-of-band worker: the action's name
    /// and schemas are the whole promise, and something must connect to
    /// serve it. `Some` means the package itself says what the action DOES,
    /// so the host can execute it with no worker connected.
    ///
    /// Identity-bound deliberately: the body is executable authority, and an
    /// authority that did not participate in the package hash could be
    /// rewritten in storage without changing what the deployment claims to
    /// be. A body edit is a new package, always.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<ActionBodyContract>,
}

/// A declarative action body committed into package identity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ActionBodyContract {
    /// One command line, executed directly with argv-element parameter
    /// substitution — never through a shell. The text is the authored form
    /// with `$` interpolation intact; the executor parses it and substitutes
    /// each referenced parameter as one whole argument.
    Run {
        /// The authored command text.
        command: String,
    },
}

impl ActionContract {
    fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        encode_text(&mut bytes, &self.name);
        encode_json(&mut bytes, None, &self.input_schema);
        encode_json(&mut bytes, None, &self.output_schema);
        encode_optional_text(&mut bytes, self.node.as_deref());
        encode_optional_duration(&mut bytes, self.timeout);
        match &self.retry {
            None => bytes.push(0),
            Some(RetryContract::Every { count, every }) => {
                bytes.push(1);
                bytes.extend_from_slice(&count.to_be_bytes());
                encode_duration(&mut bytes, *every);
            }
            Some(RetryContract::Backoff { count, min, max }) => {
                bytes.push(2);
                bytes.extend_from_slice(&count.to_be_bytes());
                encode_duration(&mut bytes, *min);
                encode_duration(&mut bytes, *max);
            }
        }
        // ADVISORY is encoded ONLY when true: a single marker byte appended
        // after the retry block, and nothing at all when false. It is
        // injective because absence and the marker cannot be confused at the
        // end of a positional record — the block that FOLLOWS it always
        // begins with a body discriminant (`0`/`1`), never `ADVISORY_MARKER`.
        if self.advisory {
            bytes.push(ADVISORY_MARKER);
        }
        // The BODY block always encodes — a discriminant byte, then the
        // command text for `Run`. Adding it consumed the record's optional
        // tail (the advisory marker was the one tail-append the previous
        // domain could injectively absorb), which is why this encoding lives
        // under the bumped `.v5` identity domain rather than as a second
        // conditional suffix: two optional tails are not injective, and a
        // contract identity that two different declarations can share is a
        // spoofable deployment.
        //
        // LAW for the next field: encode it UNCONDITIONALLY after this
        // block and bump the identity domain again. Never append another
        // optional tail.
        match &self.body {
            None => bytes.push(0),
            Some(ActionBodyContract::Run { command }) => {
                bytes.push(1);
                encode_text(&mut bytes, command);
            }
        }
        bytes
    }
}

/// The marker byte appended to an advisory action's canonical record.
///
/// Distinct from every retry-kind discriminant (`0`/`1`/`2`) it can follow,
/// so a reader of the trailing bytes is never ambiguous.
const ADVISORY_MARKER: u8 = 0xA0;

/// A declaration-owned retry envelope.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RetryContract {
    /// Constant delay between attempts.
    Every {
        /// Number of further attempts after the first.
        count: u64,
        /// Delay between attempts.
        every: Duration,
    },
    /// Bounded backoff between attempts.
    Backoff {
        /// Number of further attempts after the first.
        count: u64,
        /// Minimum delay.
        min: Duration,
        /// Maximum delay.
        max: Duration,
    },
}

/// One declared child workflow callable.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChildContract {
    /// Child workflow type.
    pub name: String,
    /// Object schema over child parameters.
    #[serde(serialize_with = "crate::canonical::serialize_value")]
    pub input_schema: Value,
    /// Child result schema.
    #[serde(serialize_with = "crate::canonical::serialize_value")]
    pub output_schema: Value,
}

impl ChildContract {
    fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        encode_text(&mut bytes, &self.name);
        encode_json(&mut bytes, None, &self.input_schema);
        encode_json(&mut bytes, None, &self.output_schema);
        bytes
    }
}

/// One declared signal payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignalContract {
    /// Signal name.
    pub name: String,
    /// Signal payload schema.
    #[serde(serialize_with = "crate::canonical::serialize_value")]
    pub input_schema: Value,
}

/// Typed shape of an additional entry in the same package.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdditionalWorkflowContract {
    /// Routing workflow type.
    pub workflow_type: String,
    /// Entry input schema.
    #[serde(serialize_with = "crate::canonical::serialize_value")]
    pub input_schema: Value,
    /// Entry output schema.
    #[serde(serialize_with = "crate::canonical::serialize_value")]
    pub output_schema: Value,
}

fn encode_json(bytes: &mut Vec<u8>, parent_key: Option<&str>, value: &Value) {
    match value {
        Value::Null => bytes.push(0),
        Value::Bool(value) => bytes.extend_from_slice(&[1, u8::from(*value)]),
        Value::Number(value) => {
            bytes.push(2);
            encode_text(bytes, &value.to_string());
        }
        Value::String(value) => {
            bytes.push(3);
            encode_text(bytes, value);
        }
        Value::Array(values) => {
            bytes.push(4);
            let mut values = values.iter().collect::<Vec<_>>();
            if matches!(parent_key, Some("required" | "enum")) {
                values.sort_by_key(ToString::to_string);
            }
            encode_len(bytes, values.len());
            for value in values {
                encode_json(bytes, None, value);
            }
        }
        Value::Object(values) => {
            bytes.push(5);
            let mut entries = values.iter().collect::<Vec<_>>();
            entries.sort_by_key(|(left, _)| *left);
            encode_len(bytes, entries.len());
            for (key, value) in entries {
                encode_text(bytes, key);
                encode_json(bytes, Some(key), value);
            }
        }
    }
}

fn encode_len(bytes: &mut Vec<u8>, len: usize) {
    bytes.extend_from_slice(&(len as u64).to_be_bytes());
}

fn encode_text(bytes: &mut Vec<u8>, value: &str) {
    encode_len(bytes, value.len());
    bytes.extend_from_slice(value.as_bytes());
}

fn update_record(bytes: &mut Vec<u8>, record: &[u8]) {
    encode_len(bytes, record.len());
    bytes.extend_from_slice(record);
}

fn encode_optional_text(bytes: &mut Vec<u8>, value: Option<&str>) {
    match value {
        Some(value) => {
            bytes.push(1);
            encode_text(bytes, value);
        }
        None => bytes.push(0),
    }
}

fn encode_optional_duration(bytes: &mut Vec<u8>, value: Option<Duration>) {
    match value {
        Some(value) => {
            bytes.push(1);
            encode_duration(bytes, value);
        }
        None => bytes.push(0),
    }
}

fn encode_duration(bytes: &mut Vec<u8>, value: Duration) {
    bytes.extend_from_slice(&value.as_secs().to_be_bytes());
    bytes.extend_from_slice(&value.subsec_nanos().to_be_bytes());
}

#[cfg(test)]
#[path = "contract_tests.rs"]
mod tests;
