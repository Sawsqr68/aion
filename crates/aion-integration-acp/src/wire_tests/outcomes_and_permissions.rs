use super::*;

#[tokio::test]
async fn end_turn_is_the_completed_run_and_carries_the_agents_text() -> TestResult {
    let Wired {
        mut session,
        agent,
        prompt_id,
    } = wire_up(PermissionPolicy::Deny).await?;
    agent
        .send_notification(&session_update(&json!({
            "sessionUpdate": "agent_message_chunk",
            "content": { "type": "text", "text": "42" },
            "messageId": "m1"
        })))
        .await
        .map_err(|error| fail("the chunk must be writable", &error))?;
    agent
        .send_response(&prompt_response(prompt_id, "end_turn"))
        .await
        .map_err(|error| fail("the terminal response must be writable", &error))?;

    // Drain the transcript to END-OF-STREAM, exactly as the platform's driver does. Breaking out at
    // the Stop event would hide the defect this shape exists to catch: the agent is still alive, so
    // the stream can only end because the adapter closed it.
    let events = session.events();
    let collected = within("draining the transcript", events.collect::<Vec<_>>()).await?;
    let saw_stop = collected.iter().any(|event| {
        matches!(
            event.kind,
            ActivityEventKind::Stop {
                reason: StopKind::EndTurn
            }
        )
    });
    assert!(saw_stop, "a completed turn emits a terminal Stop event");

    let payload = within("taking the terminal result", session.wait_result())
        .await?
        .map_err(|error| fail("end_turn is a completed run", &error))?;
    assert_eq!(payload.content_type(), &ContentType::Json);
    let decoded = payload
        .to_json()
        .map_err(|error| fail("the payload is JSON", &error))?;
    assert_eq!(
        decoded,
        json!({ "text": "42", "final_message": "42", "stop_reason": "end_turn", "session_id": SESSION })
    );
    Ok(())
}

