//! Live in-process protocol smoke for the stdio server lifecycle and features.

use std::error::Error;
use std::thread;

use aion_awl_lsp::run_connection;
use lsp_server::{Connection, Message, Notification, Request, RequestId};
use serde_json::json;

type TestResult = Result<(), Box<dyn Error>>;

fn send(connection: &Connection, message: Message) -> TestResult {
    connection.sender.send(message)?;
    Ok(())
}

fn receive(connection: &Connection) -> Result<Message, Box<dyn Error>> {
    Ok(connection.receiver.recv()?)
}

#[test]
fn live_initialize_open_diagnose_format_shutdown_exchange() -> TestResult {
    let (server, client) = Connection::memory();
    let server_thread = thread::spawn(move || run_connection(&server));

    send(
        &client,
        Message::Request(Request::new(
            RequestId::from(1),
            "initialize".to_owned(),
            json!({ "capabilities": {} }),
        )),
    )?;
    let Message::Response(initialize) = receive(&client)? else {
        return Err("server did not respond to initialize".into());
    };
    assert!(
        initialize.error.is_none(),
        "initialize failed: {initialize:?}"
    );
    let capabilities = initialize
        .result
        .ok_or("initialize response omitted capabilities")?;
    assert_eq!(capabilities["capabilities"]["positionEncoding"], "utf-16");
    assert_eq!(
        capabilities["capabilities"]["textDocumentSync"]["change"],
        1
    );

    send(
        &client,
        Message::Notification(Notification::new("initialized".to_owned(), json!({}))),
    )?;
    let uri = "file:///tmp/broken.awl";
    let broken = "//! Broken route.\nworkflow probe\n  outcome done: type String, route success\n\nstep one\n  route missing\n";
    send(
        &client,
        Message::Notification(Notification::new(
            "textDocument/didOpen".to_owned(),
            json!({
                "textDocument": {
                    "uri": uri,
                    "languageId": "awl",
                    "version": 1,
                    "text": broken
                }
            }),
        )),
    )?;
    let Message::Notification(published) = receive(&client)? else {
        return Err("server did not publish diagnostics".into());
    };
    assert_eq!(published.method, "textDocument/publishDiagnostics");
    let diagnostics = published.params["diagnostics"]
        .as_array()
        .ok_or("diagnostics payload is not an array")?;
    assert!(!diagnostics.is_empty(), "broken document checked clean");
    assert_eq!(diagnostics[0]["source"], "awl");

    send(
        &client,
        Message::Request(Request::new(
            RequestId::from(2),
            "textDocument/formatting".to_owned(),
            json!({
                "textDocument": { "uri": uri },
                "options": { "tabSize": 2, "insertSpaces": true }
            }),
        )),
    )?;
    let Message::Response(formatting) = receive(&client)? else {
        return Err("server did not respond to formatting".into());
    };
    assert!(
        formatting.error.is_none(),
        "formatting failed: {formatting:?}"
    );
    assert!(
        formatting.result.is_some_and(|value| value.is_array()),
        "formatting response was not an edit array"
    );

    send(
        &client,
        Message::Request(Request::new(
            RequestId::from(3),
            "shutdown".to_owned(),
            serde_json::Value::Null,
        )),
    )?;
    let Message::Response(shutdown) = receive(&client)? else {
        return Err("server did not respond to shutdown".into());
    };
    assert!(shutdown.error.is_none(), "shutdown failed: {shutdown:?}");
    send(
        &client,
        Message::Notification(Notification::new(
            "exit".to_owned(),
            serde_json::Value::Null,
        )),
    )?;
    let server_result = server_thread.join().map_err(|_| "server thread panicked")?;
    server_result?;
    Ok(())
}

