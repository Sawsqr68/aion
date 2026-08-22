use super::*;

#[test]
fn completed_string_output_is_the_canonical_agent_outcome_record() -> TestResult {
    // A schema-less run's sole final output is a string, so it honestly fills both `text` and
    // `final_message`; the envelope's own `completed` stop maps to canonical `end_turn`.
    let payload = payload_of(completed_envelope(&json!("the final answer")))
        .map_err(|error| format!("a completed string output must be a payload: {error}"))?;
    assert_eq!(payload.content_type(), &ContentType::Json);
    let decoded = payload
        .to_json()
        .map_err(|error| format!("the payload must be JSON: {error}"))?;
    assert_eq!(
        decoded,
        json!({ "text": "the final answer", "final_message": "the final answer", "stop_reason": "end_turn", "session_id": "step-042" })
    );
    Ok(())
}

/// The envelope-Dict path, at the session boundary — ruling #181 acceptance (3) and (6):
/// run `cc1d82c2`'s shape (a completed envelope whose `output` is a JSON object) must be a
/// typed contract refusal NAMING the canonical agent-outcome contract and what was found,
/// never a payload a workflow-side decoder then fails on with a serde error.
#[test]
fn a_completed_structured_output_is_refused_naming_the_agent_outcome_contract() -> TestResult {
    let output = json!({ "verdict": "pass", "notes": ["a", "b"] });
    let Err(error) = payload_of(completed_envelope(&output)) else {
        return Err("a structured completed output must be refused, never a payload".into());
    };
    assert!(
        matches!(error, HarnessError::Contract { .. }),
        "the refusal is the typed (terminal) Contract variant, got {error:?}"
    );
    let message = error.to_string();
    for fragment in [
        "agent-outcome",
        "`text: String`",
        "`stop_reason: String`",
        "a JSON object",
    ] {
        assert!(
            message.contains(fragment),
            "the refusal must name `{fragment}`: {message}"
        );
    }
    Ok(())
}

#[test]
fn each_non_completed_stop_reason_surfaces_with_its_detail_and_partial_verbatim() -> TestResult {
    // Every non-completed reason is a harness error whose message carries the reason AND its
    // per-variant detail fields — the caller judges retry on that text. The reasons that may
    // hold a partial `output` (timed_out / truncated / schema_unreachable) must carry it in
    // the same message, labelled, so an accept-the-partial policy stays reachable.
    let cases: Vec<(serde_json::Value, serde_json::Value, Vec<&str>)> = vec![
        (
            json!({ "reason": "schema_unreachable", "attempts": 3,
                    "validation_errors": ["missing field `verdict`"] }),
            json!({ "notes": "best attempt" }),
            vec![
                "schema_unreachable",
                "attempts",
                "3",
                "validation_errors",
                "missing field `verdict`",
                "partial output: {\"notes\":\"best attempt\"}",
            ],
        ),
        (
            json!({ "reason": "max_iterations" }),
            json!(null),
            vec!["max_iterations"],
        ),
        (
            json!({ "reason": "timed_out", "elapsed_ms": 300_000, "iterations": 12 }),
            json!("a half-written summary"),
            vec![
                "timed_out",
                "elapsed_ms",
                "300000",
                "iterations",
                "12",
                "partial output: \"a half-written summary\"",
            ],
        ),
        (
            json!({ "reason": "cancelled" }),
            json!(null),
            vec!["cancelled"],
        ),
        (
            json!({ "reason": "truncated", "truncation": "max_tokens", "iterations": 2 }),
            json!("cut off mid-sente"),
            vec![
                "truncated",
                "truncation",
                "max_tokens",
                "iterations",
                "2",
                "partial output: \"cut off mid-sente\"",
            ],
        ),
    ];
    for (stop, output, fragments) in cases {
        let Err(error) = payload_of(stopped_envelope(&stop, &output)) else {
            return Err(format!("stop {stop} must be refused as an error, never a payload").into());
        };
        assert!(
            matches!(error, HarnessError::Harness { .. }),
            "a non-completed stop is a harness error, got {error:?}"
        );
        let message = error.to_string();
        for fragment in fragments {
            assert!(
                message.contains(fragment),
                "stop {stop} must surface `{fragment}` verbatim, got: {message}"
            );
        }
        if output.is_null() {
            assert!(
                !message.contains("partial output"),
                "a null output must not fabricate a partial: {message}"
            );
        }
    }
    Ok(())
}

#[test]
fn structured_provider_policy_refusal_has_its_own_class() -> TestResult {
    let response: JsonRpcResponse = serde_json::from_value(json!({
        "jsonrpc": "2.0",
        "id": 1,
        "error": {
            "code": -32603,
            "message": "agent error",
            "data": { "error": { "code": "cyber_policy" } }
        }
    }))?;
    let Err(error) = response_into_payload(response) else {
        return Err("a structured policy refusal must not be a payload".into());
    };
    assert!(matches!(
        error,
        HarnessError::PolicyRefused {
            detection: PolicyRefusalDetection::Structured,
            ..
        }
    ));
    Ok(())
}

