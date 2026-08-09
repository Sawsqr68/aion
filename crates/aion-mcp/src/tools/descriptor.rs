//! Tool descriptors and behaviour annotations.

use serde::Serialize;
use serde::ser::SerializeMap;
use serde_json::Value;

/// Whether a tool modifies its environment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Modification {
    /// The tool only reads.
    ReadOnly,
    /// The tool changes something.
    Mutating,
}

/// Whether a mutating tool may destroy or overwrite existing state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Destructiveness {
    /// May destroy or overwrite.
    Destructive,
    /// Only adds or advances.
    Additive,
}

/// Whether repeating a call with the same arguments has any additional effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Idempotence {
    /// Repeating is a no-op.
    Idempotent,
    /// Repeating does the work again.
    Repeating,
}

/// Whether the tool reaches an open world of external entities.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorldScope {
    /// Reaches arbitrary external entities.
    Open,
    /// Operates on a closed, known domain.
    Closed,
}

/// Behaviour hints a client may use to decide how to present a tool.
///
/// All four are stated explicitly on every tool. The spec's defaults are the
/// *dangerous* readings — `destructiveHint` defaults to true and
/// `openWorldHint` defaults to true — so an omitted hint makes a pure read look
/// destructive and world-reaching. Stating each one is the only way a client
/// sees the truth. Each is modelled as a named two-variant enum rather than a
/// bare `bool` so neither side of the pair can be written by accident.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToolAnnotations {
    /// Whether the tool modifies anything.
    pub modification: Modification,
    /// Whether it may destroy existing state.
    pub destructiveness: Destructiveness,
    /// Whether repeating it is a no-op.
    pub idempotence: Idempotence,
    /// Whether it reaches an open world.
    pub world: WorldScope,
}

impl ToolAnnotations {
    /// A read-only tool: modifies nothing, and therefore neither destroys nor
    /// repeats an effect.
    #[must_use]
    pub const fn read_only(world: WorldScope) -> Self {
        Self {
            modification: Modification::ReadOnly,
            destructiveness: Destructiveness::Additive,
            idempotence: Idempotence::Idempotent,
            world,
        }
    }

    /// A mutating tool, with its destructive and idempotent character stated
    /// rather than defaulted.
    #[must_use]
    pub const fn mutating(
        destructiveness: Destructiveness,
        idempotence: Idempotence,
        world: WorldScope,
    ) -> Self {
        Self {
            modification: Modification::Mutating,
            destructiveness,
            idempotence,
            world,
        }
    }

    /// Whether this tool is annotated read-only.
    #[must_use]
    pub const fn is_read_only(self) -> bool {
        matches!(self.modification, Modification::ReadOnly)
    }
}

impl Serialize for ToolAnnotations {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(4))?;
        map.serialize_entry("readOnlyHint", &self.is_read_only())?;
        map.serialize_entry(
            "destructiveHint",
            &matches!(self.destructiveness, Destructiveness::Destructive),
        )?;
        map.serialize_entry(
            "idempotentHint",
            &matches!(self.idempotence, Idempotence::Idempotent),
        )?;
        map.serialize_entry("openWorldHint", &matches!(self.world, WorldScope::Open))?;
        map.end()
    }
}

/// One tool the server publishes.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Tool {
    /// Programmatic identifier: 1–128 characters of `A-Za-z0-9_-.`, unique
    /// within the server.
    pub name: String,
    /// Human display name; takes precedence over `annotations.title` and `name`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// What the tool does, written for the model that will call it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// JSON Schema (2020-12) for the arguments object. Root `type` is
    /// `"object"`.
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
    /// JSON Schema (2020-12) the tool's `structuredContent` conforms to.
    #[serde(rename = "outputSchema", skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<Value>,
    /// Behaviour hints.
    pub annotations: ToolAnnotations,
}

impl Tool {
    /// Whether `name` satisfies the tool-naming guidance: 1–128 characters,
    /// drawn from `A-Za-z0-9_-.` only.
    #[must_use]
    pub fn is_valid_name(name: &str) -> bool {
        !name.is_empty()
            && name.chars().count() <= 128
            && name
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || "_-.".contains(character))
    }

    /// Whether the input schema is a JSON Schema **object** schema, as required.
    #[must_use]
    pub fn has_object_input_schema(&self) -> bool {
        self.input_schema
            .get("type")
            .and_then(Value::as_str)
            .is_some_and(|kind| kind == "object")
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{Destructiveness, Idempotence, Modification, Tool, ToolAnnotations, WorldScope};

    #[test]
    fn annotation_constructors_state_every_hint() {
        let read = ToolAnnotations::read_only(WorldScope::Closed);
        assert_eq!(read.modification, Modification::ReadOnly);
        assert_eq!(read.destructiveness, Destructiveness::Additive);
        assert_eq!(read.idempotence, Idempotence::Idempotent);
        let write = ToolAnnotations::mutating(
            Destructiveness::Destructive,
            Idempotence::Repeating,
            WorldScope::Closed,
        );
        assert_eq!(write.modification, Modification::Mutating);
        assert!(!write.is_read_only());
    }

    #[test]
    fn annotations_serialize_under_their_wire_names() -> Result<(), serde_json::Error> {
        let encoded = serde_json::to_value(ToolAnnotations::read_only(WorldScope::Closed))?;
        assert_eq!(encoded["readOnlyHint"], true);
        assert_eq!(encoded["destructiveHint"], false);
        assert_eq!(encoded["idempotentHint"], true);
        assert_eq!(encoded["openWorldHint"], false);

        let mutating = serde_json::to_value(ToolAnnotations::mutating(
            Destructiveness::Destructive,
            Idempotence::Repeating,
            WorldScope::Open,
        ))?;
        assert_eq!(mutating["readOnlyHint"], false);
        assert_eq!(mutating["destructiveHint"], true);
        assert_eq!(mutating["idempotentHint"], false);
        assert_eq!(mutating["openWorldHint"], true);
        Ok(())
    }

    #[test]
    fn tool_names_are_checked_against_the_permitted_alphabet() {
        assert!(Tool::is_valid_name("read_transcript"));
        assert!(Tool::is_valid_name("aion.read-history"));
        assert!(!Tool::is_valid_name(""));
        assert!(!Tool::is_valid_name("read transcript"));
        assert!(!Tool::is_valid_name("read,transcript"));
        assert!(!Tool::is_valid_name(&"a".repeat(129)));
    }

    #[test]
    fn an_object_input_schema_is_recognised() {
        let tool = Tool {
            name: "x".to_owned(),
            title: None,
            description: None,
            input_schema: json!({ "type": "object", "additionalProperties": false }),
            output_schema: None,
            annotations: ToolAnnotations::read_only(WorldScope::Closed),
        };
        assert!(tool.has_object_input_schema());
    }
}
