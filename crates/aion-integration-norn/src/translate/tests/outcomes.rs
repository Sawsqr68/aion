use super::*;

#[test]
fn completed_string_output_becomes_the_agent_outcome_record() -> TestResult {
    let envelope = json!({
        "envelope_version": 1,
        "stop": { "reason": "completed" },
        "output": "the final answer",
        "session_id": "norn-sess-7",
    });
    let outcome = run_result_to_outcome(&envelope)
        .map_err(|error| format!("a completed string output must yield the record: {error}"))?;
    assert_eq!(outcome.text, "the final answer");
    assert_eq!(
        outcome.final_message, outcome.text,
        "norn has one terminal output, so the last message equals the accumulated text"
    );
    assert_eq!(outcome.stop_reason, "end_turn");
    assert_eq!(
        outcome.session_id, "norn-sess-7",
        "the record carries the envelope's own session id — the fact, never a derivation"
    );
    Ok(())
}

/// THE DECISION, encoded as a test rather than a comment (ruling #181): norn's envelope
/// reason `completed` maps to the canonical `end_turn`. The stream mapping ([`stop_kind`])
/// carries no `completed` arm — it maps per-turn `event/stop` reasons — so the run-level
/// reason is resolved against norn's own protocol document (DRIVEN-PROTOCOL.md, "Stop
/// envelope": `completed` = the run finished normally, exit 0, `output` holds the final
/// value), which is exactly the neutral `EndTurn` semantics. Never invented: it is the
/// envelope's real `stop.reason`, mapped.
#[test]
fn a_completed_envelope_maps_to_the_canonical_end_turn_stop_reason() -> TestResult {
    let envelope = json!({
        "envelope_version": 1,
        "stop": { "reason": "completed" },
        "output": "done",
        "session_id": "norn-sess-7",
    });
    let outcome = run_result_to_outcome(&envelope)
        .map_err(|error| format!("a completed envelope must yield the record: {error}"))?;
    assert_eq!(
        outcome.stop_reason,
        StopKind::EndTurn.canonical_stop_reason(),
        "norn `completed` resolves to the canonical `end_turn`, through the one mapping"
    );
    Ok(())
}

/// The `--no-session` refusal (ruling #205 condition 1): the stop envelope contractually
/// reports `session_id: null` ONLY when no session was opened, and a completed run without a
/// session handle cannot fill the record's mandatory `session_id` honestly. Both spellings of
/// "no handle" — an explicit `null` and an absent key — are the same deterministic Contract
/// refusal naming `--no-session`, so a misconfigured worker fails the same way every attempt.
#[test]
fn a_completed_envelope_without_a_session_id_is_a_contract_refusal_naming_no_session() -> TestResult
{
    let without_key = json!({
        "envelope_version": 1,
        "stop": { "reason": "completed" },
        "output": "done",
    });
    let with_null = json!({
        "envelope_version": 1,
        "stop": { "reason": "completed" },
        "output": "done",
        "session_id": null,
    });
    for envelope in [without_key, with_null] {
        let Err(error) = run_result_to_outcome(&envelope) else {
            return Err("a completed run with no session handle must be refused".into());
        };
        assert!(
            matches!(error, HarnessError::Contract { .. }),
            "no-handle is a deterministic Contract refusal, got {error:?}"
        );
        let message = error.to_string();
        assert!(
            message.contains("--no-session"),
            "the refusal names the configuration that causes it: {message}"
        );
        assert!(
            message.contains("session_id"),
            "the refusal names the missing field: {message}"
        );
    }
    Ok(())
}

/// An empty-string `session_id` is off the stop-envelope contract (`<id | null>`, never `""`)
/// — a Protocol refusal, distinct from the `--no-session` Contract arm, so a norn that emits
/// a blank handle is reported as speaking the protocol wrongly rather than as misconfigured.
#[test]
fn an_empty_string_session_id_is_a_protocol_refusal() -> TestResult {
    let envelope = json!({
        "envelope_version": 1,
        "stop": { "reason": "completed" },
        "output": "done",
        "session_id": "",
    });
    let Err(error) = run_result_to_outcome(&envelope) else {
        return Err("an empty-string session_id must be refused".into());
    };
    assert!(
        matches!(error, HarnessError::Protocol { .. }),
        "an off-contract spelling is a Protocol refusal, got {error:?}"
    );
    let message = error.to_string();
    assert!(
        message.contains("never \"\""),
        "the refusal states the contract's two legal spellings: {message}"
    );
    Ok(())
}

