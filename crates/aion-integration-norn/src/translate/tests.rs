use super::*;
use aion_core::{ActivityId, InjectPriority, WorkflowId};
use serde_json::json;

/// Failures are reported as errors naming what was expected, so no assertion needs `unwrap`.
type TestResult = Result<(), Box<dyn std::error::Error>>;

fn identity() -> EventIdentity {
    EventIdentity {
        workflow_id: WorkflowId::new(Uuid::nil()),
        run_id: aion_core::RunId::new(Uuid::from_u128(0x11)),
        activity_id: ActivityId::from_sequence_position(7),
        attempt: 3,
    }
}

/// The `event/*` notification params Norn emits (`agent_event_to_value` shape, with
/// `agent_id` / `agent_role` added by its emitter).
///
/// # Errors
///
/// Returns an error when the fixture is not a JSON object — every `event/*` params is one.
fn params(value: Value, agent_role: &str) -> Result<Value, Box<dyn std::error::Error>> {
    let mut object = match value {
        Value::Object(object) => object,
        other => {
            return Err(
                format!("an event/* params fixture must be a JSON object, got {other}").into(),
            );
        }
    };
    object.insert(
        "agent_id".to_owned(),
        json!("11111111-1111-1111-1111-111111111111"),
    );
    object.insert("agent_role".to_owned(), json!(agent_role));
    Ok(Value::Object(object))
}

// --- OUT: event/* notification -> ActivityEvent, one per kind ---

#[test]
fn message_notification_maps_to_message_event() -> TestResult {
    let p = params(json!({ "type": "text", "text": "hello" }), "root")?;
    let event = notification_to_event(&identity(), 0, "event/message", &p);
    assert_eq!(event.attempt, 3);
    assert_eq!(event.agent_role, "root");
    let expected_agent = Uuid::parse_str("11111111-1111-1111-1111-111111111111")
        .map_err(|error| format!("the fixture agent id must parse: {error}"))?;
    assert_eq!(event.agent_id, expected_agent);
    assert!(!event.ephemeral);
    match event.kind {
        ActivityEventKind::Message { role, text } => {
            assert_eq!(role, MessageRole::Assistant);
            assert_eq!(text, "hello");
        }
        other => return Err(format!("expected a Message event, got {other:?}").into()),
    }
    Ok(())
}

#[test]
fn tool_call_notification_maps_to_tool_call_event() -> TestResult {
    let p = params(
        json!({
            "type": "tool_call",
            "call_id": "c1",
            "name": "read_file",
            "arguments": { "path": "/tmp/x" },
            "kind": "function",
        }),
        "root",
    )?;
    let event = notification_to_event(&identity(), 1, "event/toolCall", &p);
    match event.kind {
        ActivityEventKind::ToolCall {
            tool,
            call_id,
            input,
        } => {
            assert_eq!(tool, "read_file");
            assert_eq!(call_id, "c1");
            assert_eq!(input, json!({ "path": "/tmp/x" }));
        }
        other => return Err(format!("expected a ToolCall event, got {other:?}").into()),
    }
    Ok(())
}

#[test]
fn tool_result_notification_maps_to_tool_result_event() -> TestResult {
    let p = params(
        json!({
            "type": "tool_result",
            "tool_call_id": "c1",
            "tool_name": "read_file",
            "output": { "bytes": 12 },
            "duration_ms": 3,
        }),
        "root",
    )?;
    let event = notification_to_event(&identity(), 2, "event/toolResult", &p);
    match event.kind {
        ActivityEventKind::ToolResult {
            call_id,
            output,
            is_error,
        } => {
            assert_eq!(call_id, "c1");
            assert_eq!(output, json!({ "bytes": 12 }));
            assert!(!is_error);
        }
        other => return Err(format!("expected a ToolResult event, got {other:?}").into()),
    }
    Ok(())
}

