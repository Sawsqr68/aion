//! Open embedded and embedded-replica libSQL connections.

use std::path::Path;
use std::time::Duration;

use aion_store::StoreError;

use crate::config::{LibSqlConfig, LibSqlMode};

/// Busy-handler wait applied to every opened connection.
///
/// Aion routinely opens MULTIPLE connections against the SAME local database
/// file: production `run.rs` opens one `LibSqlStore` for the engine and a second
/// one for the outbox dispatcher on the identical `store.url`, and recovery /
/// inspection paths open further read handles. Every durable write runs under
/// `BEGIN IMMEDIATE`, which takes `SQLite`'s `RESERVED` lock up front. With the
/// libSQL default busy timeout of zero, a second connection attempting a write
/// while another holds that lock fails *immediately* with
/// `SQLITE_BUSY` ("database is locked") instead of waiting for the in-flight
/// transaction to commit.
///
/// Setting a busy timeout makes contending connections retry for the configured
/// window — the standard, correct way to share a SQLite/libSQL file across
/// connections.
///
/// # 🔴 This number is INHERITED, not chosen
///
/// Ten seconds arrived with the fix that introduced this constant and nobody has
/// ever picked it. It is kept as the fallback **only** because changing it would
/// alter contention behaviour in a change that is supposed to alter none, and
/// inventing a different unchosen number would be worse than keeping this one.
///
/// `LibSqlConfig::busy_timeout_ms` overrides it, and choosing a real value is an
/// open question rather than a settled one. It is named `INHERITED_` so the next
/// reader cannot mistake it for a considered default.
///
/// Note what this timeout now does and does not cover, after #122: within a
/// single store, writes are serialised by the writer connection's own mutex and
/// cannot time out at all. This bounds **cross-store and external** contention —
/// production opens a second `LibSqlStore` for the outbox dispatcher on the same
/// `store.url`, and those two genuinely contend in `SQLite`.
const INHERITED_BUSY_TIMEOUT: Duration = Duration::from_secs(10);

/// Opened libSQL database handle and its mode-agnostic connection.
pub struct OpenedConnection {
    /// Database handle used to create the connection and to trigger replica sync.
    pub database: libsql::Database,
    /// Connection used by the event-store implementation.
    pub connection: libsql::Connection,
}

/// The ONE place a configured connection is produced (#122).
///
/// # Why this exists rather than a second `connect` call site
///
/// Every setting `open_connection` applies is **per-connection state**, except
/// `journal_mode`, which persists in the database file:
///
/// | setting        | scope          |
/// |----------------|----------------|
/// | `busy_timeout` | **connection** |
/// | `synchronous`  | **connection** |
/// | `journal_mode` | file           |
///
/// So a pool that produced connections by calling `database.connect()` directly
/// would hand out connections with libSQL's default busy timeout of **zero** —
/// reintroducing the exact `SQLITE_BUSY` failure [`INHERITED_BUSY_TIMEOUT`] exists to
/// prevent — and would silently drop the operator's chosen `synchronous`
/// durability setting on every connection but the first.
///
/// A second place that configures a connection is a second place to forget, so
/// there is exactly one: [`ConnectionFactory::connect`]. `open_connection` is
/// written in terms of it rather than beside it.
pub struct ConnectionFactory {
    database: libsql::Database,
    busy_timeout: Duration,
    journal_mode: Option<String>,
    synchronous: Option<String>,
    /// True when this crate, not the operator, chose WAL — see `open_connection`.
    default_wal: bool,
}

impl ConnectionFactory {
    /// Produce a fully configured connection. Every caller in the crate,
    /// including the pool, goes through here.
    ///
    /// # Errors
    ///
    /// Returns `StoreError::Backend` when libSQL cannot open the connection or
    /// when applying a configured PRAGMA fails.
    pub async fn connect(&self) -> Result<libsql::Connection, StoreError> {
        let connection = self
            .database
            .connect()
            .map_err(|error| crate::error::libsql_error(&error))?;

        connection
            .busy_timeout(self.busy_timeout)
            .map_err(|error| crate::error::libsql_error(&error))?;

        if self.default_wal {
            execute_pragma(&connection, "journal_mode", "wal").await?;
        }
        apply_pragmas(
            &connection,
            self.journal_mode.as_deref(),
            self.synchronous.as_deref(),
        )
        .await?;

        Ok(connection)
    }

    /// The database handle, for replica sync — which is a database operation,
    /// not a connection one.
    pub fn database(&self) -> &libsql::Database {
        &self.database
    }

