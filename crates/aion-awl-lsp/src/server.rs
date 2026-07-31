use std::collections::HashMap;
use std::path::PathBuf;

use lsp_server::{Connection, ErrorCode, Message, Notification, Request, Response};
use lsp_types::{
    CompletionOptions, CompletionParams, CompletionResponse, DidChangeTextDocumentParams,
    DidCloseTextDocumentParams, DidOpenTextDocumentParams, DocumentFormattingParams,
    GotoDefinitionParams, HoverParams, HoverProviderCapability, OneOf, PositionEncodingKind,
    PrepareRenameResponse, PublishDiagnosticsParams, ReferenceParams, RenameOptions, RenameParams,
    ServerCapabilities, TextDocumentPositionParams, TextDocumentSyncCapability,
    TextDocumentSyncKind, TextDocumentSyncOptions, TextEdit, Uri, WorkDoneProgressOptions,
};
use serde::de::DeserializeOwned;

use crate::analysis::{diagnostics, format_document, position_at};
use crate::navigation;

/// A fatal transport or protocol error from the AWL language server.
#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    /// The peer violated the LSP lifecycle protocol.
    #[error("LSP protocol error: {0}")]
    Protocol(#[from] lsp_server::ProtocolError),
    /// Server capabilities could not be encoded.
    #[error("failed to encode LSP capabilities: {0}")]
    Json(#[from] serde_json::Error),
    /// The stdio transport threads could not finish cleanly.
    #[error("LSP stdio transport error: {0}")]
    Io(#[from] std::io::Error),
    /// The peer closed its channel while the server was sending a message.
    #[error("LSP peer disconnected")]
    Disconnected,
    /// A full-sync notification did not contain a replacement document.
    #[error("textDocument/didChange contained no full document text")]
    MissingFullDocument,
    /// A server-only connection unexpectedly received a response.
    #[error("language server received an unexpected client response")]
    UnexpectedResponse,
}

/// Runs the AWL language server over stdin and stdout until shutdown and exit.
///
/// Protocol traffic owns stdout; callers should report a returned fatal error
/// on stderr only.
///
/// # Errors
///
/// Returns an error when initialization, the protocol channel, or stdio
/// transport fails.
pub fn run_stdio() -> Result<(), ServerError> {
    let (connection, io_threads) = Connection::stdio();
    run_connection(&connection)?;
    io_threads.join()?;
    Ok(())
}

/// Runs a fully initialized AWL server on an existing LSP connection.
///
/// This is public so protocol tests and embedders can use `Connection::memory`
/// while exercising the same initialization and request loop as stdio.
///
/// # Errors
///
/// Returns an error for a fatal lifecycle, serialization, or channel failure.
pub fn run_connection(connection: &Connection) -> Result<(), ServerError> {
    let capabilities = serde_json::to_value(capabilities())?;
    drop(connection.initialize(capabilities)?);
    let mut documents = HashMap::new();
    for message in &connection.receiver {
        match message {
            Message::Request(request) => {
                if connection.handle_shutdown(&request)? {
                    break;
                }
                handle_request(connection, request, &documents)?;
            }
            Message::Notification(notification) => {
                handle_notification(connection, notification, &mut documents)?;
            }
            Message::Response(_) => return Err(ServerError::UnexpectedResponse),
        }
    }
    Ok(())
}

fn capabilities() -> ServerCapabilities {
    ServerCapabilities {
        position_encoding: Some(PositionEncodingKind::UTF16),
        text_document_sync: Some(TextDocumentSyncCapability::Options(
            TextDocumentSyncOptions {
                open_close: Some(true),
                change: Some(TextDocumentSyncKind::FULL),
                ..TextDocumentSyncOptions::default()
            },
        )),
        document_formatting_provider: Some(OneOf::Left(true)),
        document_symbol_provider: Some(OneOf::Left(true)),
        definition_provider: Some(OneOf::Left(true)),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        references_provider: Some(OneOf::Left(true)),
        // `prepareProvider` is on so the editor asks which range it may rename
        // BEFORE prompting. Without it a client offers a rename box over a
        // comment or a keyword and only learns it was impossible after the
        // author has typed a new name.
        rename_provider: Some(OneOf::Right(RenameOptions {
            prepare_provider: Some(true),
            work_done_progress_options: WorkDoneProgressOptions::default(),
        })),
        completion_provider: Some(CompletionOptions {
            // A completion list is complete as returned: every candidate is
            // already in it, and the client filters by the prefix the author is
            // typing. There is nothing a resolve round-trip could add.
            resolve_provider: Some(false),
            // AWL's openers are plain words, so the client's own identifier
            // trigger is what fires completion. These are the characters after
            // which a word does NOT start but a candidate is still wanted: the
            // pipe's second character, and the colon before a type.
            trigger_characters: Some(vec![">".to_owned(), ":".to_owned()]),
            ..CompletionOptions::default()
        }),
        ..ServerCapabilities::default()
    }
}

fn handle_notification(
    connection: &Connection,
    notification: Notification,
    documents: &mut HashMap<String, String>,
) -> Result<(), ServerError> {
    match notification.method.as_str() {
        "textDocument/didOpen" => {
            let params: DidOpenTextDocumentParams = decode_notification(notification)?;
            let uri = params.text_document.uri;
            let version = params.text_document.version;
            documents.insert(uri.as_str().to_owned(), params.text_document.text);
            publish(connection, &uri, version, documents)
        }
        "textDocument/didChange" => {
            let params: DidChangeTextDocumentParams = decode_notification(notification)?;
            let uri = params.text_document.uri;
            let version = params.text_document.version;
            let Some(change) = params.content_changes.into_iter().last() else {
                return Err(ServerError::MissingFullDocument);
            };
            documents.insert(uri.as_str().to_owned(), change.text);
            publish(connection, &uri, version, documents)
        }
        "textDocument/didClose" => {
            let params: DidCloseTextDocumentParams = decode_notification(notification)?;
            let uri = params.text_document.uri;
            documents.remove(uri.as_str());
            let params = PublishDiagnosticsParams::new(uri, Vec::new(), None);
            send(
                connection,
                Message::Notification(lsp_server::Notification::new(
                    "textDocument/publishDiagnostics".to_owned(),
                    params,
                )),
            )
        }
        _ => Ok(()),
    }
}

fn decode_notification<T: DeserializeOwned>(notification: Notification) -> Result<T, ServerError> {
    serde_json::from_value(notification.params).map_err(ServerError::from)
}

fn publish(
    connection: &Connection,
    uri: &Uri,
    version: i32,
    documents: &HashMap<String, String>,
) -> Result<(), ServerError> {
    let Some(source) = documents.get(uri.as_str()) else {
        return Err(ServerError::Disconnected);
    };
    let root = document_root(uri);
    let diagnostics = diagnostics(source, root.as_deref());
    let params = PublishDiagnosticsParams::new(uri.clone(), diagnostics, Some(version));
    send(
        connection,
        Message::Notification(lsp_server::Notification::new(
            "textDocument/publishDiagnostics".to_owned(),
            params,
        )),
    )
}

fn document_root(uri: &Uri) -> Option<PathBuf> {
    let url = url::Url::parse(uri.as_str()).ok()?;
    let path = url.to_file_path().ok()?;
    path.parent().map(PathBuf::from)
}

fn handle_request(
    connection: &Connection,
    request: Request,
    documents: &HashMap<String, String>,
) -> Result<(), ServerError> {
    match request.method.as_str() {
        "textDocument/formatting" => {
            let Some(params) = decode_request::<DocumentFormattingParams>(connection, &request)?
            else {
                return Ok(());
            };
            let edits = documents
                .get(params.text_document.uri.as_str())
                .map_or_else(Vec::new, |source| formatting_edits(source));
            respond(connection, request.id, edits)
        }
        "textDocument/documentSymbol" => {
            let Some(params) =
                decode_request::<lsp_types::DocumentSymbolParams>(connection, &request)?
            else {
                return Ok(());
            };
            let symbols = documents
                .get(params.text_document.uri.as_str())
                .and_then(|source| navigation::document_symbols(source))
                .unwrap_or_else(|| serde_json::json!([]));
            respond(connection, request.id, symbols)
        }
        "textDocument/definition" => {
            let Some(params) = decode_request::<GotoDefinitionParams>(connection, &request)? else {
                return Ok(());
            };
            let TextDocumentPositionParams {
                text_document,
                position,
            } = params.text_document_position_params;
            let location = documents
                .get(text_document.uri.as_str())
                .and_then(|source| {
                    let root = document_root(&text_document.uri);
                    navigation::definition(source, &text_document.uri, position, root.as_deref())
                });
            respond(connection, request.id, location)
        }
        "textDocument/completion" => {
            let Some(params) = decode_request::<CompletionParams>(connection, &request)? else {
                return Ok(());
            };
            let TextDocumentPositionParams {
                text_document,
                position,
            } = params.text_document_position;
            // An unopened document yields an empty list rather than `null`: a
            // client reads `null` as "this server declined", and the server did
            // not decline — it has nothing to offer for a file it was never
            // sent.
            let items = documents
                .get(text_document.uri.as_str())
                .map_or_else(Vec::new, |source| {
                    crate::completion::completions(source, position)
                });
            respond(connection, request.id, CompletionResponse::Array(items))
        }
        "textDocument/references" => handle_references(connection, request, documents),
        "textDocument/prepareRename" => handle_prepare_rename(connection, request, documents),
        "textDocument/rename" => {
            let Some(params) = decode_request::<RenameParams>(connection, &request)? else {
                return Ok(());
            };
            handle_rename(connection, request.id, params, documents)
        }
        "textDocument/hover" => {
            let Some(params) = decode_request::<HoverParams>(connection, &request)? else {
                return Ok(());
            };
            let TextDocumentPositionParams {
                text_document,
                position,
            } = params.text_document_position_params;
            let hover = documents
                .get(text_document.uri.as_str())
                .and_then(|source| {
                    let root = document_root(&text_document.uri);
                    navigation::hover(source, position, root.as_deref())
                });
            respond(connection, request.id, hover)
        }
        _ => {
            let response = Response::new_err(
                request.id,
                ErrorCode::MethodNotFound as i32,
                format!("unsupported request: {}", request.method),
            );
            send(connection, Message::Response(response))
        }
    }
}

/// Every span the checker bound to the declaration under the cursor.
fn handle_references(
    connection: &Connection,
    request: Request,
    documents: &HashMap<String, String>,
) -> Result<(), ServerError> {
    let Some(params) = decode_request::<ReferenceParams>(connection, &request)? else {
        return Ok(());
    };
    let include = params.context.include_declaration;
    let TextDocumentPositionParams {
        text_document,
        position,
    } = params.text_document_position;
    let locations = documents
        .get(text_document.uri.as_str())
        .and_then(|source| {
            let root = document_root(&text_document.uri);
            crate::rename::references(
                source,
                &text_document.uri,
                position,
                root.as_deref(),
                include,
            )
        })
        .unwrap_or_default();
    respond(connection, request.id, locations)
}

/// The range an editor should pre-fill, or `null` when nothing here renames.
fn handle_prepare_rename(
    connection: &Connection,
    request: Request,
    documents: &HashMap<String, String>,
) -> Result<(), ServerError> {
    let Some(params) = decode_request::<TextDocumentPositionParams>(connection, &request)? else {
        return Ok(());
    };
    let range = documents
        .get(params.text_document.uri.as_str())
        .and_then(|source| {
            let root = document_root(&params.text_document.uri);
            crate::rename::prepare(source, params.position, root.as_deref())
        });
    respond(
        connection,
        request.id,
        range.map(PrepareRenameResponse::Range),
    )
}

/// Apply a rename, or report the refusal to the CLIENT rather than swallowing
/// it.
///
/// A refused rename must come back as a protocol error: an editor that gets an
/// empty edit list shows the author nothing at all and they conclude the
/// rename simply did not take. The refusal text says which rule was broken.
fn handle_rename(
    connection: &Connection,
    id: lsp_server::RequestId,
    params: RenameParams,
    documents: &HashMap<String, String>,
) -> Result<(), ServerError> {
    let uri = params.text_document_position.text_document.uri;
    let Some(source) = documents.get(uri.as_str()) else {
        let response = Response::new_err(
            id,
            ErrorCode::InvalidParams as i32,
            format!(
                "cannot rename in `{}`: the document is not open",
                uri.as_str()
            ),
        );
        return send(connection, Message::Response(response));
    };
    let root = document_root(&uri);
    match crate::rename::rename(
        source,
        params.text_document_position.position,
        &params.new_name,
        root.as_deref(),
    ) {
        Ok(edits) => {
            // The wire shape is `WorkspaceEdit.changes`, built here as a JSON
            // object rather than through `lsp_types::WorkspaceEdit`. That
            // struct keys its map by `Uri`, which carries interior mutability
            // and is therefore not a sound hash key — the URI is a plain string
            // on the wire, and that is what it is keyed by here.
            let mut changes = serde_json::Map::new();
            changes.insert(uri.as_str().to_owned(), serde_json::to_value(edits)?);
            respond(connection, id, serde_json::json!({ "changes": changes }))
        }
        Err(refusal) => {
            let response =
                Response::new_err(id, ErrorCode::RequestFailed as i32, refusal.to_string());
            send(connection, Message::Response(response))
        }
    }
}

fn decode_request<T: DeserializeOwned>(
    connection: &Connection,
    request: &Request,
) -> Result<Option<T>, ServerError> {
    match serde_json::from_value(request.params.clone()) {
        Ok(params) => Ok(Some(params)),
        Err(error) => {
            let response = Response::new_err(
                request.id.clone(),
                ErrorCode::InvalidParams as i32,
                error.to_string(),
            );
            send(connection, Message::Response(response))?;
            Ok(None)
        }
    }
}

fn formatting_edits(source: &str) -> Vec<TextEdit> {
    let Some(formatted) = format_document(source) else {
        return Vec::new();
    };
    if formatted == source {
        return Vec::new();
    }
    vec![TextEdit::new(
        lsp_types::Range::new(
            lsp_types::Position::new(0, 0),
            position_at(source, source.len()),
        ),
        formatted,
    )]
}

fn respond<T: serde::Serialize>(
    connection: &Connection,
    id: lsp_server::RequestId,
    value: T,
) -> Result<(), ServerError> {
    send(connection, Message::Response(Response::new_ok(id, value)))
}

fn send(connection: &Connection, message: Message) -> Result<(), ServerError> {
    connection
        .sender
        .send(message)
        .map_err(|_| ServerError::Disconnected)
}