#[test]
fn text_delta_notification_maps_to_ephemeral_delta() -> TestResult {
    let p = params(
        json!({ "type": "text_delta", "item_id": "m1", "text": "wor" }),
        "root",
    )?;
    let event = notification_to_event(&identity(), 3, "event/progress", &p);
    assert!(event.ephemeral, "a token delta is ephemeral");
    assert_eq!(event.store_seq, None);
    match event.kind {
        ActivityEventKind::Delta {
            message_id,
            text_fragment,
        } => {
            assert_eq!(message_id, "m1");
            assert_eq!(text_fragment, "wor");
        }
        other => return Err(format!("expected a Delta event, got {other:?}").into()),
    }
    Ok(())
}

#[test]
fn tool_call_delta_notification_is_ephemeral() -> TestResult {
    let p = params(
        json!({ "type": "tool_call_delta", "text": "{\"pa" }),
        "root",
    )?;
    let event = notification_to_event(&identity(), 3, "event/progress", &p);
    assert!(
        event.ephemeral,
        "tool-argument deltas are per-token and must never be persisted"
    );
    assert_eq!(event.store_seq, None);
    match event.kind {
        ActivityEventKind::Progress {
            detail: ProgressDetail::Note { text },
        } => assert_eq!(text, "tool_call_delta"),
        other => return Err(format!("expected a Progress::Note event, got {other:?}").into()),
    }
    Ok(())
}

#[test]
fn raw_per_token_delta_notification_is_ephemeral_and_drops_its_payload() -> TestResult {
    // The 2026-07-23 remediation-packet flood: provider-native per-token
    // frames arriving under `event/raw` were persisted whole (payload and
    // all), writing gigabytes per hour. They are the same class as the
    // `event/progress` delta arms and get the same ephemeral-note fate.
    let p = params(
        json!({
            "type": "response.function_call_arguments.delta",
            "delta": "{\"pa",
            "obfuscation": "xK9dQ2"
        }),
        "root",
    )?;
    let event = notification_to_event(&identity(), 3, "event/raw", &p);
    assert!(
        event.ephemeral,
        "per-token provider frames must never be persisted, whatever method they arrive under"
    );
    assert_eq!(event.store_seq, None);
    match event.kind {
        ActivityEventKind::Progress {
            detail: ProgressDetail::Note { text },
        } => assert_eq!(text, "response.function_call_arguments.delta"),
        other => return Err(format!("expected a Progress::Note event, got {other:?}").into()),
    }
    Ok(())
}

#[test]
fn raw_non_delta_notification_stays_durable() -> TestResult {
    let p = params(json!({ "type": "response.completed", "id": "r1" }), "root")?;
    let event = notification_to_event(&identity(), 3, "event/raw", &p);
    assert!(
        !event.ephemeral,
        "unclassified non-delta frames keep the durable Raw passthrough"
    );
    match event.kind {
        ActivityEventKind::Raw { source, .. } => assert_eq!(source, "event/raw"),
        other => return Err(format!("expected a Raw event, got {other:?}").into()),
    }
    Ok(())
}

#[test]
fn usage_estimate_notification_maps_to_progress() -> TestResult {
    let p = params(
        json!({ "type": "usage_estimate", "input_tokens": 100 }),
        "root",
    )?;
    let event = notification_to_event(&identity(), 4, "event/progress", &p);
    assert!(!event.ephemeral);
    match event.kind {
        ActivityEventKind::Progress {
            detail: ProgressDetail::UsageEstimate { input_tokens, .. },
        } => assert_eq!(input_tokens, Some(100)),
        other => {
            return Err(format!("expected a Progress::UsageEstimate event, got {other:?}").into());
        }
    }
    Ok(())
}

#[test]
fn stop_notification_maps_stop_reasons() -> TestResult {
    for (label, expected) in [
        ("end_turn", StopKind::EndTurn),
        ("tool_use", StopKind::ToolUse),
        ("max_tokens", StopKind::LimitReached),
    ] {
        let p = params(json!({ "type": "done", "stop_reason": label }), "root")?;
        let event = notification_to_event(&identity(), 5, "event/stop", &p);
        match event.kind {
            ActivityEventKind::Stop { reason } => assert_eq!(reason, expected),
            other => {
                return Err(format!("expected a Stop event for `{label}`, got {other:?}").into());
            }
        }
    }
    Ok(())
}

