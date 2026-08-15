use super::*;

#[tokio::test]
async fn the_handshake_negotiates_version_one_and_opens_a_session() -> TestResult {
    // `wire_up` asserts the wire form of all three opening frames: `protocolVersion` as the integer
    // 1, `session/new` with an absolute cwd and a PRESENT `mcpServers`, and a `text` prompt block.
    let wired = wire_up(PermissionPolicy::Deny).await?;
    assert!(
        wired
            .session
            .capabilities()
            .supports_primitive(InterventionPrimitive::Cancel),
        "a negotiated ACP v1 session advertises cancel"
    );
    Ok(())
}

#[tokio::test]
async fn the_initialize_request_sends_the_integer_version_and_all_false_capabilities() -> TestResult
{
    // Read the client's actual `initialize` frame off the wire and assert its shape. ACP versions
    // are a uint16 — a semver string here would be silently unparseable to every real agent.
    let (client_io, agent_io) = duplex(DUPLEX_CAPACITY);
    let (client_read, client_write) = split(client_io);
    let (agent_read, agent_write) = split(agent_io);
    let connection = Arc::new(JsonRpcConnection::new(client_read, client_write));
    let agent = JsonRpcConnection::new(agent_read, agent_write);

    let client = tokio::spawn(async move {
        harness(PermissionPolicy::Deny)
            .connect(
                connection.as_ref(),
                PathBuf::from(WORKSPACE),
                "prompt".to_owned(),
                None,
            )
            .await
    });
    let request = expect_request(&agent, protocol::METHOD_INITIALIZE)
        .await
        .map_err(|error| fail("initialize must be the first frame", &error))?;
    let params = request
        .params
        .ok_or("initialize must carry params".to_owned())?;
    assert_eq!(
        params["protocolVersion"],
        json!(1),
        "the protocol version is the integer 1, never a string: {params}"
    );
    assert_eq!(
        params["clientCapabilities"]["fs"]["readTextFile"],
        json!(false)
    );
    assert_eq!(
        params["clientCapabilities"]["fs"]["writeTextFile"],
        json!(false)
    );
    assert_eq!(params["clientCapabilities"]["terminal"], json!(false));
    assert_eq!(
        params["clientInfo"]["name"],
        json!("aion-integration-acp"),
        "the client identifies itself honestly: {params}"
    );

    // The frame under test has been read. Close the channel and require the client to report the
    // broken handshake rather than hanging or reporting success against a dead peer.
    drop(agent);
    let outcome = client
        .await
        .map_err(|error| fail("the client task must not panic", &error))?;
    assert!(
        outcome.is_err(),
        "a handshake against a closed channel cannot succeed"
    );
    Ok(())
}

#[tokio::test]
async fn a_version_the_adapter_cannot_speak_is_refused_before_a_session_exists() -> TestResult {
    // The spec's rule: the agent may answer with its own latest version, and a client that cannot
    // speak it MUST disconnect. Refusing here — before `session/new` — is that disconnect.
    let (client_io, agent_io) = duplex(DUPLEX_CAPACITY);
    let (client_read, client_write) = split(client_io);
    let (agent_read, agent_write) = split(agent_io);
    let connection = Arc::new(JsonRpcConnection::new(client_read, client_write));
    let agent = Arc::new(JsonRpcConnection::new(agent_read, agent_write));

    let script = {
        let agent = Arc::clone(&agent);
        // 2 is a version this adapter does not implement.
        tokio::spawn(async move { agent_opens(agent.as_ref(), 2).await })
    };
    let error = harness(PermissionPolicy::Deny)
        .connect(
            connection.as_ref(),
            PathBuf::from(WORKSPACE),
            "prompt".to_owned(),
            None,
        )
        .await
        .err()
        .ok_or("a version mismatch must refuse the connection".to_owned())?;
    assert!(
        matches!(error, HarnessError::Protocol { .. }),
        "a version mismatch is a protocol error, got {error:?}"
    );
    let message = error.to_string();
    assert!(
        message.contains("version 1") && message.contains("version 2"),
        "the refusal names both versions: {message}"
    );
    script
        .await
        .map_err(|error| fail("the agent script task must not panic", &error))?
        .map_err(|error| fail("the agent script must complete", &error))?;
    Ok(())
}

