//! Where one attempt's working directory comes from, and how it is obtained.
//!
//! # Why this is a building block and not part of the seam
//!
//! [`crate::contract::AgentHarness`] is harness-blind: `start` takes only neutral run
//! identity and the input [`Payload`], never harness configuration. The directory an agent
//! stands in IS harness configuration — it is declared in the worker document's `harness`
//! section — so it stays on the adapter's own config type and never grows a field on
//! [`crate::AgentRunSpec`].
//!
//! What it cannot stay is a plain path. A worker serves many jobs, and which tree a job
//! concerns is a property of the JOB: a document written before any work exists can only
//! name one tree, and one tree is only ever right for a worker that serves one tree. So
//! the configuration holds a SOURCE, and the source is resolved once per attempt, against
//! that attempt's input.
//!
//! # Why the resolution lives here rather than in each adapter
//!
//! Two adapters need the same answer from the same bytes, and two implementations of
//! "read this parameter out of the input" would be two chances to disagree about what an
//! absent parameter means — which is the one case that must never resolve into the
//! launching process's own directory. One function, one refusal set, both adapters.

use std::path::{Path, PathBuf};

use aion_core::Payload;

use crate::error::HarnessError;

/// Where each attempt's working directory comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HarnessWorkspace {
    /// Every attempt stands in this one directory, resolved and absolute. The form for a
    /// worker that serves exactly one tree.
    Fixed(PathBuf),
    /// Each attempt's directory arrives in that attempt's input, under this parameter
    /// name. The form for a worker whose jobs each name their own tree.
    PerRun(String),
}

impl HarnessWorkspace {
    /// This attempt's working directory.
    ///
    /// The fixed form ignores the input entirely. The per-run form reads the named
    /// parameter out of it and refuses — TERMINALLY, never falling back — when the input
    /// is not a JSON object, when the parameter is absent, when it is not a string, or
    /// when it is blank. Every one of those is a job that did not say where to work, and
    /// an agent that guesses stands in the launching process's directory: invisible in the
    /// document, in the argv and in the log, which is the whole hazard the setting exists
    /// to close.
    ///
    /// The path is returned as WRITTEN. Whether it is absolute, exists, and is a directory
    /// is the launching adapter's check, kept there so one adapter's refusal wording and
    /// one adapter's spawn stay in the same place.
    ///
    /// # Errors
    ///
    /// Returns [`HarnessError::Configuration`] for every case above. That variant is the
    /// right one for the same reason it is right for a malformed `env_pass`: nothing was
    /// spawned, no frame was exchanged, and what is wrong is a value somebody WROTE — here
    /// in the workflow that dispatched the job rather than in the worker document. The next
    /// attempt reads the same input and meets the same wall, so the worker's mapping of
    /// this variant to a TERMINAL failure is exactly what should happen.
    pub fn for_attempt(&self, input: &Payload) -> Result<PathBuf, HarnessError> {
        match self {
            Self::Fixed(directory) => Ok(directory.clone()),
            Self::PerRun(parameter) => Self::read(parameter, input),
        }
    }

    /// The fixed directory, when this workspace is the fixed form.
    ///
    /// For the callers that report a launch before any attempt exists — a worker's startup
    /// narration has no input to resolve against, and saying "the directory this worker
    /// uses" of a per-run workspace would be stating a fact that does not exist.
    #[must_use]
    pub fn fixed(&self) -> Option<&Path> {
        match self {
            Self::Fixed(directory) => Some(directory.as_path()),
            Self::PerRun(_) => None,
        }
    }

    /// The parameter name, when the directory arrives with each job.
    #[must_use]
    pub fn per_run(&self) -> Option<&str> {
        match self {
            Self::PerRun(parameter) => Some(parameter.as_str()),
            Self::Fixed(_) => None,
        }
    }