#[test]
fn unknown_stop_reason_falls_through_to_other() -> TestResult {
    let p = params(json!({ "type": "done", "stop_reason": "weird" }), "root")?;
    let event = notification_to_event(&identity(), 6, "event/stop", &p);
    match event.kind {
        ActivityEventKind::Stop {
            reason: StopKind::Other { reason },
        } => assert_eq!(reason, "weird"),
        other => return Err(format!("expected a Stop::Other event, got {other:?}").into()),
    }
    Ok(())
}

#[test]
fn unclassifiable_notification_falls_through_to_raw() -> TestResult {
    let p = params(json!({ "type": "compaction", "item_type": "x" }), "root")?;
    let event = notification_to_event(&identity(), 7, "event/raw", &p);
    match event.kind {
        ActivityEventKind::Raw { source, value } => {
            assert_eq!(source, "event/raw");
            assert_eq!(value["type"], json!("compaction"));
        }
        other => return Err(format!("expected a Raw event, got {other:?}").into()),
    }
    Ok(())
}

#[test]
fn missing_agent_id_falls_back_to_nil_never_panics() {
    // A notification with no agent attribution still yields a well-formed event.
    let event = notification_to_event(
        &identity(),
        0,
        "event/message",
        &json!({ "type": "text", "text": "hi" }),
    );
    assert_eq!(event.agent_id, Uuid::nil());
    assert_eq!(event.agent_role, "");
}

// --- IN: neutral InterventionKind -> Norn intervene/* request ---

#[test]
fn inject_interrupt_maps_to_inject_message_request() -> TestResult {
    let req = intervention_to_request(&InterventionKind::InjectMessage {
        text: "use the other module".to_owned(),
        priority: InjectPriority::Interrupt,
    })
    .map_err(|error| format!("an advertised inject must build a request: {error}"))?;
    assert_eq!(req.method, "intervene/injectMessage");
    assert_eq!(req.params["text"], json!("use the other module"));
    assert_eq!(req.params["priority"], json!("interrupt"));
    Ok(())
}

#[test]
fn inject_normal_maps_to_normal_priority() -> TestResult {
    let req = intervention_to_request(&InterventionKind::InjectMessage {
        text: "context".to_owned(),
        priority: InjectPriority::Normal,
    })
    .map_err(|error| format!("an advertised inject must build a request: {error}"))?;
    assert_eq!(req.params["priority"], json!("normal"));
    Ok(())
}

#[test]
fn cancel_maps_to_cancel_request() -> TestResult {
    let req = intervention_to_request(&InterventionKind::Cancel {
        reason: "operator abort".to_owned(),
    })
    .map_err(|error| format!("an advertised cancel must build a request: {error}"))?;
    assert_eq!(req.method, "intervene/cancel");
    assert_eq!(req.params["reason"], json!("operator abort"));
    Ok(())
}

#[test]
fn unsupported_primitives_are_capability_not_supported_and_never_build_a_request() -> TestResult {
    for kind in [
        InterventionKind::PauseResume { paused: true },
        InterventionKind::UpdateBudget {
            max_tokens: Some(1),
            max_turns: None,
        },
        InterventionKind::RespondToApproval {
            call_id: "c".to_owned(),
            decision: aion_core::ApprovalDecision::Approve,
            note: None,
        },
    ] {
        let Err(error) = intervention_to_request(&kind) else {
            return Err(format!("{kind:?} must be refused, never turned into a request").into());
        };
        assert!(
            matches!(error, HarnessError::CapabilityNotSupported { .. }),
            "an unsupported primitive must be capability-gated, got {error:?}"
        );
    }
    Ok(())
}

// --- capabilities parse (behind the protocol-version gate) ---