// --- every session/update variant ---

#[tokio::test]
async fn an_agent_message_chunk_streams_a_delta_and_closes_a_durable_message() -> TestResult {
    let events = events_for(&json!({
        "sessionUpdate": "agent_message_chunk",
        "content": { "type": "text", "text": "the answer" },
        "messageId": "m1"
    }))
    .await?;
    let delta = events
        .iter()
        .find(|event| matches!(event.kind, ActivityEventKind::Delta { .. }))
        .ok_or("the chunk must stream a delta".to_owned())?;
    assert!(delta.ephemeral, "a per-token delta is never persisted");
    let message = events
        .iter()
        .find(|event| matches!(event.kind, ActivityEventKind::Message { .. }))
        .ok_or("the turn's end must close the message durably".to_owned())?;
    assert!(!message.ephemeral, "the completed message is durable");
    match &message.kind {
        ActivityEventKind::Message { role, text } => {
            assert_eq!(*role, MessageRole::Assistant);
            assert_eq!(text, "the answer");
        }
        other => return Err(format!("expected Message, got {other:?}")),
    }
    Ok(())
}

#[tokio::test]
async fn a_user_message_chunk_becomes_one_durable_user_message() -> TestResult {
    let events = events_for(&json!({
        "sessionUpdate": "user_message_chunk",
        "content": { "type": "text", "text": "do the thing" }
    }))
    .await?;
    let user_messages: Vec<&ActivityEvent> = events
        .iter()
        .filter(|event| {
            matches!(
                &event.kind,
                ActivityEventKind::Message {
                    role: MessageRole::User,
                    ..
                }
            )
        })
        .collect();
    let [only] = user_messages.as_slice() else {
        return Err(format!(
            "exactly one durable user message per turn, got {user_messages:?}"
        ));
    };
    assert!(!only.ephemeral, "a user turn is durable");
    match &only.kind {
        ActivityEventKind::Message { text, .. } => assert_eq!(text, "do the thing"),
        other => return Err(format!("expected Message, got {other:?}")),
    }
    Ok(())
}

