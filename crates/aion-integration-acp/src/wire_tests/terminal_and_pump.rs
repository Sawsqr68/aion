use super::*;

#[tokio::test]
async fn the_agent_closing_the_channel_is_a_transport_error_not_a_hang() -> TestResult {
    // An agent that dies without answering must not leave `wait_result` waiting forever.
    let Wired {
        session,
        agent,
        prompt_id: _,
    } = wire_up(PermissionPolicy::Deny).await?;
    drop(agent);
    let error = within("taking the terminal result", session.wait_result())
        .await?
        .err()
        .ok_or("a closed channel yields no result".to_owned())?;
    assert!(
        matches!(error, HarnessError::Transport { .. }),
        "a dead agent is a transport failure, got {error:?}"
    );
    // The message must name what ACTUALLY happened. `StreamEnded` is the agent exiting without
    // answering, and it must not be reported with the same words as a pump that died holding an
    // answer, nor as a reader that hit an I/O fault.
    let message = error.to_string();
    assert!(
        message.contains("closed its stdout") && message.contains("without answering"),
        "the failure names the end-of-stream it observed: {message}"
    );
    Ok(())
}

// --- the driver contract, and the frames that must not break it ---

#[tokio::test]
async fn the_driver_contract_completes_against_an_agent_that_stays_alive() -> TestResult {
    // The exact shape of `aion-worker`'s `spawn_agent`: pump `events.next()` until the stream ends,
    // and only THEN take the result. The agent is a persistent stdio server — it answers
    // `session/prompt` and keeps running, its write half held open for the whole test — so the
    // stream can end only because the adapter closed it at the answer. Before that fix, every
    // successful turn hung here forever with a good result unread.
    let Wired {
        mut session,
        agent,
        prompt_id,
    } = wire_up(PermissionPolicy::Deny).await?;
    agent
        .send_notification(&session_update(&json!({
            "sessionUpdate": "agent_message_chunk",
            "content": { "type": "text", "text": "ablative" },
            "messageId": "m1"
        })))
        .await
        .map_err(|error| fail("the chunk must be writable", &error))?;
    agent
        .send_response(&prompt_response(prompt_id, "end_turn"))
        .await
        .map_err(|error| fail("the terminal response must be writable", &error))?;

    let mut events = AgentSession::events(&mut session);
    let mut collected = Vec::new();
    within("the driver's event loop", async {
        while let Some(event) = events.next().await {
            collected.push(event);
        }
    })
    .await?;
    drop(events);

    // The agent is STILL ALIVE, and provably so: it can still write to a channel the adapter has
    // not closed. The transcript ended because the ADAPTER ended it, which is the whole point.
    agent
        .send_notification(&JsonRpcNotification::new("agent/still_here", None))
        .await
        .map_err(|error| {
            fail(
                "the agent must still be writable after the transcript ended",
                &error,
            )
        })?;

    let last = collected
        .last()
        .ok_or("the turn must produce events".to_owned())?;
    assert!(
        matches!(
            last.kind,
            ActivityEventKind::Stop {
                reason: StopKind::EndTurn
            }
        ),
        "the LAST event is the terminal boundary, got {:?}",
        last.kind
    );

    let payload = within("taking the terminal result", session.wait_result())
        .await?
        .map_err(|error| fail("the driver contract must yield a result", &error))?;
    let decoded = payload
        .to_json()
        .map_err(|error| fail("the payload is JSON", &error))?;
    assert_eq!(
        decoded,
        json!({ "text": "ablative", "final_message": "ablative", "stop_reason": "end_turn", "session_id": SESSION })
    );
    drop(agent);
    Ok(())
}

