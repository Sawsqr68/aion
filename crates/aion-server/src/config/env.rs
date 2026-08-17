//! Environment variable overlays for `AION_` prefixed server configuration.

use std::net::SocketAddr;

use crate::{
    config::{ServerConfig, StoreBackend, config_error},
    error::ServerError,
};

/// Apply supported `AION_` environment variable overrides to a config value.
///
/// # Errors
///
/// Returns [`ServerError::Config`] when an environment variable cannot be parsed into the target
/// typed field.
pub fn overlay(config: &mut ServerConfig) -> Result<(), ServerError> {
    overlay_vars(config, std::env::vars())
}

pub(crate) fn overlay_vars(
    config: &mut ServerConfig,
    vars: impl IntoIterator<Item = (String, String)>,
) -> Result<(), ServerError> {
    for (name, value) in vars {
        match name.as_str() {
            "AION_SERVER_LISTEN_ADDRESS" => {
                config.server.listen_address = parse_socket_addr(&name, &value)?;
            }
            "AION_SERVER_GRPC_ADDRESS" => {
                config.server.grpc_address = parse_socket_addr(&name, &value)?;
            }
            "AION_SERVER_CORS_ALLOWED_ORIGINS" => {
                config.server.cors_allowed_origins = parse_csv_origins(&value);
            }
            "AION_STORE_BACKEND" => {
                config.store.backend = parse_store_backend(&name, &value)?;
            }
            "AION_STORE_URL" => {
                if value.is_empty() {
                    return config_error("AION_STORE_URL must not be empty");
                }
                config.store.url = Some(value);
                // AION_STORE_URL names an embedded libSQL database file, so it is
                // an explicit libSQL selection: coerce the implicit durable
                // defaults (memory, or the new haematite default) to libsql. The
                // haematite backend ignores store.url, so a URL is only meaningful
                // as a libSQL choice. See `apply_cli_overrides` for the matching
                // `--store-url` path.
                if matches!(
                    config.store.backend,
                    StoreBackend::Memory | StoreBackend::Haematite
                ) {
                    config.store.backend = StoreBackend::LibSql;
                }
            }
            "AION_STORE_DATA_DIR" => {
                if value.is_empty() {
                    return config_error("AION_STORE_DATA_DIR must not be empty");
                }
                config.store.data_dir = Some(value);
            }
            "AION_STORE_SHARD_COUNT" => {
                config.store.shard_count = parse_positive_usize(&name, &value)?;
            }
            "AION_STORE_NODE_CACHE_BUDGET" => {
                config.store.node_cache_budget = Some(parse_node_cache_budget(&name, &value)?);
            }
            "AION_RUNTIME_SCHEDULER_THREADS" => {
                config.runtime.scheduler_threads = parse_positive_usize(&name, &value)?;
            }
            "AION_RUNTIME_QUERY_TIMEOUT_MS" => {
                config.runtime.query_timeout_ms = Some(parse_positive_u64(&name, &value)?);
            }
            "AION_DRAIN_TIMEOUT_SECONDS" => {
                config.drain.timeout_seconds = parse_positive_u64(&name, &value)?;
            }
            "AION_AUTH_ENABLED" => {
                config.auth.enabled = parse_bool(&name, &value)?;
            }
            "AION_AUTH_JWKS_URL" => {
                if value.is_empty() {
                    return config_error("AION_AUTH_JWKS_URL must not be empty");
                }
                config.auth.jwks_url = Some(value);
            }
            "AION_AUTH_JWKS_REFRESH_SECONDS" => {
                config.auth.jwks_refresh_seconds = parse_positive_u64(&name, &value)?;
            }
            "AION_METRICS_ENABLED" => {
                config.metrics.enabled = parse_bool(&name, &value)?;
            }
            "AION_WEBSOCKET_OUTBOUND_BUFFER_BOUND" => {
                config.websocket.outbound_buffer_bound = parse_positive_usize(&name, &value)?;
            }
            "AION_DEPLOY_ENABLED" => {
                config.deploy.enabled = parse_bool(&name, &value)?;
            }
            "AION_DEPLOY_MAX_ARCHIVE_BYTES" => {
                config.deploy.max_archive_bytes = Some(parse_positive_u64(&name, &value)?);
            }
            "AION_DEPLOY_MAX_INFLATED_BYTES" => {
                config.deploy.max_inflated_bytes = Some(parse_positive_u64(&name, &value)?);
            }
            "AION_DEV_ENABLED" => {
                config.dev.enabled = parse_bool(&name, &value)?;
            }
            other => overlay_authoring(config, other, &value)?,
        }
    }
    Ok(())
}