#[test]
fn rev2_corpus_hover_and_definition_use_checker_semantics() -> TestResult {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../aion-awl/tests/fixtures/rev2/header-types/valid/doc_comments.awl");
    let source = std::fs::read_to_string(&fixture)?;
    let uri = format!("file://{}", fixture.display());
    let (server, client) = Connection::memory();
    let server_thread = thread::spawn(move || run_connection(&server));

    send(
        &client,
        Message::Request(Request::new(
            RequestId::from(10),
            "initialize".to_owned(),
            json!({ "capabilities": {} }),
        )),
    )?;
    let Message::Response(initialize) = receive(&client)? else {
        return Err("server did not respond to initialize".into());
    };
    let capabilities = initialize.result.ok_or("initialize omitted capabilities")?;
    assert_eq!(capabilities["capabilities"]["hoverProvider"], true);
    assert_eq!(capabilities["capabilities"]["definitionProvider"], true);
    send(
        &client,
        Message::Notification(Notification::new("initialized".to_owned(), json!({}))),
    )?;
    send(
        &client,
        Message::Notification(Notification::new(
            "textDocument/didOpen".to_owned(),
            json!({
                "textDocument": {
                    "uri": uri,
                    "languageId": "awl",
                    "version": 1,
                    "text": source
                }
            }),
        )),
    )?;
    let Message::Notification(published) = receive(&client)? else {
        return Err("server did not publish corpus diagnostics".into());
    };
    assert_eq!(published.method, "textDocument/publishDiagnostics");
    assert_eq!(published.params["diagnostics"], json!([]));

    assert_semantic_requests(&client, &uri)?;

    send(
        &client,
        Message::Request(Request::new(
            RequestId::from(13),
            "shutdown".to_owned(),
            serde_json::Value::Null,
        )),
    )?;
    let Message::Response(shutdown) = receive(&client)? else {
        return Err("server did not respond to shutdown".into());
    };
    assert!(shutdown.error.is_none(), "shutdown failed: {shutdown:?}");
    send(
        &client,
        Message::Notification(Notification::new(
            "exit".to_owned(),
            serde_json::Value::Null,
        )),
    )?;
    let server_result = server_thread.join().map_err(|_| "server thread panicked")?;
    server_result?;
    Ok(())
}

fn assert_semantic_requests(client: &Connection, uri: &str) -> TestResult {
    send(
        client,
        Message::Request(Request::new(
            RequestId::from(11),
            "textDocument/hover".to_owned(),
            json!({
                "textDocument": { "uri": uri },
                "position": { "line": 21, "character": 34 }
            }),
        )),
    )?;
    let Message::Response(hover) = receive(client)? else {
        return Err("server did not respond to hover".into());
    };
    assert!(hover.error.is_none(), "hover failed: {hover:?}");
    let hover = hover.result.ok_or("hover returned null")?;
    assert_eq!(hover["contents"]["kind"], "markdown");
    let markdown = hover["contents"]["value"]
        .as_str()
        .ok_or("hover markdown is not a string")?;
    assert!(markdown.contains("type Document: Document"), "{markdown}");
    assert!(
        markdown.contains("A fetched document, exactly as retrieved."),
        "{markdown}"
    );

    send(
        client,
        Message::Request(Request::new(
            RequestId::from(12),
            "textDocument/definition".to_owned(),
            json!({
                "textDocument": { "uri": uri },
                "position": { "line": 25, "character": 10 }
            }),
        )),
    )?;
    let Message::Response(definition) = receive(client)? else {
        return Err("server did not respond to definition".into());
    };
    assert!(
        definition.error.is_none(),
        "definition failed: {definition:?}"
    );
    let definition = definition.result.ok_or("definition returned null")?;
    assert_eq!(definition["uri"], uri);
    assert_eq!(definition["range"]["start"]["line"], 21);
    assert_eq!(definition["range"]["start"]["character"], 9);
    Ok(())
}

/// A document left MID-EDIT: a half-typed statement on the last line, which is
/// the only state in which a real editor asks for completion.
const HALF_TYPED: &str = "//! Completing.\nworkflow probe\n  input incident_id: String\n\nworker ops\n  action gather(incident_id: String) -> Diagnostics\n\nstep collect\n  ga";

