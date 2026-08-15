use super::*;

// No typed send can produce a non-JSON-RPC line, so these tests hold the fake agent's write half
// RAW and write every frame as a literal line.

/// The fake agent's reader when the test writes its own frames.
///
/// The writer is a [`Sink`] because the real writing happens on the raw half; the connection exists
/// only to classify the client's outbound frames.
type RawWire = JsonRpcConnection<ReadHalf<DuplexStream>, Sink>;

/// A live pairing where the agent's stdout is written by hand.
struct RawWired {
    session: Session,
    reader: RawWire,
    writer: WriteHalf<DuplexStream>,
    prompt_id: JsonRpcId,
}

/// Writes one line verbatim on the agent's stdout, newline-terminated.
async fn write_line(writer: &mut WriteHalf<DuplexStream>, line: &str) -> TestResult {
    writer
        .write_all(line.as_bytes())
        .await
        .map_err(|error| fail("the raw line must be writable", &error))?;
    writer
        .write_all(b"\n")
        .await
        .map_err(|error| fail("the frame terminator must be writable", &error))?;
    writer
        .flush()
        .await
        .map_err(|error| fail("the raw line must flush", &error))
}

/// The literal JSON of a JSON-RPC id, for embedding in a hand-written frame.
fn id_json(id: &JsonRpcId) -> TestResult<String> {
    serde_json::to_string(id).map_err(|error| fail("a request id must encode", &error))
}

/// Opens a turn against an agent whose stdout the test writes itself.
async fn wire_up_raw(policy: PermissionPolicy) -> TestResult<RawWired> {
    let (client_io, agent_io) = duplex(DUPLEX_CAPACITY);
    let (client_read, client_write) = split(client_io);
    let (agent_read, agent_write) = split(agent_io);
    let connection = Arc::new(JsonRpcConnection::new(client_read, client_write));
    let reader: RawWire = JsonRpcConnection::new(agent_read, tokio::io::sink());

    let script = tokio::spawn(async move {
        let mut writer = agent_write;
        let initialize = expect_request(&reader, protocol::METHOD_INITIALIZE)
            .await
            .map_err(|error| fail("initialize must arrive", &error))?;
        write_line(
            &mut writer,
            &format!(
                r#"{{"jsonrpc":"2.0","id":{},"result":{{"protocolVersion":1,"agentCapabilities":{{}},"authMethods":[],"agentInfo":{{"name":"raw-agent","version":"0.0.0"}}}}}}"#,
                id_json(&initialize.id)?
            ),
        )
        .await?;
        let new_session = expect_request(&reader, protocol::METHOD_SESSION_NEW)
            .await
            .map_err(|error| fail("session/new must arrive", &error))?;
        write_line(
            &mut writer,
            &format!(
                r#"{{"jsonrpc":"2.0","id":{},"result":{{"sessionId":"{SESSION}"}}}}"#,
                id_json(&new_session.id)?
            ),
        )
        .await?;
        expect_request(&reader, protocol::METHOD_SESSION_PROMPT)
            .await
            .map_err(|error| fail("session/prompt must arrive", &error))?;
        Ok::<_, String>((reader, writer))
    });

    let opened = harness(policy)
        .connect(
            connection.as_ref(),
            PathBuf::from(WORKSPACE),
            "do the thing".to_owned(),
            None,
        )
        .await
        .map_err(|error| fail("the handshake must succeed", &error))?;
    let (reader, writer) = script
        .await
        .map_err(|error| fail("the agent script task must not panic", &error))??;

    let prompt_id = opened.prompt_id.clone();
    let session = AcpSession::start(SessionStart {
        connection,
        session_id: opened.session_id,
        prompt_id: opened.prompt_id,
        policy,
        identity: identity(),
        deferred: opened.deferred,
        child: None,
        exit_grace: EXIT_GRACE,
    });
    Ok(RawWired {
        session,
        reader,
        writer,
        prompt_id,
    })
}