/// Apply authoring path and default-namespace overrides.
///
/// Split out so the three `[authoring]` paths remain visibly consistent and
/// [`overlay`] stays below the workspace function-length ceiling. Unknown
/// names continue through the existing overlay chain.
fn overlay_authoring(
    config: &mut ServerConfig,
    name: &str,
    value: &str,
) -> Result<(), ServerError> {
    match name {
        "AION_AUTHORING_GLEAM_PATH" => {
            if value.is_empty() {
                return config_error("AION_AUTHORING_GLEAM_PATH must not be empty");
            }
            config.authoring.gleam_path = Some(std::path::PathBuf::from(value));
        }
        "AION_AUTHORING_PROJECT_ROOT" => {
            if value.is_empty() {
                return config_error("AION_AUTHORING_PROJECT_ROOT must not be empty");
            }
            config.authoring.project_root = Some(std::path::PathBuf::from(value));
        }
        "AION_AUTHORING_WORKSPACE_DIR" => {
            if value.is_empty() {
                return config_error("AION_AUTHORING_WORKSPACE_DIR must not be empty");
            }
            config.authoring.workspace_dir = Some(std::path::PathBuf::from(value));
        }
        "AION_NAMESPACES_DEFAULT" => {
            if value.is_empty() {
                return config_error("AION_NAMESPACES_DEFAULT must not be empty");
            }
            value.clone_into(&mut config.namespaces.default);
        }
        other => overlay_websocket(config, other, value)?,
    }
    Ok(())
}

/// Apply the WS3/streaming broadcast-capacity `AION_WEBSOCKET_*` overrides.
///
/// Split out of [`overlay`] so the broadcast-capacity knobs live together and
/// `overlay` stays within the per-function line budget. Unknown names fall
/// through to [`overlay_outbox`] and ultimately the silent-ignore default.
fn overlay_websocket(
    config: &mut ServerConfig,
    name: &str,
    value: &str,
) -> Result<(), ServerError> {
    match name {
        "AION_WEBSOCKET_EVENT_BROADCAST_CAPACITY" => {
            config.websocket.event_broadcast_capacity = Some(parse_positive_usize(name, value)?);
        }
        "AION_WEBSOCKET_CLUSTER_BROADCAST_CAPACITY" => {
            config.websocket.cluster_broadcast_capacity = Some(parse_positive_usize(name, value)?);
        }
        other => overlay_observability(config, other, value)?,
    }
    Ok(())
}

/// Apply the `AION_OBSERVABILITY_*` transcript retention-bound overrides.
///
/// Split out so the observability knobs live together and each overlay stays
/// within the per-function line budget. Unknown names fall through to
/// [`overlay_outbox`] and ultimately the silent-ignore default.
/// Parse `AION_STORE_NODE_CACHE_BUDGET` through haematite's OWN serde repr.
///
/// The variable's value is the TOML right-hand side the operator would write in
/// `config.toml`, verbatim — `{ bytes = 1073741824 }` or `"unlimited"` — so the
/// env override and the file spelling are one spelling, and there is no second
/// byte-size parser anywhere in this crate to drift from haematite's. Wrapping
/// the value in a one-key document is the whole of the translation; deciding
/// what the value MEANS stays with the type that owns it.
///
/// # Errors
///
/// Returns [`ServerError::Config`] naming the variable when the value is not a
/// budget haematite accepts (including a zero byte count, which haematite
/// refuses because a zero ceiling admits nothing).
fn parse_node_cache_budget(
    name: &str,
    value: &str,
) -> Result<haematite::NodeCacheBudget, ServerError> {
    /// The one-key document `value` is parsed as.
    #[derive(serde::Deserialize)]
    struct Document {
        node_cache_budget: haematite::NodeCacheBudget,
    }

    let document: Document = toml::from_str(&format!("node_cache_budget = {value}"))
        .map_err(|error| ServerError::Config {
            message: format!(
                "{name} must be a node cache budget written exactly as it would be in \
                 config.toml — `{{ bytes = <positive integer> }}` or `\"unlimited\"` — got \
                 `{value}`: {error}"
            ),
        })?;
    Ok(document.node_cache_budget)
}