#[tokio::test]
async fn the_terminal_response_is_recorded_verbatim_on_the_transcript() -> TestResult {
    // Every other frame gets a verbatim twin; the one that decides the result is no exception.
    // `PromptResponse._meta` lives nowhere else once the payload is formed.
    let Wired {
        mut session,
        agent,
        prompt_id,
    } = wire_up(PermissionPolicy::Deny).await?;
    agent
        .send_response(&JsonRpcResponse::success(
            prompt_id,
            json!({ "stopReason": "end_turn", "_meta": { "costUsd": 0.02 } }),
        ))
        .await
        .map_err(|error| fail("the terminal response must be writable", &error))?;
    let events = session.events();
    let collected = within("draining the transcript", events.collect::<Vec<_>>()).await?;

    let recorded = collected
        .iter()
        .find_map(|event| match &event.kind {
            ActivityEventKind::Raw { source, value }
                if source == protocol::METHOD_SESSION_PROMPT =>
            {
                Some(value.clone())
            }
            _ => None,
        })
        .ok_or_else(|| format!("the terminal frame must be recorded: {collected:?}"))?;
    assert_eq!(
        recorded["result"]["_meta"]["costUsd"],
        json!(0.02),
        "the agent's `_meta` survives: {recorded}"
    );
    drop(session);
    drop(agent);
    Ok(())
}

#[tokio::test]
async fn a_failed_terminal_frame_still_emits_a_terminal_boundary() -> TestResult {
    // A transcript that just stops mid-stream is not an honest record of a failed turn, and
    // `StopKind::Error` exists for exactly this. Nothing is invented by using it.
    let Wired {
        mut session,
        agent,
        prompt_id,
    } = wire_up(PermissionPolicy::Deny).await?;
    agent
        .send_response(&JsonRpcResponse::failure(
            prompt_id,
            aion_integrations::jsonrpc::JsonRpcError::new(-32603, "the model exploded"),
        ))
        .await
        .map_err(|error| fail("the terminal response must be writable", &error))?;
    let events = session.events();
    let collected = within("draining the transcript", events.collect::<Vec<_>>()).await?;

    let stop = collected
        .iter()
        .find_map(|event| match &event.kind {
            ActivityEventKind::Stop {
                reason: StopKind::Error { message },
            } => Some(message.clone()),
            _ => None,
        })
        .ok_or_else(|| format!("an error Response must still stop the run: {collected:?}"))?;
    assert!(
        stop.contains("-32603") && stop.contains("the model exploded"),
        "the boundary names the failure: {stop}"
    );
    // And the offending frame is recorded, not merely summarised into the stop message.
    assert!(
        has_raw(&collected, protocol::METHOD_SESSION_PROMPT),
        "the error frame must be on the transcript verbatim: {collected:?}"
    );
    drop(session);
    drop(agent);
    Ok(())
}

#[tokio::test]
async fn an_undecodable_terminal_result_still_emits_a_terminal_boundary() -> TestResult {
    let Wired {
        mut session,
        agent,
        prompt_id,
    } = wire_up(PermissionPolicy::Deny).await?;
    agent
        .send_response(&JsonRpcResponse::success(
            prompt_id,
            json!({ "stopReason": "teleported" }),
        ))
        .await
        .map_err(|error| fail("the terminal response must be writable", &error))?;
    let events = session.events();
    let collected = within("draining the transcript", events.collect::<Vec<_>>()).await?;

    let stop = collected
        .iter()
        .find_map(|event| match &event.kind {
            ActivityEventKind::Stop {
                reason: StopKind::Error { message },
            } => Some(message.clone()),
            _ => None,
        })
        .ok_or_else(|| format!("an undecodable result must still stop the run: {collected:?}"))?;
    assert!(
        stop.contains("PromptResponse"),
        "the boundary names why the frame could not be read: {stop}"
    );
    assert!(
        has_raw(&collected, protocol::METHOD_SESSION_PROMPT),
        "the undecodable frame must be recorded verbatim: {collected:?}"
    );
    drop(session);
    drop(agent);
    Ok(())
}