/// Drive the handshake and open `source`, returning once its diagnostics land.
///
/// Asserts the completion capability on the way through: an editor asks by
/// advertising, so a provider the server never announces is never requested,
/// and the feature would be unreachable however well it worked.
fn initialize_and_open(client: &Connection, uri: &str, source: &str) -> TestResult {
    send(
        client,
        Message::Request(Request::new(
            RequestId::from(20),
            "initialize".to_owned(),
            json!({ "capabilities": {} }),
        )),
    )?;
    let Message::Response(initialize) = receive(client)? else {
        return Err("server did not respond to initialize".into());
    };
    let capabilities = initialize.result.ok_or("initialize omitted capabilities")?;
    let completion = &capabilities["capabilities"]["completionProvider"];
    assert!(
        completion.is_object(),
        "the server must ADVERTISE completion or no editor will ever ask for it: {capabilities}"
    );
    assert_eq!(completion["resolveProvider"], false);

    send(
        client,
        Message::Notification(Notification::new("initialized".to_owned(), json!({}))),
    )?;
    send(
        client,
        Message::Notification(Notification::new(
            "textDocument/didOpen".to_owned(),
            json!({
                "textDocument": {
                    "uri": uri,
                    "languageId": "awl",
                    "version": 1,
                    "text": source
                }
            }),
        )),
    )?;
    let Message::Notification(published) = receive(client)? else {
        return Err("server did not publish diagnostics".into());
    };
    assert_eq!(published.method, "textDocument/publishDiagnostics");
    Ok(())
}

/// Ask for completion at the very end of `source` and return the labels.
fn completion_labels(
    client: &Connection,
    uri: &str,
    source: &str,
) -> Result<Vec<String>, Box<dyn Error>> {
    let line = u32::try_from(source.lines().count().saturating_sub(1))?;
    let column = u32::try_from(
        source
            .lines()
            .next_back()
            .unwrap_or_default()
            .chars()
            .count(),
    )?;
    send(
        client,
        Message::Request(Request::new(
            RequestId::from(21),
            "textDocument/completion".to_owned(),
            json!({
                "textDocument": { "uri": uri },
                "position": { "line": line, "character": column }
            }),
        )),
    )?;
    let Message::Response(response) = receive(client)? else {
        return Err("server did not respond to completion".into());
    };
    assert!(
        response.error.is_none(),
        "completion failed over the wire: {response:?}"
    );
    let items = response
        .result
        .ok_or("completion response carried no result")?;
    Ok(items
        .as_array()
        .ok_or("completion result was not an item array")?
        .iter()
        .filter_map(|item| item["label"].as_str().map(str::to_owned))
        .collect())
}

fn shutdown_and_exit(client: &Connection, id: i32) -> TestResult {
    send(
        client,
        Message::Request(Request::new(
            RequestId::from(id),
            "shutdown".to_owned(),
            serde_json::Value::Null,
        )),
    )?;
    let Message::Response(shutdown) = receive(client)? else {
        return Err("server did not respond to shutdown".into());
    };
    assert!(shutdown.error.is_none(), "shutdown failed: {shutdown:?}");
    send(
        client,
        Message::Notification(Notification::new(
            "exit".to_owned(),
            serde_json::Value::Null,
        )),
    )
}

/// THE WIRE PROOF. The completion provider is advertised in the handshake AND
/// a real `textDocument/completion` request comes back answered, on a document
/// that does not parse.
///
/// Both halves matter and either alone is satisfiable while the author still
/// gets nothing: a provider that works when called directly but is never
/// announced is not reachable, and an announced provider that returns an empty
/// list for a half-typed line is not a feature.
#[test]
fn live_completion_answers_a_half_typed_step_body() -> TestResult {
    assert!(
        aion_awl::parse(HALF_TYPED).is_err(),
        "the premise is a document mid-edit; if it parses, this proves nothing about the case \
         completion exists for"
    );

    let (server, client) = Connection::memory();
    let server_thread = thread::spawn(move || run_connection(&server));
    let uri = "file:///tmp/completing.awl";

    initialize_and_open(&client, uri, HALF_TYPED)?;
    let labels = completion_labels(&client, uri, HALF_TYPED)?;
    assert!(
        labels.iter().any(|label| label == "gather"),
        "the declared action must come back over the wire for a half-typed step body: {labels:?}"
    );
    assert!(
        labels.iter().any(|label| label == "spawn"),
        "the step-body vocabulary must come back too: {labels:?}"
    );

    shutdown_and_exit(&client, 22)?;
    let server_result = server_thread.join().map_err(|_| "server thread panicked")?;
    server_result?;
    Ok(())
}