fn overlay_observability(
    config: &mut ServerConfig,
    name: &str,
    value: &str,
) -> Result<(), ServerError> {
    match name {
        "AION_OBSERVABILITY_MAX_EVENT_BYTES" => {
            config.observability.max_event_bytes = parse_positive_usize(name, value)?;
        }
        "AION_OBSERVABILITY_MAX_STREAM_EVENTS" => {
            config.observability.max_stream_events = parse_positive_u64(name, value)?;
        }
        "AION_OBSERVABILITY_MAX_BATCH_EVENTS" => {
            config.observability.max_batch_events = Some(parse_positive_usize(name, value)?);
        }
        "AION_OBSERVABILITY_MAX_BATCH_HOLD_MS" => {
            // Zero is meaningful here (never hold a partial batch), so this
            // parses a non-negative value rather than a positive one.
            config.observability.max_batch_hold_ms = Some(parse_u64(name, value)?);
        }
        other => overlay_outbox(config, other, value)?,
    }
    Ok(())
}

/// Apply the `AION_OUTBOX_*` overrides for the durable-outbox dispatcher.
///
/// Split out of [`overlay`] so the durable-outbox knobs (default-off and inert
/// unless `outbox.enabled` is set) live beside one another and `overlay` stays
/// within the per-function line budget. Unknown names are ignored, exactly as
/// the `overlay` fallthrough does for every non-`AION_` variable.
fn overlay_outbox(config: &mut ServerConfig, name: &str, value: &str) -> Result<(), ServerError> {
    match name {
        "AION_OUTBOX_ENABLED" => {
            config.outbox.enabled = parse_bool(name, value)?;
        }
        "AION_OUTBOX_POLL_INTERVAL_MS" => {
            config.outbox.poll_interval_ms = Some(parse_positive_u64(name, value)?);
        }
        "AION_OUTBOX_BATCH_SIZE" => {
            config.outbox.batch_size = Some(parse_positive_u32(name, value)?);
        }
        "AION_OUTBOX_MAX_ATTEMPTS" => {
            config.outbox.max_attempts = Some(parse_positive_u32(name, value)?);
        }
        "AION_OUTBOX_BACKOFF_BASE_MS" => {
            config.outbox.backoff_base_ms = Some(parse_positive_u64(name, value)?);
        }
        "AION_OUTBOX_BACKOFF_MULTIPLIER" => {
            config.outbox.backoff_multiplier = Some(parse_positive_u32(name, value)?);
        }
        "AION_OUTBOX_BACKOFF_MAX_MS" => {
            config.outbox.backoff_max_ms = Some(parse_positive_u64(name, value)?);
        }
        "AION_OUTBOX_RECONCILE_INTERVAL_MS" => {
            config.outbox.reconcile_interval_ms = Some(parse_positive_u64(name, value)?);
        }
        "AION_OUTBOX_RECONCILE_STALE_AFTER_MS" => {
            config.outbox.reconcile_stale_after_ms = Some(parse_positive_u64(name, value)?);
        }
        "AION_OUTBOX_LIMINAL_LISTEN_ADDRESS" => {
            config.outbox.liminal_listen_address = Some(value.to_owned());
        }
        _ => {}
    }
    Ok(())
}

/// Parse a comma-separated `AION_SERVER_CORS_ALLOWED_ORIGINS` list into the
/// per-origin vector. Entries are trimmed and empties dropped, so an empty
/// value clears the list (back to the secure no-cross-origin default); the
/// resulting origins are validated for shape by `ServerConfig::validate`.
fn parse_csv_origins(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|origin| !origin.is_empty())
        .map(str::to_owned)
        .collect()
}

fn parse_socket_addr(name: &str, value: &str) -> Result<SocketAddr, ServerError> {
    value.parse().map_err(|source| ServerError::Config {
        message: format!("{name} must be a socket address: {source}"),
    })
}

fn parse_store_backend(name: &str, value: &str) -> Result<StoreBackend, ServerError> {
    match value.to_ascii_lowercase().as_str() {
        "memory" => Ok(StoreBackend::Memory),
        "libsql" => Ok(StoreBackend::LibSql),
        "haematite" => Ok(StoreBackend::Haematite),
        _ => config_error(format!("{name} must be one of: memory, libsql, haematite")),
    }
}