#[tokio::test]
async fn a_line_that_is_not_a_jsonrpc_frame_does_not_destroy_the_turn() -> TestResult {
    // The exact shape a `node`/`npx`-launched agent emits: a debugger banner on stdout between a
    // chunk and the terminal Response. It used to end the pump, throw away an answer that had
    // ALREADY been received, emit no Stop, and report "the agent closed the channel" — a cause that
    // had not happened.
    let RawWired {
        mut session,
        reader,
        mut writer,
        prompt_id,
    } = wire_up_raw(PermissionPolicy::Deny).await?;

    write_line(
        &mut writer,
        &format!(
            r#"{{"jsonrpc":"2.0","method":"session/update","params":{{"sessionId":"{SESSION}","update":{{"sessionUpdate":"agent_message_chunk","content":{{"type":"text","text":"42"}},"messageId":"m1"}}}}}}"#
        ),
    )
    .await?;
    // Not JSON at all.
    write_line(&mut writer, "Debugger listening on ws://127.0.0.1:9229").await?;
    // Valid JSON, but not any of the three JSON-RPC shapes.
    write_line(&mut writer, r#"{"hello":"world"}"#).await?;
    write_line(
        &mut writer,
        &format!(
            r#"{{"jsonrpc":"2.0","id":{},"result":{{"stopReason":"end_turn"}}}}"#,
            id_json(&prompt_id)?
        ),
    )
    .await?;

    let events = session.events();
    let collected = within("draining the transcript", events.collect::<Vec<_>>()).await?;
    assert!(
        has_raw(&collected, "acp/unreadable_frame"),
        "the rejected line must be recorded, not swallowed: {collected:?}"
    );
    let unreadable = collected
        .iter()
        .filter(|event| {
            matches!(&event.kind, ActivityEventKind::Raw { source, .. }
                if source == "acp/unreadable_frame")
        })
        .count();
    assert_eq!(unreadable, 2, "both bad lines are recorded: {collected:?}");
    assert!(
        collected.iter().any(|event| matches!(
            event.kind,
            ActivityEventKind::Stop {
                reason: StopKind::EndTurn
            }
        )),
        "the turn still reaches its terminal boundary: {collected:?}"
    );

    let payload = within("taking the terminal result", session.wait_result())
        .await?
        .map_err(|error| fail("a bad line must not fail a completed turn", &error))?;
    let decoded = payload
        .to_json()
        .map_err(|error| fail("the payload is JSON", &error))?;
    assert_eq!(
        decoded,
        json!({ "text": "42", "final_message": "42", "stop_reason": "end_turn", "session_id": SESSION }),
        "the answer received before the bad line survives it"
    );
    drop(reader);
    drop(writer);
    Ok(())
}

// --- session/load: the resume lane (#205) ---

/// The prior session id the resume tests carry into `connect`.
const PRIOR_SESSION: &str = "sess-prior";

/// The fake agent's opening conversation for a RESUMED turn: advertises `loadSession: true`,
/// verifies and answers `session/load` for [`PRIOR_SESSION`] — replaying one history update
/// mid-load, exactly where a real agent streams them — and consumes the prompt, asserting it is
/// scoped to the LOADED session, not a fresh one.
async fn agent_opens_with_load(agent: &Wire) -> Result<(), HarnessError> {
    let initialize = expect_request(agent, protocol::METHOD_INITIALIZE).await?;
    agent
        .send_response(&JsonRpcResponse::success(
            initialize.id,
            json!({
                "protocolVersion": protocol::PROTOCOL_VERSION.as_u16(),
                "agentCapabilities": { "loadSession": true },
                "authMethods": [],
                "agentInfo": { "name": "fake-acp-agent", "version": "0.0.0" }
            }),
        ))
        .await?;
    // THE arm under test: the very next frame must be `session/load` — a `session/new` here
    // would be the silent-fresh-session defect, and `expect_request` fails the script on it.
    let load = expect_request(agent, protocol::METHOD_SESSION_LOAD).await?;
    let params = load
        .params
        .as_ref()
        .ok_or_else(|| HarnessError::protocol("session/load carried no params"))?;
    if params["sessionId"] != json!(PRIOR_SESSION) {
        return Err(HarnessError::protocol(format!(
            "session/load must name the job's prior session, got {}",
            params["sessionId"]
        )));
    }
    if params["cwd"] != json!(WORKSPACE) {
        return Err(HarnessError::protocol(format!(
            "session/load must send the configured absolute cwd, got {}",
            params["cwd"]
        )));
    }
    if params.get("mcpServers") != Some(&json!([])) {
        return Err(HarnessError::protocol(format!(
            "session/load must always send `mcpServers`, got {:?}",
            params.get("mcpServers")
        )));
    }
    // Replayed history streams as `session/update` notifications DURING the load, before the
    // response — the adapter must defer it for the pump, not lose it.
    agent
        .send_notification(&JsonRpcNotification::new(
            protocol::METHOD_SESSION_UPDATE,
            Some(json!({
                "sessionId": PRIOR_SESSION,
                "update": {
                    "sessionUpdate": "agent_message_chunk",
                    "content": { "type": "text", "text": "replayed history" },
                },
            })),
        ))
        .await?;
    agent
        .send_response(&JsonRpcResponse::success(load.id, json!({})))
        .await?;
    let prompt = expect_request(agent, protocol::METHOD_SESSION_PROMPT).await?;
    let prompt_params = prompt
        .params
        .as_ref()
        .ok_or_else(|| HarnessError::protocol("session/prompt carried no params"))?;
    if prompt_params["sessionId"] != json!(PRIOR_SESSION) {
        return Err(HarnessError::protocol(format!(
            "the prompt must be scoped to the LOADED session, got {}",
            prompt_params["sessionId"]
        )));
    }
    Ok(())
}

#[tokio::test]
async fn a_prior_session_and_a_load_capable_agent_resume_via_session_load() -> TestResult {
    let (client_io, agent_io) = duplex(DUPLEX_CAPACITY);
    let (client_read, client_write) = split(client_io);
    let (agent_read, agent_write) = split(agent_io);
    let connection = Arc::new(JsonRpcConnection::new(client_read, client_write));
    let agent = Arc::new(JsonRpcConnection::new(agent_read, agent_write));

    let script = {
        let agent = Arc::clone(&agent);
        tokio::spawn(async move { agent_opens_with_load(agent.as_ref()).await })
    };
    let opened = within(
        "the resume handshake",
        harness(PermissionPolicy::Deny).connect(
            connection.as_ref(),
            PathBuf::from(WORKSPACE),
            "carry on".to_owned(),
            Some(agent_client_protocol_schema::v1::SessionId::from(
                PRIOR_SESSION.to_owned(),
            )),
        ),
    )
    .await?
    .map_err(|error| fail("the resume handshake must succeed", &error))?;
    within("the agent script", script)
        .await?
        .map_err(|error| fail("the agent script task must not panic", &error))?
        .map_err(|error| fail("the agent script must complete", &error))?;

    // The turn runs IN the loaded session: its id — not a fresh mint — is what the session (and
    // ultimately the outcome record's `session_id`) carries.
    assert_eq!(
        opened.session_id.to_string(),
        PRIOR_SESSION,
        "the opened turn carries the loaded session's id"
    );
    // The history the agent replayed during the load was deferred for the pump, not dropped.
    assert!(
        opened.deferred.iter().any(|note| {
            note.method == protocol::METHOD_SESSION_UPDATE
                && note.params.as_ref().is_some_and(|params| {
                    params["update"]["content"]["text"] == json!("replayed history")
                })
        }),
        "the replayed session/update is deferred for the pump: {:?}",
        opened.deferred
    );
    Ok(())
}

#[tokio::test]
async fn asking_to_resume_an_agent_that_cannot_load_refuses_and_mints_nothing() -> TestResult {
    // The existing opening script advertises `loadSession: false` and, on an aborted handshake,
    // stops after answering `initialize` — exactly the conversation this refusal produces.
    let (client_io, agent_io) = duplex(DUPLEX_CAPACITY);
    let (client_read, client_write) = split(client_io);
    let (agent_read, agent_write) = split(agent_io);
    let connection = Arc::new(JsonRpcConnection::new(client_read, client_write));
    let agent = Arc::new(JsonRpcConnection::new(agent_read, agent_write));

    let script = {
        let agent = Arc::clone(&agent);
        tokio::spawn(async move {
            let initialize = expect_request(agent.as_ref(), protocol::METHOD_INITIALIZE).await?;
            agent
                .send_response(&JsonRpcResponse::success(
                    initialize.id,
                    json!({
                        "protocolVersion": protocol::PROTOCOL_VERSION.as_u16(),
                        "agentCapabilities": { "loadSession": false },
                        "authMethods": [],
                        "agentInfo": { "name": "fake-acp-agent", "version": "0.0.0" }
                    }),
                ))
                .await?;
            Ok::<(), HarnessError>(())
        })
    };
    let result = within(
        "the refused resume handshake",
        harness(PermissionPolicy::Deny).connect(
            connection.as_ref(),
            PathBuf::from(WORKSPACE),
            "carry on".to_owned(),
            Some(agent_client_protocol_schema::v1::SessionId::from(
                PRIOR_SESSION.to_owned(),
            )),
        ),
    )
    .await?;
    within("the agent script", script)
        .await?
        .map_err(|error| fail("the agent script task must not panic", &error))?
        .map_err(|error| fail("the agent script must complete", &error))?;

    let Err(error) = result else {
        return Err("resume against a load-incapable agent must refuse, not open a turn".into());
    };
    assert!(
        matches!(error, HarnessError::Configuration { .. }),
        "the refusal is a deterministic Configuration error, got {error:?}"
    );
    let message = error.to_string();
    assert!(
        message.contains("loadSession"),
        "the refusal names the missing capability: {message}"
    );
    assert!(
        message.contains(PRIOR_SESSION),
        "the refusal names the session the job asked for: {message}"
    );

    // No session was minted and no load attempted: after the client end is dropped, the agent's
    // reader sees a clean end-of-stream with NO further frame — a `session/new` here would be
    // the silent-fresh-session defect.
    drop(connection);
    let next = within("the agent's post-refusal read", agent.recv())
        .await?
        .map_err(|error| fail("the agent's read must not error", &error))?;
    assert!(
        next.is_none(),
        "no frame follows the refusal — nothing was minted, got {next:?}"
    );
    Ok(())
}