#[tokio::test]
async fn answering_a_flood_of_peer_requests_does_not_stall_the_read_loop() -> TestResult {
    // The deadlock: the transport holds the writer mutex across the write, so answering inline
    // stops the read loop as soon as the child's stdin fills — and the child cannot drain its stdin
    // while its stdout is not being read. A small duplex reproduces it exactly: this agent NEVER
    // reads its inbound pipe, so every answer after the first kilobyte blocks forever.
    let Wired {
        session,
        agent,
        prompt_id,
    } = wire_up_with_capacity(PermissionPolicy::Deny, 1_024).await?;

    let script = {
        let agent = Arc::clone(&agent);
        let prompt_id = prompt_id.clone();
        tokio::spawn(async move {
            for id in 0..40_u64 {
                agent
                    .send_request(&permission_request(JsonRpcId::number(9_000 + id)))
                    .await?;
            }
            agent
                .send_response(&prompt_response(prompt_id, "end_turn"))
                .await
        })
    };

    // With the write awaited inline this never resolves; the turn must still reach its result.
    let payload = within(
        "the terminal result behind a flood of unread answers",
        session.wait_result(),
    )
    .await?
    .map_err(|error| fail("the flood must not fail the turn", &error))?;
    assert_eq!(payload.content_type(), &ContentType::Json);

    script
        .await
        .map_err(|error| fail("the agent script task must not panic", &error))?
        .map_err(|error| fail("the agent script must complete", &error))?;
    drop(agent);
    Ok(())
}

#[tokio::test]
async fn a_notification_read_during_the_handshake_reaches_the_transcript() -> TestResult {
    // The handshake reads frames before the pump exists. Dropping one there broke the crate's
    // "nothing is dropped" rule, so they are carried forward and replayed as the transcript's first
    // events instead.
    let (client_io, agent_io) = duplex(DUPLEX_CAPACITY);
    let (client_read, client_write) = split(client_io);
    let (agent_read, agent_write) = split(agent_io);
    let connection = Arc::new(JsonRpcConnection::new(client_read, client_write));
    let agent = Arc::new(JsonRpcConnection::new(agent_read, agent_write));

    let script = {
        let agent = Arc::clone(&agent);
        tokio::spawn(async move {
            let initialize = expect_request(agent.as_ref(), protocol::METHOD_INITIALIZE).await?;
            // Sent BEFORE the initialize answer, so it is read while the handshake is blocked.
            agent
                .send_notification(&JsonRpcNotification::new(
                    "agent/booting",
                    Some(json!({ "pid": 4_242 })),
                ))
                .await?;
            agent
                .send_response(&JsonRpcResponse::success(
                    initialize.id,
                    json!({
                        "protocolVersion": protocol::PROTOCOL_VERSION.as_u16(),
                        "agentCapabilities": {},
                        "authMethods": [],
                        "agentInfo": { "name": "fake-acp-agent", "version": "0.0.0" }
                    }),
                ))
                .await?;
            let new_session = expect_request(agent.as_ref(), protocol::METHOD_SESSION_NEW).await?;
            agent
                .send_response(&JsonRpcResponse::success(
                    new_session.id,
                    json!({ "sessionId": SESSION }),
                ))
                .await?;
            expect_request(agent.as_ref(), protocol::METHOD_SESSION_PROMPT)
                .await
                .map(|_prompt| ())
        })
    };
    let opened = harness(PermissionPolicy::Deny)
        .connect(
            connection.as_ref(),
            PathBuf::from(WORKSPACE),
            "do the thing".to_owned(),
            None,
        )
        .await
        .map_err(|error| fail("the handshake must succeed", &error))?;
    script
        .await
        .map_err(|error| fail("the agent script task must not panic", &error))?
        .map_err(|error| fail("the agent script must complete", &error))?;
    assert_eq!(
        opened.deferred.len(),
        1,
        "the handshake-window notification must be carried forward, not dropped"
    );

    let prompt_id = opened.prompt_id.clone();
    let mut session = AcpSession::start(SessionStart {
        connection,
        session_id: opened.session_id,
        prompt_id: opened.prompt_id,
        policy: PermissionPolicy::Deny,
        identity: identity(),
        deferred: opened.deferred,
        child: None,
        exit_grace: EXIT_GRACE,
    });
    agent
        .send_response(&prompt_response(prompt_id, "end_turn"))
        .await
        .map_err(|error| fail("the terminal response must be writable", &error))?;
    let events = session.events();
    let collected = within("draining the transcript", events.collect::<Vec<_>>()).await?;
    let first = collected
        .first()
        .ok_or("the deferred notification must lead the transcript".to_owned())?;
    match &first.kind {
        ActivityEventKind::Raw { source, value } => {
            assert_eq!(source, "agent/booting");
            assert_eq!(value["pid"], json!(4_242));
        }
        other => return Err(format!("expected the deferred Raw first, got {other:?}")),
    }
    drop(session);
    drop(agent);
    Ok(())
}