#[test]
fn parse_capabilities_reads_norns_advertised_set() -> TestResult {
    // The exact shape Norn's `initialize` result emits.
    let init = json!({
        "protocol": "norn-driven/1",
        "capabilities": { "interventions": ["inject_message", "cancel"] }
    });
    let caps = parse_capabilities(&init)
        .map_err(|error| format!("a norn-driven/1 advertisement must parse: {error}"))?;
    assert!(caps.supports_primitive(InterventionPrimitive::InjectMessage));
    assert!(caps.supports_primitive(InterventionPrimitive::Cancel));
    assert!(!caps.supports_primitive(InterventionPrimitive::PauseResume));
    Ok(())
}

#[test]
fn parse_capabilities_missing_array_is_empty_set() -> TestResult {
    let init = json!({ "protocol": "norn-driven/1", "capabilities": {} });
    let caps = parse_capabilities(&init).map_err(|error| {
        format!("an advertisement without an interventions array must parse: {error}")
    })?;
    assert!(
        caps.is_empty(),
        "no interventions advertised = observability-only"
    );
    Ok(())
}

#[test]
fn parse_capabilities_ignores_unknown_labels() -> TestResult {
    let init = json!({
        "protocol": "norn-driven/1",
        "capabilities": { "interventions": ["inject_message", "teleport"] }
    });
    let caps = parse_capabilities(&init)
        .map_err(|error| format!("an advertisement with an unknown label must parse: {error}"))?;
    assert!(caps.supports_primitive(InterventionPrimitive::InjectMessage));
    // "teleport" is unknown and silently ignored, never fabricated into a primitive.
    assert_eq!(caps.supported.len(), 1);
    Ok(())
}

// --- the protocol-version gate ---

#[test]
fn missing_protocol_field_is_a_protocol_error_naming_expected_and_received() -> TestResult {
    // The pre-norn-driven/1 shape: `protocolVersion: "2.0"` and no `protocol` field. This is
    // the "your norn binary is stale" signal.
    let init = json!({
        "protocolVersion": "2.0",
        "capabilities": { "interventions": ["inject_message", "cancel"] }
    });
    let Err(error) = parse_capabilities(&init) else {
        return Err("an advertisement with no `protocol` field must be refused".into());
    };
    assert!(
        matches!(error, HarnessError::Protocol { .. }),
        "a stale advertisement is a protocol error, got {error:?}"
    );
    let message = error.to_string();
    assert!(
        message.contains("norn-driven/1"),
        "the error names the expected version: {message}"
    );
    assert!(
        message.contains("no `protocol` field"),
        "the error names what was received: {message}"
    );
    Ok(())
}

#[test]
fn different_protocol_version_is_a_protocol_error_naming_expected_and_received() -> TestResult {
    let init = json!({
        "protocol": "norn-driven/2",
        "capabilities": { "interventions": ["inject_message"] }
    });
    let Err(error) = parse_capabilities(&init) else {
        return Err("a norn-driven/2 advertisement must be refused".into());
    };
    assert!(matches!(error, HarnessError::Protocol { .. }));
    let message = error.to_string();
    assert!(
        message.contains("norn-driven/1") && message.contains("norn-driven/2"),
        "the error names the expected AND received versions: {message}"
    );
    Ok(())
}

#[test]
fn non_string_protocol_field_is_reported_as_the_value_received() -> TestResult {
    // A present-but-non-string `protocol` (e.g. `1`) must be reported as what actually
    // arrived, never misdescribed as a missing field.
    let Err(error) = parse_capabilities(&json!({ "protocol": 1 })) else {
        return Err("a non-string `protocol` field must be refused".into());
    };
    assert!(matches!(error, HarnessError::Protocol { .. }));
    let message = error.to_string();
    assert!(
        message.contains("received 1"),
        "the error renders the received JSON value: {message}"
    );
    assert!(
        !message.contains("no `protocol` field"),
        "a present field must not be reported as missing: {message}"
    );
    Ok(())
}

// --- the run/execute stop envelope ---

mod outcomes;