/// A non-string, non-null `session_id` is a malformed envelope — a Protocol refusal that
/// names what was found instead of decoding garbage into the record.
#[test]
fn a_non_string_session_id_is_a_protocol_refusal() -> TestResult {
    let envelope = json!({
        "envelope_version": 1,
        "stop": { "reason": "completed" },
        "output": "done",
        "session_id": 42,
    });
    let Err(error) = run_result_to_outcome(&envelope) else {
        return Err("a non-string session_id must be refused".into());
    };
    assert!(
        matches!(error, HarnessError::Protocol { .. }),
        "a malformed field is a Protocol refusal, got {error:?}"
    );
    Ok(())
}

/// The envelope-Dict path (ruling #181 acceptance 3 + 6, live repro `cc1d82c2`): a completed
/// envelope whose `output` is not a String is a typed contract refusal NAMING the canonical
/// agent-outcome contract and what was found — never a value passed through for a
/// workflow-side decoder to fail on with "Expected String, found Dict".
#[test]
fn a_completed_non_string_output_is_a_contract_refusal_naming_what_was_found() -> TestResult {
    let cases = [
        (json!({ "answer": 7 }), "a JSON object"),
        (json!(["a", "b"]), "a JSON array"),
        (json!(7), "a JSON number"),
        (json!(true), "a JSON boolean"),
        (json!(null), "null"),
    ];
    for (output, found) in cases {
        let envelope = json!({
            "envelope_version": 1,
            "stop": { "reason": "completed" },
            "output": output,
        });
        let Err(error) = run_result_to_outcome(&envelope) else {
            return Err(format!(
                "a completed `output` of {found} must be refused, never yield a record"
            )
            .into());
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
            found,
        ] {
            assert!(
                message.contains(fragment),
                "the refusal must name `{fragment}`: {message}"
            );
        }
    }
    Ok(())
}

#[test]
fn non_completed_stop_is_a_harness_error_carrying_the_stop_object_and_partial() -> TestResult {
    let envelope = json!({
        "envelope_version": 1,
        "stop": { "reason": "timed_out", "elapsed_ms": 30000, "iterations": 4 },
        "output": "a half-written summary",
    });
    let Err(error) = run_result_to_outcome(&envelope) else {
        return Err("a timed_out envelope must be refused, never yield an output".into());
    };
    assert!(matches!(error, HarnessError::Harness { .. }));
    let message = error.to_string();
    for fragment in ["timed_out", "elapsed_ms", "30000", "iterations", "4"] {
        assert!(
            message.contains(fragment),
            "the stop detail survives verbatim ({fragment}): {message}"
        );
    }
    assert!(
        message.contains("partial output: \"a half-written summary\""),
        "the partial output rides the message, labelled: {message}"
    );
    Ok(())
}

#[test]
fn non_completed_stop_with_null_output_carries_no_partial_label() -> TestResult {
    // `max_iterations`/`cancelled` envelopes carry `output: null` — nothing to preserve, so
    // the message must not fabricate a partial.
    let envelope = json!({
        "envelope_version": 1,
        "stop": { "reason": "max_iterations" },
        "output": null,
    });
    let Err(error) = run_result_to_outcome(&envelope) else {
        return Err("a max_iterations envelope must be refused, never yield an output".into());
    };
    let message = error.to_string();
    assert!(
        !message.contains("partial output"),
        "a null output is not a partial: {message}"
    );
    Ok(())
}

#[test]
fn an_enormous_partial_output_is_bounded_with_an_explicit_marker() -> TestResult {
    // The bound is presentational truncation of the error MESSAGE, not data loss — the run
    // already failed. The marker makes the cut explicit.
    let huge = "x".repeat(10_000);
    let envelope = json!({
        "envelope_version": 1,
        "stop": { "reason": "truncated", "truncation": "max_tokens", "iterations": 2 },
        "output": huge,
    });
    let Err(error) = run_result_to_outcome(&envelope) else {
        return Err("a truncated envelope must be refused, never yield an output".into());
    };
    let message = error.to_string();
    assert!(
        message.contains("partial output: "),
        "the partial still rides, bounded: {message:.120}"
    );
    assert!(
        message.contains("truncated for this message"),
        "the truncation marker is explicit"
    );
    assert!(
        message.len() < 5_000,
        "the message is bounded, got {} bytes",
        message.len()
    );
    Ok(())
}