#[tokio::test]
async fn refusal_and_the_two_limits_are_failures_naming_themselves() -> TestResult {
    for reason in ["refusal", "max_tokens", "max_turn_requests"] {
        let Wired {
            session,
            agent,
            prompt_id,
        } = wire_up(PermissionPolicy::Deny).await?;
        agent
            .send_response(&prompt_response(prompt_id, reason))
            .await
            .map_err(|error| fail("the terminal response must be writable", &error))?;
        let error =
            session.wait_result().await.err().ok_or_else(|| {
                format!("`{reason}` must never be reported as a completed result")
            })?;
        let expected_class = if reason == "refusal" {
            matches!(error, HarnessError::PolicyRefused { .. })
        } else {
            matches!(error, HarnessError::Harness { .. })
        };
        assert!(
            expected_class,
            "`{reason}` has its adapter-local class, got {error:?}"
        );
        let message = error.to_string();
        assert!(
            message.contains(reason),
            "the failure names `{reason}`: {message}"
        );
        assert!(
            !message.contains(protocol::CANCELLED_DETAIL_PREFIX),
            "`{reason}` is not a cancellation: {message}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn a_cancelled_stop_reason_is_distinguishable_from_a_failure() -> TestResult {
    let Wired {
        session,
        agent,
        prompt_id,
    } = wire_up(PermissionPolicy::Deny).await?;
    agent
        .send_response(&prompt_response(prompt_id, "cancelled"))
        .await
        .map_err(|error| fail("the terminal response must be writable", &error))?;
    let error = session
        .wait_result()
        .await
        .err()
        .ok_or("a cancelled turn is not a completed result".to_owned())?;
    assert!(
        error
            .to_string()
            .contains(protocol::CANCELLED_DETAIL_PREFIX),
        "a cancellation carries the documented prefix: {error}"
    );
    Ok(())
}

// --- cancellation ---

#[tokio::test]
async fn cancelling_sends_session_cancel_and_ends_the_turn_as_cancelled() -> TestResult {
    let Wired {
        session,
        agent,
        prompt_id,
    } = wire_up(PermissionPolicy::AllowOnce).await?;

    session
        .intervene(cancel_command())
        .await
        .map_err(|error| fail("cancel is an advertised primitive", &error))?;

    // The agent observes the notification — a NOTIFICATION, never a request: it carries no id, so it
    // can never be mistaken for something awaiting a reply.
    let message = agent
        .recv()
        .await
        .map_err(|error| fail("the cancel frame must be readable", &error))?
        .ok_or("the cancel frame must arrive".to_owned())?;
    match message {
        IncomingMessage::Notification(note) => {
            assert_eq!(note.method, protocol::METHOD_SESSION_CANCEL);
            let params = note
                .params
                .ok_or("session/cancel carries params".to_owned())?;
            assert_eq!(params["sessionId"], json!(SESSION));
        }
        other => {
            return Err(format!(
                "session/cancel must be a notification, got {other:?}"
            ));
        }
    }

    // The spec: the agent MUST then answer the still-open prompt with `cancelled`, never an error.
    agent
        .send_response(&prompt_response(prompt_id, "cancelled"))
        .await
        .map_err(|error| fail("the terminal response must be writable", &error))?;
    let error = session
        .wait_result()
        .await
        .err()
        .ok_or("a cancelled turn is not a completed result".to_owned())?;
    assert!(
        error
            .to_string()
            .contains(protocol::CANCELLED_DETAIL_PREFIX),
        "the terminal outcome is a cancellation: {error}"
    );
    Ok(())
}

#[tokio::test]
async fn an_unadvertised_primitive_is_gated_before_anything_is_written() -> TestResult {
    let Wired {
        session,
        agent,
        prompt_id,
    } = wire_up(PermissionPolicy::Deny).await?;
    let mut command = cancel_command();
    command.kind = InterventionKind::InjectMessage {
        text: "steer".to_owned(),
        priority: aion_core::InjectPriority::Interrupt,
    };
    let error = session
        .intervene(command)
        .await
        .err()
        .ok_or("ACP cannot inject into a running turn".to_owned())?;
    assert!(
        matches!(error, HarnessError::CapabilityNotSupported { .. }),
        "an unadvertised primitive is capability-gated, got {error:?}"
    );
    assert_eq!(
        error.to_string(),
        "capability not supported: inject_message",
        "the rejection names the primitive in the neutral vocabulary"
    );

    // Nothing was written: ending the turn is the only frame the agent ever sees, and the run still
    // completes normally.
    close_turn(session, &agent, prompt_id, "end_turn").await
}

#[tokio::test]
async fn a_permission_request_arriving_after_a_cancel_is_answered_cancelled() -> TestResult {
    // The other half of the client's cancellation obligation: a client that cancels MUST answer
    // every pending permission request with the `cancelled` outcome, whatever its policy would
    // otherwise have chosen.
    let Wired {
        session,
        agent,
        prompt_id,
    } = wire_up(PermissionPolicy::AllowOnce).await?;
    session
        .intervene(cancel_command())
        .await
        .map_err(|error| fail("cancel is an advertised primitive", &error))?;
    // Consume the cancel notification so the next agent-side read is the permission answer.
    let cancel = agent
        .recv()
        .await
        .map_err(|error| fail("the cancel frame must be readable", &error))?
        .ok_or("the cancel frame must arrive".to_owned())?;
    assert!(
        matches!(&cancel, IncomingMessage::Notification(note)
            if note.method == protocol::METHOD_SESSION_CANCEL),
        "the first frame after intervene is session/cancel, got {cancel:?}"
    );

    let outcome = ask_permission(&agent, 5_001).await?;
    assert_eq!(
        outcome["outcome"],
        json!("cancelled"),
        "an allow-once policy is overridden by the cancellation: {outcome}"
    );

    close_turn(session, &agent, prompt_id, "cancelled").await
}

// --- permission requests ---

/// Sends one `session/request_permission` from the fake agent and returns the `outcome` object the
/// adapter answered with.
async fn ask_permission(agent: &Wire, request_id: u64) -> TestResult<Value> {
    let request = permission_request(JsonRpcId::number(request_id));
    agent
        .send_request(&request)
        .await
        .map_err(|error| fail("the permission request must be writable", &error))?;
    let message = agent
        .recv()
        .await
        .map_err(|error| fail("the answer must be readable", &error))?
        .ok_or("a permission request must ALWAYS be answered".to_owned())?;
    match message {
        IncomingMessage::Response(response) => {
            if response.id != JsonRpcId::number(request_id) {
                return Err(format!(
                    "the answer must be id-matched, got {:?}",
                    response.id
                ));
            }
            let result = response.result.ok_or_else(|| {
                format!("the answer must carry a result, got {:?}", response.error)
            })?;
            Ok(result["outcome"].clone())
        }
        other => Err(format!("expected a response, got {other:?}")),
    }
}

#[tokio::test]
async fn the_deny_policy_selects_a_reject_option_and_records_the_decision() -> TestResult {
    let Wired {
        mut session,
        agent,
        prompt_id,
    } = wire_up(PermissionPolicy::Deny).await?;
    let mut events = session.events();

    let outcome = ask_permission(&agent, 4_001).await?;
    assert_eq!(outcome["outcome"], json!("selected"));
    assert_eq!(
        outcome["optionId"],
        json!("deny-1"),
        "the deny policy picks the reject option: {outcome}"
    );

    // Both the ask and the decision must be on the transcript: a policy answer an operator cannot
    // audit after the fact is exactly what this event pair exists to prevent.
    let asked = next_raw(&mut events, protocol::METHOD_SESSION_REQUEST_PERMISSION).await?;
    assert_eq!(asked["toolCall"]["toolCallId"], json!("t1"));
    let decided = next_raw(
        &mut events,
        &format!("{}/decision", protocol::METHOD_SESSION_REQUEST_PERMISSION),
    )
    .await?;
    assert_eq!(decided["policy"], json!("deny"));
    assert_eq!(decided["outcome"]["optionId"], json!("deny-1"));
    assert_eq!(decided["toolCallId"], json!("t1"));
    drop(events);

    close_turn(session, &agent, prompt_id, "end_turn").await
}

#[tokio::test]
async fn the_allow_once_policy_selects_allow_once_and_records_the_decision() -> TestResult {
    let Wired {
        mut session,
        agent,
        prompt_id,
    } = wire_up(PermissionPolicy::AllowOnce).await?;
    let mut events = session.events();

    let outcome = ask_permission(&agent, 4_002).await?;
    assert_eq!(
        outcome["optionId"],
        json!("allow-1"),
        "the allow-once policy picks allow_once, never allow_always: {outcome}"
    );

    let asked = next_raw(&mut events, protocol::METHOD_SESSION_REQUEST_PERMISSION).await?;
    assert_eq!(asked["toolCall"]["toolCallId"], json!("t1"));
    let decided = next_raw(
        &mut events,
        &format!("{}/decision", protocol::METHOD_SESSION_REQUEST_PERMISSION),
    )
    .await?;
    assert_eq!(decided["policy"], json!("allow_once"));
    assert_eq!(decided["outcome"]["optionId"], json!("allow-1"));
    drop(events);

    close_turn(session, &agent, prompt_id, "end_turn").await
}

/// Reads the event stream until a `Raw` event with exactly `source` arrives, returning its value.
async fn next_raw(
    events: &mut futures::stream::BoxStream<'static, ActivityEvent>,
    source: &str,
) -> TestResult<Value> {
    while let Some(event) = events.next().await {
        if let ActivityEventKind::Raw { source: got, value } = event.kind
            && got == source
        {
            return Ok(value);
        }
    }
    Err(format!("no `{source}` event reached the transcript"))
}

// --- unadvertised peer requests ---

#[tokio::test]
async fn an_inbound_fs_read_gets_an_error_response_rather_than_silence() -> TestResult {
    // The whole point: we advertise `fs.readTextFile: false`, but a non-conforming agent may ask
    // anyway, and an unanswered request blocks it forever. It must get an ANSWER.
    let Wired {
        mut session,
        agent,
        prompt_id,
    } = wire_up(PermissionPolicy::Deny).await?;
    let mut events = session.events();

    let request = JsonRpcRequest::new(
        JsonRpcId::number(7_001),
        protocol::METHOD_FS_READ_TEXT_FILE,
        Some(json!({ "sessionId": SESSION, "path": "/etc/hosts" })),
    );
    agent
        .send_request(&request)
        .await
        .map_err(|error| fail("the fs request must be writable", &error))?;
    let message = agent
        .recv()
        .await
        .map_err(|error| fail("the refusal must be readable", &error))?
        .ok_or("an unadvertised request must still be answered".to_owned())?;
    match message {
        IncomingMessage::Response(response) => {
            assert_eq!(response.id, JsonRpcId::number(7_001));
            let error = response
                .error
                .ok_or("the refusal is an error response".to_owned())?;
            assert_eq!(
                error.code,
                protocol::method_not_found_code(),
                "an unadvertised capability is `method not found`"
            );
            assert!(
                error.message.contains(protocol::METHOD_FS_READ_TEXT_FILE),
                "the refusal names the method it refused: {}",
                error.message
            );
        }
        other => return Err(format!("expected an error response, got {other:?}")),
    }

    // And the refusal is on the transcript, because it changes what the agent could do.
    let recorded = next_raw(
        &mut events,
        &format!("refused/{}", protocol::METHOD_FS_READ_TEXT_FILE),
    )
    .await?;
    assert_eq!(recorded["params"]["path"], json!("/etc/hosts"));
    drop(events);

    // The run is still alive: the turn completes normally after the refusal.
    agent
        .send_response(&prompt_response(prompt_id, "end_turn"))
        .await
        .map_err(|error| fail("the terminal response must be writable", &error))?;
    let payload = session
        .wait_result()
        .await
        .map_err(|error| fail("a refused peer request must not kill the run", &error))?;
    assert_eq!(payload.content_type(), &ContentType::Json);
    Ok(())
}

#[tokio::test]
async fn an_inbound_terminal_create_is_refused_the_same_way() -> TestResult {
    let Wired {
        session,
        agent,
        prompt_id,
    } = wire_up(PermissionPolicy::Deny).await?;
    let request = JsonRpcRequest::new(
        JsonRpcId::number(7_002),
        "terminal/create",
        Some(json!({ "sessionId": SESSION, "command": "ls" })),
    );
    agent
        .send_request(&request)
        .await
        .map_err(|error| fail("the terminal request must be writable", &error))?;
    let message = agent
        .recv()
        .await
        .map_err(|error| fail("the refusal must be readable", &error))?
        .ok_or("an unadvertised request must still be answered".to_owned())?;
    let refused = matches!(
        message,
        IncomingMessage::Response(ref response)
            if response.error.as_ref().map(|error| error.code)
                == Some(protocol::method_not_found_code())
    );
    assert!(refused, "terminal/create must be refused, got {message:?}");

    // The run survives the refusal and completes normally.
    close_turn(session, &agent, prompt_id, "end_turn").await
}