/// A document that checks clean, for the rename exchange.
const RENAMEABLE: &str = "//! Rename over the wire.\nworkflow triage\n  input incident_id: String\n  outcome resolved: type Resolved, route success\n\ntype Resolved { incident_id: String }\n\nstep report\n  outcome ok: otherwise, route resolved(incident_id: incident_id)\n";

fn request(client: &Connection, id: i32, method: &str, params: serde_json::Value) -> TestResult {
    send(
        client,
        Message::Request(Request::new(RequestId::from(id), method.to_owned(), params)),
    )
}

/// THE WIRE PROOF for references and rename, including the refusal.
///
/// The refusal half is the one that cannot be skipped. A refused rename MUST
/// come back as a protocol error: an editor handed an empty edit list shows the
/// author nothing and they conclude the rename simply did not take, which is
/// indistinguishable from a broken server and teaches them to stop trusting it.
#[test]
fn live_references_and_rename_including_a_refusal() -> TestResult {
    let (server, client) = Connection::memory();
    let server_thread = thread::spawn(move || run_connection(&server));
    let uri = "file:///tmp/renaming.awl";

    initialize_and_open(&client, uri, RENAMEABLE)?;

    // The declaration is on line 2 (`  input incident_id: String`), at the
    // start of the name.
    let at_name = json!({ "line": 2, "character": 8 });

    request(
        &client,
        30,
        "textDocument/references",
        json!({
            "textDocument": { "uri": uri },
            "position": at_name,
            "context": { "includeDeclaration": true }
        }),
    )?;
    let Message::Response(found) = receive(&client)? else {
        return Err("server did not respond to references".into());
    };
    assert!(found.error.is_none(), "references failed: {found:?}");
    let locations = found
        .result
        .ok_or("references carried no result")?
        .as_array()
        .ok_or("references result was not an array")?
        .len();
    assert!(
        locations >= 2,
        "the input is declared and used, so a single location means references are not being \
         collected: {locations}"
    );

    request(
        &client,
        31,
        "textDocument/rename",
        json!({
            "textDocument": { "uri": uri },
            "position": at_name,
            "newName": "case_id"
        }),
    )?;
    let Message::Response(renamed) = receive(&client)? else {
        return Err("server did not respond to rename".into());
    };
    assert!(renamed.error.is_none(), "rename failed: {renamed:?}");
    let edits = renamed.result.ok_or("rename carried no result")?;
    let edits = edits["changes"][uri]
        .as_array()
        .ok_or("rename produced no edits for the document")?;
    assert!(
        edits.len() >= 2,
        "a rename must edit the declaration AND its uses: {edits:?}"
    );

    // THE REFUSAL. `TitleCase` cannot stand where a `snake_case` name does.
    request(
        &client,
        32,
        "textDocument/rename",
        json!({
            "textDocument": { "uri": uri },
            "position": at_name,
            "newName": "CaseId"
        }),
    )?;
    let Message::Response(refused) = receive(&client)? else {
        return Err("server did not respond to the refused rename".into());
    };
    assert!(
        refused.result.is_none(),
        "a refusal must not also carry edits: {refused:?}"
    );
    let error = refused.error.ok_or(
        "a refused rename came back as SUCCESS — the author is shown nothing and concludes the \
         rename did not take",
    )?;
    assert!(
        error.message.contains("snake_case"),
        "the refusal must reach the author saying which rule was broken: {}",
        error.message
    );

    shutdown_and_exit(&client, 33)?;
    let server_result = server_thread.join().map_err(|_| "server thread panicked")?;
    server_result?;
    Ok(())
}