    /// The busy-wait this factory stamps on every connection it produces.
    /// Reported rather than assumed, so a contention error can name it.
    pub fn busy_timeout(&self) -> Duration {
        self.busy_timeout
    }
}

/// Open the configured database and return the factory that produces its
/// connections.
///
/// # Errors
///
/// Returns `StoreError::Backend` when libSQL cannot build the database.
pub async fn open_factory(config: &LibSqlConfig) -> Result<ConnectionFactory, StoreError> {
    let database = match &config.mode {
        LibSqlMode::Embedded { path } => build_embedded(path).await?,
        LibSqlMode::EmbeddedReplica {
            path,
            primary_url,
            auth_token,
        } => {
            build_embedded_replica(
                path,
                primary_url.clone(),
                auth_token.clone(),
                config.sync_interval_seconds,
            )
            .await?
        }
    };

    Ok(ConnectionFactory {
        database,
        busy_timeout: config
            .busy_timeout_ms
            .map_or(INHERITED_BUSY_TIMEOUT, Duration::from_millis),
        journal_mode: config.journal_mode.clone(),
        synchronous: config.synchronous.clone(),
        default_wal: matches!(config.mode, LibSqlMode::Embedded { .. })
            && config.journal_mode.is_none(),
    })
}

/// Open the configured libSQL database and return its handle and connection.
///
/// Operator-provided journal and synchronous settings are applied only when present on the
/// configuration. This crate intentionally does not choose durability defaults for omitted
/// tunables.
///
/// # Errors
///
/// Returns `StoreError::Backend` when libSQL cannot build/connect the database or when applying an
/// explicitly configured PRAGMA fails.
pub async fn open_connection(config: &LibSqlConfig) -> Result<OpenedConnection, StoreError> {
    // Written in terms of the factory rather than beside it: one configuration
    // path, so there is one place to forget a per-connection PRAGMA and it is
    // already covered. See `ConnectionFactory`.
    let factory = open_factory(config).await?;
    let connection = factory.connect().await?;
    Ok(OpenedConnection {
        database: factory.database,
        connection,
    })
}

async fn build_embedded(path: &Path) -> Result<libsql::Database, StoreError> {
    libsql::Builder::new_local(path)
        .build()
        .await
        .map_err(|error| crate::error::libsql_error(&error))
}

async fn build_embedded_replica(
    path: &Path,
    primary_url: String,
    auth_token: String,
    sync_interval_seconds: Option<u64>,
) -> Result<libsql::Database, StoreError> {
    let mut builder = libsql::Builder::new_remote_replica(path, primary_url, auth_token);
    if let Some(seconds) = sync_interval_seconds {
        builder = builder.sync_interval(Duration::from_secs(seconds));
    }

    builder
        .build()
        .await
        .map_err(|error| crate::error::libsql_error(&error))
}

async fn apply_pragmas(
    conn: &libsql::Connection,
    journal_mode: Option<&str>,
    synchronous: Option<&str>,
) -> Result<(), StoreError> {
    if let Some(value) = journal_mode {
        execute_pragma(conn, "journal_mode", value).await?;
    }

    if let Some(value) = synchronous {
        execute_pragma(conn, "synchronous", value).await?;
    }

    Ok(())
}

async fn execute_pragma(
    conn: &libsql::Connection,
    name: &str,
    value: &str,
) -> Result<(), StoreError> {
    let value = validate_pragma_value(value)?;
    let sql = format!("PRAGMA {name} = {value}");
    conn.query(&sql, ())
        .await
        .map_err(|error| crate::error::libsql_error(&error))?;

    Ok(())
}