#[test]
fn measured_norn_policy_phrase_is_the_narrow_textual_fallback() -> TestResult {
    let response: JsonRpcResponse = serde_json::from_value(json!({
        "jsonrpc": "2.0",
        "id": 1,
        "error": {
            "code": -32603,
            "message": "provider error: invalid request: provider rejected the request under its cybersecurity policy"
        }
    }))?;
    let Err(error) = response_into_payload(response) else {
        return Err("the measured policy refusal phrase must not be a payload".into());
    };
    assert!(matches!(
        error,
        HarnessError::PolicyRefused {
            detection: PolicyRefusalDetection::Textual,
            ..
        }
    ));
    Ok(())
}

#[test]
fn ordinary_generic_json_rpc_failure_remains_harness_retryable() -> TestResult {
    let response: JsonRpcResponse = serde_json::from_value(json!({
        "jsonrpc": "2.0",
        "id": 1,
        "error": {
            "code": -32603,
            "message": "provider overloaded",
            "data": { "error": { "code": "server_error" } }
        }
    }))?;
    let Err(error) = response_into_payload(response) else {
        return Err("an ordinary JSON-RPC failure must not be a payload".into());
    };
    assert!(matches!(error, HarnessError::Harness { .. }));
    assert!(!error.is_deterministic());
    Ok(())
}

#[test]
fn null_result_is_the_slash_command_harness_error() -> TestResult {
    // A prompt that resolves entirely to a local slash command is answered with a success
    // Response whose result is JSON null — an agent activity must produce output.
    let response: JsonRpcResponse =
        serde_json::from_value(json!({ "jsonrpc": "2.0", "id": 1, "result": null }))
            .map_err(|error| format!("the canned null-result frame must decode: {error}"))?;
    let Err(error) = response_into_payload(response) else {
        return Err("a null result must be refused as an error, never a payload".into());
    };
    assert!(
        matches!(error, HarnessError::Harness { .. }),
        "a null result is a harness error, got {error:?}"
    );
    assert_eq!(
        error.to_string(),
        "harness reported failure: run resolved to a local slash command; no output"
    );
    Ok(())
}

#[test]
fn a_response_with_neither_result_nor_error_is_a_protocol_error() -> TestResult {
    // A frame with NO `result` key and no `error` is a broken peer — distinct from the legal
    // `"result": null` slash-command answer above.
    let response: JsonRpcResponse = serde_json::from_value(json!({ "jsonrpc": "2.0", "id": 1 }))
        .map_err(|error| format!("the canned result-less frame must decode: {error}"))?;
    let Err(error) = response_into_payload(response) else {
        return Err(
            "a frame with neither result nor error must be refused, never a payload".into(),
        );
    };
    assert!(
        matches!(error, HarnessError::Protocol { .. }),
        "a result-less, error-less response is a protocol error, got {error:?}"
    );
    assert!(
        error
            .to_string()
            .contains("carried neither result nor error"),
        "the error names the broken frame: {error}"
    );
    Ok(())
}

#[test]
fn completed_envelope_with_present_null_output_is_refused_naming_null() -> TestResult {
    // `"output": null` is norn's legal null output, but null cannot fill the agent-outcome
    // record's `text: String` — since #181 it is a typed contract refusal naming the null
    // that arrived, distinct from the missing-key protocol error below.
    let Err(error) = payload_of(completed_envelope(&json!(null))) else {
        return Err("a null output cannot fill `text: String`; it must be refused".into());
    };
    let message = error.to_string();
    assert!(
        message.contains("is null"),
        "the refusal names the null that arrived: {message}"
    );
    assert!(
        message.contains("agent-outcome"),
        "the refusal names the canonical contract: {message}"
    );
    Ok(())
}

#[test]
fn completed_envelope_missing_the_output_key_is_a_protocol_error() -> TestResult {
    // The contract says completed ALWAYS carries `output`; an absent key is off-contract.
    let Err(error) = payload_of(json!({
        "envelope_version": 1,
        "stop": { "reason": "completed" },
    })) else {
        return Err("a completed envelope without output must be refused, never a payload".into());
    };
    assert!(
        matches!(error, HarnessError::Protocol { .. }),
        "a completed envelope without output is a protocol error, got {error:?}"
    );
    assert!(
        error
            .to_string()
            .contains("completed envelope carried no output field")
    );
    Ok(())
}

#[test]
fn non_envelope_result_is_a_protocol_error_never_a_silent_passthrough() -> TestResult {
    // The pre-envelope result shape must be rejected naming what was missing.
    let Err(error) = payload_of(json!({ "result": "completed", "output": "x" })) else {
        return Err("a pre-envelope result shape must be refused, never a payload".into());
    };
    assert!(
        matches!(error, HarnessError::Protocol { .. }),
        "an unknown result shape is a protocol error, got {error:?}"
    );
    assert!(error.to_string().contains("envelope_version"));

    let Err(error) = payload_of(json!({ "envelope_version": 1, "output": "x" })) else {
        return Err("an envelope without `stop` must be refused, never a payload".into());
    };
    assert!(matches!(error, HarnessError::Protocol { .. }));
    assert!(error.to_string().contains("`stop`"));
    Ok(())
}