#[tokio::test]
async fn a_multi_chunk_user_turn_is_still_one_durable_message() -> TestResult {
    // The defence against an echoing agent: N chunks must not become N durable events.
    let Wired {
        mut session,
        agent,
        prompt_id,
    } = wire_up(PermissionPolicy::Deny).await?;
    for fragment in ["do ", "the ", "thing"] {
        agent
            .send_notification(&session_update(&json!({
                "sessionUpdate": "user_message_chunk",
                "content": { "type": "text", "text": fragment },
                "messageId": "u1"
            })))
            .await
            .map_err(|error| fail("the chunk must be writable", &error))?;
    }
    agent
        .send_response(&prompt_response(prompt_id, "end_turn"))
        .await
        .map_err(|error| fail("the terminal response must be writable", &error))?;
    let events = session.events();
    let collected = within("draining the transcript", events.collect::<Vec<_>>()).await?;

    let texts: Vec<&str> = collected
        .iter()
        .filter_map(|event| match &event.kind {
            ActivityEventKind::Message {
                role: MessageRole::User,
                text,
            } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        texts,
        vec!["do the thing"],
        "three chunks, one durable message: {collected:?}"
    );
    drop(session);
    drop(agent);
    Ok(())
}

#[tokio::test]
async fn an_agent_thought_chunk_streams_live_and_survives_durably() -> TestResult {
    let events = events_for(&json!({
        "sessionUpdate": "agent_thought_chunk",
        "content": { "type": "text", "text": "considering" }
    }))
    .await?;
    let note = events
        .iter()
        .find(|event| {
            matches!(
                &event.kind,
                ActivityEventKind::Progress {
                    detail: ProgressDetail::Note { .. }
                }
            )
        })
        .ok_or("reasoning must reach the live transcript".to_owned())?;
    assert!(note.ephemeral, "the live reasoning stream is ephemeral");
    match &note.kind {
        ActivityEventKind::Progress {
            detail: ProgressDetail::Note { text },
        } => assert_eq!(text, "considering"),
        other => return Err(format!("expected Progress::Note, got {other:?}")),
    }

    // The ruling: reasoning that vanishes at end-of-run defeats durable observability.
    let durable = events
        .iter()
        .find(|event| {
            matches!(
                &event.kind,
                ActivityEventKind::Message {
                    role: MessageRole::Assistant,
                    ..
                }
            )
        })
        .ok_or_else(|| format!("reasoning must also reach the DURABLE record: {events:?}"))?;
    assert!(!durable.ephemeral, "the accumulated reasoning is durable");
    match &durable.kind {
        ActivityEventKind::Message { text, .. } => assert_eq!(text, "considering"),
        other => return Err(format!("expected Message, got {other:?}")),
    }
    Ok(())
}

#[tokio::test]
async fn reasoning_never_becomes_the_activitys_result() -> TestResult {
    // The one distinction the neutral vocabulary cannot express in a role, and the one that matters:
    // the payload's text is the answer, never the thinking that preceded it.
    let Wired {
        session,
        agent,
        prompt_id,
    } = wire_up(PermissionPolicy::Deny).await?;
    agent
        .send_notification(&session_update(&json!({
            "sessionUpdate": "agent_thought_chunk",
            "content": { "type": "text", "text": "maybe I should refuse" }
        })))
        .await
        .map_err(|error| fail("the thought chunk must be writable", &error))?;
    agent
        .send_notification(&session_update(&json!({
            "sessionUpdate": "agent_message_chunk",
            "content": { "type": "text", "text": "42" },
            "messageId": "m1"
        })))
        .await
        .map_err(|error| fail("the message chunk must be writable", &error))?;
    agent
        .send_response(&prompt_response(prompt_id, "end_turn"))
        .await
        .map_err(|error| fail("the terminal response must be writable", &error))?;

    let payload = within("taking the terminal result", session.wait_result())
        .await?
        .map_err(|error| fail("end_turn is a completed run", &error))?;
    let decoded = payload
        .to_json()
        .map_err(|error| fail("the payload is JSON", &error))?;
    assert_eq!(
        decoded,
        json!({ "text": "42", "final_message": "42", "stop_reason": "end_turn", "session_id": SESSION })
    );
    drop(agent);
    Ok(())
}

#[tokio::test]
async fn a_tool_call_and_its_completion_become_a_call_and_a_result() -> TestResult {
    let created = events_for(&json!({
        "sessionUpdate": "tool_call",
        "toolCallId": "t1",
        "title": "Read src/main.rs",
        "kind": "read",
        "status": "pending",
        "rawInput": { "path": "src/main.rs" }
    }))
    .await?;
    let call = created
        .iter()
        .find_map(|event| match &event.kind {
            ActivityEventKind::ToolCall {
                tool,
                call_id,
                input,
            } => Some((tool.clone(), call_id.clone(), input.clone())),
            _ => None,
        })
        .ok_or("a tool call must be classified".to_owned())?;
    assert_eq!(call.0, "Read src/main.rs");
    assert_eq!(call.1, "t1");
    assert_eq!(call.2, json!({ "path": "src/main.rs" }));
    assert!(
        has_raw(&created, "session/update/tool_call"),
        "the verbatim frame preserves `kind`, which the neutral ToolCall cannot carry: {created:?}"
    );

    let updated = events_for(&json!({
        "sessionUpdate": "tool_call_update",
        "toolCallId": "t1",
        "status": "completed",
        "rawOutput": { "lines": 12 }
    }))
    .await?;
    let result = updated
        .iter()
        .find_map(|event| match &event.kind {
            ActivityEventKind::ToolResult {
                call_id,
                output,
                is_error,
            } => Some((call_id.clone(), output.clone(), *is_error)),
            _ => None,
        })
        .ok_or("a completed update must be classified as a result".to_owned())?;
    assert_eq!(result.0, "t1");
    assert_eq!(result.1, json!({ "lines": 12 }));
    assert!(!result.2);
    assert!(has_raw(&updated, "session/update/tool_call_update"));
    Ok(())
}

#[tokio::test]
async fn a_failed_tool_call_update_is_an_error_result() -> TestResult {
    let events = events_for(&json!({
        "sessionUpdate": "tool_call_update",
        "toolCallId": "t2",
        "status": "failed"
    }))
    .await?;
    let is_error = events.iter().any(|event| {
        matches!(
            &event.kind,
            ActivityEventKind::ToolResult { is_error: true, .. }
        )
    });
    assert!(is_error, "a failed status is an error result: {events:?}");
    Ok(())
}

#[tokio::test]
async fn the_six_remaining_update_variants_each_reach_the_transcript() -> TestResult {
    // Table-driven, but every row genuinely goes over the wire as the ACP frame an agent sends.
    // Losing an update silently is not acceptable: each must produce a transcript entry.
    let cases: Vec<(Value, &str)> = vec![
        (
            json!({
                "sessionUpdate": "plan",
                "entries": [
                    { "content": "read the code", "priority": "high", "status": "pending" },
                    { "content": "write the fix", "priority": "medium", "status": "in_progress" }
                ]
            }),
            "session/update/plan",
        ),
        (
            json!({
                "sessionUpdate": "available_commands_update",
                "availableCommands": [{ "name": "create_plan", "description": "draft a plan" }]
            }),
            "session/update/available_commands_update",
        ),
        (
            json!({ "sessionUpdate": "current_mode_update", "currentModeId": "architect" }),
            "session/update/current_mode_update",
        ),
        (
            json!({ "sessionUpdate": "config_option_update", "configOptions": [] }),
            "session/update/config_option_update",
        ),
        (
            json!({ "sessionUpdate": "session_info_update", "title": "investigating" }),
            "session/update/session_info_update",
        ),
        (
            json!({ "sessionUpdate": "usage_update", "used": 4_096, "size": 200_000 }),
            "session/update/usage_update",
        ),
    ];
    for (update, source) in cases {
        let events = events_for(&update).await?;
        assert!(
            has_raw(&events, source),
            "`{update}` must reach the transcript under `{source}`, got {events:?}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn a_plan_reaches_the_transcript_with_every_entry() -> TestResult {
    // A plan is a COMPLETE replacement, so every entry has to survive or an operator's plan view
    // goes stale without anything reporting it.
    let events = events_for(&json!({
        "sessionUpdate": "plan",
        "entries": [
            { "content": "one", "priority": "high", "status": "completed" },
            { "content": "two", "priority": "low", "status": "pending" }
        ]
    }))
    .await?;
    let entries = events
        .iter()
        .find_map(|event| match &event.kind {
            ActivityEventKind::Raw { source, value } if source == "session/update/plan" => {
                value["entries"].as_array().cloned()
            }
            _ => None,
        })
        .ok_or("the plan entries must survive".to_owned())?;
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0]["content"], json!("one"));
    assert_eq!(entries[1]["status"], json!("pending"));
    Ok(())
}

#[tokio::test]
async fn an_unrecognised_update_variant_still_reaches_the_transcript() -> TestResult {
    // Forward compatibility: a variant from a future protocol release must surface, not vanish and
    // not fail the run.
    let events = events_for(&json!({
        "sessionUpdate": "teleport_update",
        "destination": "mars"
    }))
    .await?;
    assert!(
        has_raw(&events, "session/update/undecodable"),
        "an unknown variant must be recorded verbatim: {events:?}"
    );
    Ok(())
}

// --- the five stop reasons ---