fn validate_pragma_value(value: &str) -> Result<&str, StoreError> {
    if !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        Ok(value)
    } else {
        Err(StoreError::Backend(format!(
            "invalid libSQL PRAGMA value {value:?}"
        )))
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use aion_store::StoreError;

    use super::open_connection;
    use crate::config::{LibSqlConfig, LibSqlMode};

    #[tokio::test]
    async fn opens_embedded_connection_and_queries() -> Result<(), StoreError> {
        let config = LibSqlConfig {
            mode: LibSqlMode::Embedded {
                path: unique_temp_path("embedded-select"),
            },
            journal_mode: Some(String::from("wal")),
            synchronous: Some(String::from("normal")),
            sync_interval_seconds: None,
            busy_timeout_ms: None,
            read_pool_size: None,
        };

        let opened = open_connection(&config).await?;
        let conn = opened.connection;
        let mut rows = conn
            .query("SELECT 1", ())
            .await
            .map_err(|error| crate::error::libsql_error(&error))?;
        let row = rows
            .next()
            .await
            .map_err(|error| crate::error::libsql_error(&error))?
            .ok_or_else(|| StoreError::Backend(String::from("SELECT 1 returned no rows")))?;
        let value: i64 = row
            .get(0)
            .map_err(|error| crate::error::libsql_error(&error))?;

        assert_eq!(value, 1);
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_writers_on_the_same_file_do_not_lock_each_other_out()
    -> Result<(), StoreError> {
        // Two connections on the SAME file, exactly how production opens the
        // engine store and the outbox-dispatcher store on one `store.url`. Both
        // hammer short IMMEDIATE write transactions concurrently. Every durable
        // write takes the RESERVED/write lock up front; with the libSQL default
        // busy timeout of zero (and the default rollback journal) one of these
        // contending writers fails immediately with SQLITE_BUSY ("database is
        // locked"). The WAL + busy-timeout configuration applied in
        // `open_connection` makes the contending writer wait out the lock and
        // commit, so neither side errors.
        const ROUNDS: i64 = 40;
        let path = unique_temp_path("concurrent-writers-shared-file");
        let config = |path: &std::path::Path| LibSqlConfig {
            mode: LibSqlMode::Embedded {
                path: path.to_path_buf(),
            },
            journal_mode: None,
            synchronous: None,
            sync_interval_seconds: None,
            busy_timeout_ms: None,
            read_pool_size: None,
        };

        let setup = open_connection(&config(&path)).await?;
        setup
            .connection
            .execute("CREATE TABLE t (id INTEGER PRIMARY KEY, who INTEGER)", ())
            .await
            .map_err(|error| crate::error::libsql_error(&error))?;
        drop(setup);

        let writer = |who: i64, path: std::path::PathBuf| async move {
            let opened = open_connection(&config(&path)).await?;
            for _ in 0..ROUNDS {
                let tx = opened
                    .connection
                    .transaction_with_behavior(libsql::TransactionBehavior::Immediate)
                    .await
                    .map_err(|error| crate::error::libsql_error(&error))?;
                tx.execute("INSERT INTO t (who) VALUES (?1)", [who])
                    .await
                    .map_err(|error| crate::error::libsql_error(&error))?;
                tx.commit()
                    .await
                    .map_err(|error| crate::error::libsql_error(&error))?;
            }
            Ok::<(), StoreError>(())
        };

        let a = tokio::spawn(writer(1, path.clone()));
        let b = tokio::spawn(writer(2, path.clone()));
        a.await
            .map_err(|error| StoreError::Backend(error.to_string()))??;
        b.await
            .map_err(|error| StoreError::Backend(error.to_string()))??;

        let reader = open_connection(&config(&path)).await?;
        let mut rows = reader
            .connection
            .query("SELECT COUNT(*) FROM t", ())
            .await
            .map_err(|error| crate::error::libsql_error(&error))?;
        let row = rows
            .next()
            .await
            .map_err(|error| crate::error::libsql_error(&error))?
            .ok_or_else(|| StoreError::Backend(String::from("count returned no row")))?;
        let count: i64 = row
            .get(0)
            .map_err(|error| crate::error::libsql_error(&error))?;
        assert_eq!(count, ROUNDS * 2, "every concurrent write committed");
        Ok(())
    }

    #[tokio::test]
    async fn maps_replica_open_failure_to_backend() -> Result<(), Box<dyn std::error::Error>> {
        let config = LibSqlConfig {
            mode: LibSqlMode::EmbeddedReplica {
                path: unique_temp_path("replica-unavailable-primary"),
                primary_url: String::from("http://127.0.0.1:9"),
                auth_token: String::from("token"),
            },
            journal_mode: None,
            synchronous: None,
            sync_interval_seconds: Some(1),
            busy_timeout_ms: None,
            read_pool_size: None,
        };

        match open_connection(&config).await {
            Ok(_) => Err("expected embedded-replica open to fail for an invalid URL".into()),
            Err(StoreError::Backend(_)) => Ok(()),
            Err(other) => Err(format!("expected backend error, got {other:?}").into()),
        }
    }

    fn unique_temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        std::env::temp_dir().join(format!(
            "aion-store-libsql-{name}-{}-{nanos}.db",
            std::process::id()
        ))
    }
}