    /// The prompt text this attempt's input carries.
    ///
    /// The activity input is a serialized [`Payload`], not raw prompt text, so the decoding
    /// is deliberate:
    ///
    /// - Bytes that are not valid JSON → the raw UTF-8 text, unchanged ([`Payload`] is a
    ///   dumb carrier that does not validate on construction).
    /// - JSON **string** → the inner string, so the agent receives the exact text a caller
    ///   passed and a multi-line prompt survives verbatim.
    /// - JSON **object** → the ONE field that is not this workspace's directory parameter
    ///   and not one of the caller's `reserved` parameter names, read by name. This is the
    ///   shape an authored action produces: its input is an object keyed by parameter name
    ///   even when it declares a single parameter, so an adapter that passed the object
    ///   through handed the agent `{"prompt":"…"}` as its literal instructions.
    /// - Any other JSON value (array, number, boolean, null) → the raw JSON text: there is
    ///   no field to read by, and inventing a projection would lose information.
    ///
    /// `reserved` names every further input parameter the caller's adapter reads for
    /// itself — a session parameter, say — so the prompt stays defined by SUBTRACTION: the
    /// prompt is whatever remains once every parameter the adapter already knows by name is
    /// removed. An adapter with no such parameters passes `&[]` and the rule is unchanged.
    ///
    /// The object arm is read by NAME rather than by position because position is not
    /// carried on the wire — a JSON object has no order that survives serialization — and
    /// because the subtracted fields are exactly the fields whose names the adapter already
    /// knows. Subtracting them leaves exactly one field for every shape the checker admits,
    /// which is what makes this a total function rather than a guess.
    ///
    /// # Errors
    ///
    /// [`HarnessError::Protocol`] when the bytes are not UTF-8 at all. Otherwise
    /// [`HarnessError::Configuration`] — terminal, never retried — when an object carries
    /// no prompt field, carries more than one, or carries one that is not a string. Each is
    /// a call that did not state what to ask the agent, and an adapter that guessed would
    /// send an agent instructions nobody wrote.
    pub fn prompt_for_attempt(
        &self,
        input: &Payload,
        reserved: &[&str],
    ) -> Result<String, HarnessError> {
        let text = std::str::from_utf8(input.bytes())
            .map(str::to_owned)
            .map_err(|source| {
                HarnessError::protocol(format!("the run input is not valid UTF-8: {source}"))
            })?;
        // Matched exhaustively rather than guarded with `!matches!(…, Json)`: a payload
        // carries exactly one content type today, so a guard would be an arm that cannot
        // run and a claim about behaviour nobody can observe. A second content type added
        // later stops compiling here, which is where the decision about it belongs.
        match input.content_type() {
            aion_core::ContentType::Json => {
                match serde_json::from_str::<serde_json::Value>(&text) {
                    Ok(serde_json::Value::String(inner)) => Ok(inner),
                    Ok(serde_json::Value::Object(fields)) => self.prompt_field(&fields, reserved),
                    _ => Ok(text),
                }
            }
        }
    }

    /// Reads the prompt out of an input object: the one field that is neither the
    /// directory nor a reserved adapter parameter.
    fn prompt_field(
        &self,
        fields: &serde_json::Map<String, serde_json::Value>,
        reserved: &[&str],
    ) -> Result<String, HarnessError> {
        let directory = self.per_run();
        let carried = fields
            .iter()
            .filter(|(name, _)| {
                Some(name.as_str()) != directory && !reserved.contains(&name.as_str())
            })
            .collect::<Vec<_>>();
        let [(name, value)] = carried.as_slice() else {
            return Err(HarnessError::configuration(format!(
                "an agent is asked one thing, and this job's input carries {carried} \
                 {besides} to ask it with{named}. The agent is not started rather than \
                 started on instructions nobody wrote.",
                carried = carried.len(),
                besides = match (directory, reserved.is_empty()) {
                    (Some(_), _) | (None, false) =>
                        "fields besides the parameters this worker's harness reads itself",
                    (None, true) => "fields",
                },
                named = list(&carried)
            )));
        };
        let Some(prompt) = value.as_str() else {
            return Err(HarnessError::configuration(format!(
                "an agent is asked in words, and this job carries its `{name}` as {kind} \
                 rather than text",
                kind = describe(value)
            )));
        };
        Ok(prompt.to_owned())
    }

    /// Reads one named string parameter out of an attempt's input.
    fn read(parameter: &str, input: &Payload) -> Result<PathBuf, HarnessError> {
        let value: serde_json::Value = serde_json::from_slice(input.bytes()).map_err(|error| {
            HarnessError::configuration(format!(
                "this worker takes each agent's working directory from the `{parameter}` \
                     parameter of the job, and this job's input is not readable as JSON: {error}"
            ))
        })?;
        let serde_json::Value::Object(fields) = value else {
            return Err(HarnessError::configuration(format!(
                "this worker takes each agent's working directory from the `{parameter}` \
                 parameter of the job, and this job's input is not an object, so it carries no \
                 parameters at all"
            )));
        };
        let Some(field) = fields.get(parameter) else {
            return Err(HarnessError::configuration(format!(
                "this worker takes each agent's working directory from the `{parameter}` \
                 parameter of the job, and this job's input does not carry it. The agent is \
                 not started rather than started in whichever directory this worker process \
                 happens to be in."
            )));
        };
        let Some(directory) = field.as_str() else {
            return Err(HarnessError::configuration(format!(
                "this worker takes each agent's working directory from the `{parameter}` \
                 parameter of the job, and this job carries `{parameter}` as {kind} rather \
                 than a path",
                kind = describe(field)
            )));
        };
        if directory.trim().is_empty() {
            return Err(HarnessError::configuration(format!(
                "this worker takes each agent's working directory from the `{parameter}` \
                 parameter of the job, and this job carries it empty. An empty directory is \
                 not the current one; it is a job that did not say where to work."
            )));
        }
        Ok(PathBuf::from(directory))
    }
}

/// The field names a refusal reports, so an author can see what arrived.
///
/// Empty for an empty set rather than an empty list, because " (): " reads as a fault in
/// the message itself.
fn list(fields: &[(&String, &serde_json::Value)]) -> String {
    if fields.is_empty() {
        return String::new();
    }
    format!(
        " ({})",
        fields
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    )
}

/// What a JSON value is, for a refusal that has to say what arrived instead of a path.
fn describe(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "a boolean",
        serde_json::Value::Number(_) => "a number",
        serde_json::Value::String(_) => "a string",
        serde_json::Value::Array(_) => "a list",
        serde_json::Value::Object(_) => "an object",
    }
}

#[cfg(test)]
mod tests;