#[test]
fn a_completed_envelope_without_the_output_key_is_a_protocol_error() -> TestResult {
    // The contract says completed ALWAYS carries `output`; an absent key is off-contract and
    // must never be silently coerced into a null payload.
    let envelope = json!({
        "envelope_version": 1,
        "stop": { "reason": "completed" },
    });
    let Err(error) = run_result_to_outcome(&envelope) else {
        return Err(
            "a completed envelope without `output` must be refused, never yield an output".into(),
        );
    };
    assert!(
        matches!(error, HarnessError::Protocol { .. }),
        "a completed envelope without output is a protocol error, got {error:?}"
    );
    assert!(
        error
            .to_string()
            .contains("completed envelope carried no output field"),
        "the error names the off-contract shape: {error}"
    );
    Ok(())
}

#[test]
fn a_present_null_output_is_distinct_from_a_missing_key_and_refused_as_null() -> TestResult {
    // A present `"output": null` is norn's legal null output, distinct from a MISSING key (a
    // broken frame). Under the agent-outcome contract neither can complete — but the refusal
    // must name what actually arrived: `null`, never "carried no output field".
    //
    // THE CLASSIFICATION DECISION (#181 mi6), settled against norn's own protocol document
    // (DRIVEN-PROTOCOL.md, "Stop envelope" table): `null` output belongs ONLY to the
    // non-completed reasons (`max_iterations`, `cancelled`, `error`); the `completed` row
    // holds "the final output value" with no "if any" qualifier — unlike `timed_out` /
    // `schema_unreachable`, whose partials are explicitly optional. A completed envelope
    // carrying `null` is therefore configuration- or peer-determined (an output schema
    // admitting null, or an off-doc peer), never a transient property of a legitimate run —
    // so it is the DETERMINISTIC `Contract` refusal (terminal in the worker's mapping),
    // exactly like every other non-String completed output.
    let envelope = json!({
        "envelope_version": 1,
        "stop": { "reason": "completed" },
        "output": null,
    });
    let Err(error) = run_result_to_outcome(&envelope) else {
        return Err("a null output cannot fill `text: String`; it must be refused".into());
    };
    assert!(
        matches!(error, HarnessError::Contract { .. }),
        "a completed null output is the deterministic Contract refusal, got {error:?}"
    );
    let message = error.to_string();
    assert!(
        message.contains("is null"),
        "the refusal names the null that arrived: {message}"
    );
    assert!(
        !message.contains("carried no output field"),
        "a present null must not be reported as a missing key: {message}"
    );
    Ok(())
}

#[test]
fn a_result_without_envelope_version_is_a_protocol_error_naming_it() -> TestResult {
    // The pre-envelope shape must never silently pass through.
    let Err(error) = run_result_to_outcome(&json!({ "result": "completed" })) else {
        return Err("a result without `envelope_version` must be refused".into());
    };
    assert!(matches!(error, HarnessError::Protocol { .. }));
    assert!(error.to_string().contains("envelope_version"));
    Ok(())
}

#[test]
fn a_result_without_stop_is_a_protocol_error_naming_it() -> TestResult {
    let Err(error) = run_result_to_outcome(&json!({ "envelope_version": 1, "output": "x" })) else {
        return Err("an envelope without `stop` must be refused".into());
    };
    assert!(matches!(error, HarnessError::Protocol { .. }));
    assert!(error.to_string().contains("`stop`"));
    Ok(())
}

#[test]
fn a_stop_without_a_string_reason_is_a_protocol_error() -> TestResult {
    let envelope = json!({ "envelope_version": 1, "stop": { "reason": 3 } });
    let Err(error) = run_result_to_outcome(&envelope) else {
        return Err("a `stop` without a string `reason` must be refused".into());
    };
    assert!(matches!(error, HarnessError::Protocol { .. }));
    assert!(error.to_string().contains("`reason`"));
    Ok(())
}