fn parse_positive_usize(name: &str, value: &str) -> Result<usize, ServerError> {
    let parsed = value
        .parse::<usize>()
        .map_err(|source| ServerError::Config {
            message: format!("{name} must be a positive integer: {source}"),
        })?;
    if parsed == 0 {
        return config_error(format!("{name} must be a positive integer"));
    }
    Ok(parsed)
}

fn parse_positive_u32(name: &str, value: &str) -> Result<u32, ServerError> {
    let parsed = value.parse::<u32>().map_err(|source| ServerError::Config {
        message: format!("{name} must be a positive integer: {source}"),
    })?;
    if parsed == 0 {
        return config_error(format!("{name} must be a positive integer"));
    }
    Ok(parsed)
}

fn parse_positive_u64(name: &str, value: &str) -> Result<u64, ServerError> {
    let parsed = value.parse::<u64>().map_err(|source| ServerError::Config {
        message: format!("{name} must be a positive integer: {source}"),
    })?;
    if parsed == 0 {
        return config_error(format!("{name} must be a positive integer"));
    }
    Ok(parsed)
}

/// Parse a NON-NEGATIVE integer: for knobs where zero is a meaningful setting
/// rather than a misconfiguration (`observability.max_batch_hold_ms` = never
/// hold a partial batch open).
fn parse_u64(name: &str, value: &str) -> Result<u64, ServerError> {
    value.parse::<u64>().map_err(|source| ServerError::Config {
        message: format!("{name} must be a non-negative integer: {source}"),
    })
}

fn parse_bool(name: &str, value: &str) -> Result<bool, ServerError> {
    match value.to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => config_error(format!("{name} must be a boolean")),
    }
}

#[cfg(test)]
mod tests {
    use super::parse_node_cache_budget;

    /// `AION_STORE_NODE_CACHE_BUDGET` takes the TOML right-hand side verbatim,
    /// in BOTH of haematite's spellings — the env override and the config file
    /// are one spelling, decided by one parser (haematite's).
    #[test]
    fn the_env_override_accepts_both_haematite_spellings()
    -> Result<(), Box<dyn std::error::Error>> {
        let name = "AION_STORE_NODE_CACHE_BUDGET";
        assert_eq!(
            parse_node_cache_budget(name, "{ bytes = 1073741824 }")?,
            haematite::NodeCacheBudget::bytes(1 << 30)?,
            "a 1 GiB ceiling written as it would be in config.toml"
        );
        assert_eq!(
            parse_node_cache_budget(name, "\"unlimited\"")?,
            haematite::NodeCacheBudget::Unlimited,
            "the pre-budget behaviour, spelled out loud"
        );
        Ok(())
    }

    /// A zero ceiling is REFUSED, and the refusal is haematite's own — this
    /// crate never re-decides what a budget may be. A ceiling of zero admits
    /// nothing, which is "disable the cache", a different decision that must be
    /// spelled differently.
    #[test]
    fn the_env_override_refuses_a_zero_ceiling() -> Result<(), Box<dyn std::error::Error>> {
        let error = parse_node_cache_budget("AION_STORE_NODE_CACHE_BUDGET", "{ bytes = 0 }")
            .err()
            .ok_or("a zero byte ceiling must be refused, not accepted as 'no cache'")?;
        let crate::error::ServerError::Config { message } = error else {
            return Err("a bad env value must be a config refusal".into());
        };
        assert!(
            message.contains("AION_STORE_NODE_CACHE_BUDGET"),
            "the refusal must name the variable, got: {message}"
        );
        assert!(
            message.contains("greater than zero"),
            "the refusal must carry haematite's own reason, got: {message}"
        );
        Ok(())
    }

    /// Junk is refused with the variable named and the accepted spellings shown,
    /// rather than silently falling back to any value at all.
    #[test]
    fn the_env_override_refuses_junk() -> Result<(), Box<dyn std::error::Error>> {
        let error = parse_node_cache_budget("AION_STORE_NODE_CACHE_BUDGET", "1GiB")
            .err()
            .ok_or("`1GiB` is not a spelling haematite accepts and must be refused")?;
        let crate::error::ServerError::Config { message } = error else {
            return Err("a bad env value must be a config refusal".into());
        };
        assert!(
            message.contains("unlimited") && message.contains("bytes"),
            "the refusal must show the accepted spellings, got: {message}"
        );
        Ok(())
    }
}
