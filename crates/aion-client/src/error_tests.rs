//! Unit tests for the [`ClientError`](super::ClientError) taxonomy.
//!
//! A sibling file rather than an inline `mod tests`, so `error.rs` stays under
//! the 500-line law without the test bodies being thinned.

use super::{ClientError, ErrorDetail};

fn assert_send_sync_static<T: Send + Sync + 'static>() {}

#[test]
fn client_error_is_send_sync_static() {
    assert_send_sync_static::<ClientError>();
}

/// Every variant of the taxonomy, exercised so adding a variant breaks
/// this list until its class/Display contract is pinned.
fn all_variants() -> Vec<ClientError> {
    vec![
        ClientError::not_found("d"),
        ClientError::already_exists("d"),
        ClientError::query_failed("d"),
        ClientError::query_timeout("d"),
        ClientError::unknown_query("d"),
        ClientError::not_running("d"),
        ClientError::cancelled("d"),
        ClientError::unavailable("d"),
        ClientError::unauthenticated("d"),
        ClientError::namespace_denied("d"),
        ClientError::invalid_argument("d"),
        ClientError::invalid_state("d"),
        ClientError::not_owner("d"),
        ClientError::server("d"),
    ]
}

#[test]
fn display_is_class_colon_detail_for_every_variant() {
    let mut classes = Vec::new();
    for error in all_variants() {
        assert_eq!(
            error.to_string(),
            format!("{}: d", error.class()),
            "{error:?} Display must be `<class>: <detail>`",
        );
        assert_eq!(error.detail().message, "d");
        classes.push(error.class());
    }
    let expected = [
        "not_found",
        "already_exists",
        "query_failed",
        "query_timeout",
        "unknown_query",
        "not_running",
        "cancelled",
        "unavailable",
        "unauthenticated",
        "namespace_denied",
        "invalid_input",
        "invalid_state",
        "not_owner",
        "backend",
    ];
    assert_eq!(classes, expected, "class strings are a pinned contract");
}

#[test]
fn detail_display_appends_the_typed_discriminator() {
    assert_eq!(ErrorDetail::new("plain").to_string(), "plain");
    assert_eq!(
        ErrorDetail::with_type("store unavailable", "Durability").to_string(),
        "store unavailable [Durability]"
    );
    assert_eq!(
        ClientError::not_found(ErrorDetail::with_type(
            "workflow was not found",
            "WorkflowNotFound"
        ))
        .to_string(),
        "not_found: workflow was not found [WorkflowNotFound]"
    );
}

/// The wrong-shard-owner fence is its OWN class, not `unavailable`.
///
/// The two have opposite remedies — re-resolve/retry versus check the endpoint
/// — so a surface that cannot tell them apart gives the wrong advice. This pins
/// the split at the taxonomy, where every surface reads it from the type rather
/// than from message text.
#[test]
fn not_owner_is_its_own_class_and_never_collapses_into_unavailable() {
    let fenced = ClientError::from_wire_error(aion_proto::WireError::not_owner(
        "workflow shard 1 is owned by another cluster node",
    ));
    assert_eq!(fenced.class(), "not_owner");
    assert!(matches!(fenced, ClientError::NotOwner { .. }));

    // The lagged-stream signal keeps its `unavailable` mapping unchanged.
    let lagged = ClientError::from_wire_error(aion_proto::WireError::lagged("subscriber lagged"));
    assert_eq!(lagged.class(), "unavailable");
}
