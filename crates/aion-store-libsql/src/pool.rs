//! Read-connection reuse cache (#122).
//!
//! # What this is, and the thing it deliberately is not
//!
//! WAL permits **unlimited concurrent readers**. So this is a *reuse cache*, not
//! a concurrency limiter: when it holds nothing, a caller **opens a connection
//! rather than waiting for a slot**. Capping concurrency would reintroduce a
//! queue exactly where `SQLite` has none, and would invent a throughput ceiling
//! nobody asked for.
//!
//! `retain` therefore bounds **memory**, not parallelism. That is what lets the
//! unset case be honest: absent retention does not mean "unlimited connections",
//! it means "no reuse" — the live count is bounded by concurrent operations
//! either way.

use std::sync::{Arc, Mutex, PoisonError};

use aion_store::StoreError;

use crate::connection::ConnectionFactory;

/// Idle read connections held for reuse.
pub(crate) struct ReadPool {
    factory: Arc<ConnectionFactory>,
    /// A `std::sync::Mutex`, not tokio's, because [`ReadGuard::drop`] returns
    /// connections and `Drop` cannot await. The critical section is a single
    /// `Vec` push or pop with no `.await` inside it, so it never blocks a
    /// runtime thread for a meaningful interval.
    idle: Mutex<Vec<libsql::Connection>>,
    /// `None` = retain nothing. See the module docs.
    retain: Option<usize>,
}

impl ReadPool {
    pub(crate) fn new(factory: Arc<ConnectionFactory>, retain: Option<usize>) -> Self {
        Self {
            factory,
            idle: Mutex::new(Vec::new()),
            retain,
        }
    }

    /// Take a read connection for the duration of one operation.
    ///
    /// # Errors
    ///
    /// Returns `StoreError::Backend` when the pool is empty and libSQL cannot
    /// open or configure a new connection.
    pub(crate) async fn acquire(self: &Arc<Self>) -> Result<ReadGuard, StoreError> {
        let connection = match self.take_idle() {
            Some(connection) => connection,
            // Empty pool OPENS rather than waits — see the module docs.
            None => self.factory.connect().await?,
        };
        Ok(ReadGuard {
            connection,
            pool: Arc::clone(self),
        })
    }

    fn take_idle(&self) -> Option<libsql::Connection> {
        self.locked_idle().pop()
    }

    /// Lock the idle list, RECOVERING from poisoning rather than failing.
    ///
    /// Poisoning means some thread panicked while holding this lock. That is a
    /// real signal in general — but the guarded value is a `Vec` of connection
    /// handles with no invariant spanning more than one `push`/`pop`, so a panic
    /// cannot have left it half-updated. There is no corrupt state to protect a
    /// caller from, and refusing the lock would turn an unrelated panic into a
    /// permanently unusable read path.
    ///
    /// This is why the pool has no silent-drop branch: recovery is total, so
    /// nothing is ever discarded or swallowed here.
    fn locked_idle(&self) -> std::sync::MutexGuard<'_, Vec<libsql::Connection>> {
        self.idle.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Offer a connection back. Returns whether it was retained, so the caller
    /// can distinguish reuse from a deliberate drop instead of guessing.
    fn release(&self, connection: libsql::Connection) -> bool {
        let Some(retain) = self.retain else {
            return false;
        };
        let mut idle = self.locked_idle();
        if idle.len() >= retain {
            return false;
        }
        idle.push(connection);
        true
    }

    #[cfg(test)]
    pub(crate) fn idle_count(&self) -> usize {
        self.locked_idle().len()
    }
}

/// A read connection owned for one operation, returned to the pool on drop.
///
/// Exclusive for its whole lifetime, which is what makes reads safe under #122:
/// no other task can be inside a transaction on this connection, so a read can
/// never observe another task's uncommitted rows.
pub(crate) struct ReadGuard {
    connection: libsql::Connection,
    pool: Arc<ReadPool>,
}

impl std::ops::Deref for ReadGuard {
    type Target = libsql::Connection;

    fn deref(&self) -> &Self::Target {
        &self.connection
    }
}

impl Drop for ReadGuard {
    fn drop(&mut self) {
        // `libsql::Connection` is `Arc<dyn Conn>`, so the clone names the SAME
        // underlying connection and costs a refcount bump. Handing the clone
        // back lets the guard hold its connection by value — no `Option`, no
        // `unwrap` in `Deref`, and no unreachable branch to justify.
        self.pool.release(self.connection.clone());
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use aion_store::{NamespaceOrigin, NamespaceStore};

    use crate::config::{LibSqlConfig, LibSqlMode};
    use crate::store::LibSqlStore;

    type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

    fn unique_temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        std::env::temp_dir().join(format!(
            "aion-store-libsql-pool-{name}-{}-{nanos}.db",
            std::process::id()
        ))
    }

    fn config(
        path: PathBuf,
        read_pool_size: Option<usize>,
        synchronous: Option<&str>,
    ) -> LibSqlConfig {
        LibSqlConfig {
            mode: LibSqlMode::Embedded { path },
            journal_mode: None,
            synchronous: synchronous.map(ToOwned::to_owned),
            sync_interval_seconds: None,
            busy_timeout_ms: None,
            read_pool_size,
        }
    }

    /// Read `PRAGMA synchronous` from a POOLED connection — never the writer,
    /// and never the first connection opened.
    ///
    /// The pool starts empty, so `store.reader()` produces a connection the
    /// FACTORY made after open: exactly the one a naive pool would have left
    /// unconfigured. Reading the writer instead would be a green about
    /// connection #1 that could not see the defect at all.
    async fn pooled_synchronous(store: &LibSqlStore) -> TestResult<i64> {
        let reader = store.reader().await?;
        let mut rows = reader
            .query("PRAGMA synchronous", ())
            .await
            .map_err(|error| crate::error::libsql_error(&error))?;
        let row = rows
            .next()
            .await
            .map_err(|error| crate::error::libsql_error(&error))?
            .ok_or("PRAGMA synchronous returned no row")?;
        Ok(row
            .get(0)
            .map_err(|error| crate::error::libsql_error(&error))?)
    }

    /// 🔴 The trap named in the design: every PRAGMA except `journal_mode` is
    /// PER-CONNECTION, so a pool that opened connections without re-applying
    /// them would silently drop the operator's durability choice on every
    /// connection but the first.
    ///
    /// # Why this compares against a measured baseline instead of a constant
    ///
    /// The first version of this test asserted a hardcoded `2` for
    /// `synchronous = full`, reasoning that WAL's default is `1`. That reasoning
    /// was wrong: `journal_mode = wal` is applied AFTER the connection opens, so
    /// the WAL-specific default never applies and the connection already reports
    /// `2`. The expected value equalled the default, and the assertion **passed
    /// with the PRAGMA application deleted** — it was proving nothing. It was
    /// caught by deliberately breaking `ConnectionFactory::connect` and watching
    /// this test stay green.
    ///
    /// So the assertion is now the property, not the number: a connection with
    /// the setting applied must report something DIFFERENT from an otherwise
    /// identical connection without it. The baseline is measured in-test, so no
    /// future change to libSQL's defaults can make this vacuous again — and the
    /// non-vacuity guard fails loudly if the chosen value ever coincides with
    /// the default rather than silently asserting a tautology.
    ///
    /// `off` is the value under test purely because no `SQLite` default is `0`.
    /// It is a discriminator for a temp-file fixture, not a durability opinion.
    #[tokio::test]
    async fn a_pooled_connection_carries_the_configured_pragmas_not_only_the_first() -> TestResult {
        const OFF: i64 = 0;

        let unconfigured =
            LibSqlStore::connect(config(unique_temp_path("pragma-baseline"), None, None)).await?;
        let baseline = pooled_synchronous(&unconfigured).await?;

        assert_ne!(
            baseline, OFF,
            "NON-VACUITY GUARD: the unconfigured default is already the value \
             under test, so this test would pass whether or not the PRAGMA is \
             ever applied. Choose a different discriminating value rather than \
             trusting the green"
        );

        let configured =
            LibSqlStore::connect(config(unique_temp_path("pragma"), None, Some("off"))).await?;
        let applied = pooled_synchronous(&configured).await?;

        assert_eq!(
            applied, OFF,
            "a POOLED read connection must carry the operator's synchronous \
             setting; it reported the {baseline} it would show if the pool had \
             opened the connection without re-applying per-connection PRAGMAs"
        );
        Ok(())
    }

    /// The retention cap bounds MEMORY. Three guards live at once — which is
    /// the concurrency assertion — and only two survive their drop.
    #[tokio::test]
    async fn the_pool_retains_at_most_the_configured_count() -> TestResult {
        let store = LibSqlStore::connect(config(unique_temp_path("retain"), Some(2), None)).await?;

        {
            let _a = store.reader().await?;
            let _b = store.reader().await?;
            let _c = store.reader().await?;
            // Three simultaneously, against a retain of 2: if the cap limited
            // CONCURRENCY rather than retention this would deadlock here rather
            // than fail an assertion.
        }

        assert_eq!(
            store.readers().idle_count(),
            2,
            "retain = 2 must keep two connections and drop the third"
        );
        Ok(())
    }

    /// The inverted control for the test above: absent retention must keep
    /// NOTHING, or `None` would silently mean "unbounded cache".
    #[tokio::test]
    async fn an_absent_read_pool_size_retains_nothing() -> TestResult {
        let store = LibSqlStore::connect(config(unique_temp_path("noretain"), None, None)).await?;

        {
            let _a = store.reader().await?;
            let _b = store.reader().await?;
        }

        assert_eq!(
            store.readers().idle_count(),
            0,
            "an unset read_pool_size means NO REUSE, not an unbounded cache"
        );
        Ok(())
    }

    /// 🔴 The test the old suite only appeared to have (#122).
    ///
    /// `concurrent_writers_on_the_same_file_do_not_lock_each_other_out` opens
    /// TWO SEPARATE connections and proves a configuration production does not
    /// use inside a store. This drives MANY TASKS AGAINST ONE STORE, which is
    /// what every `Arc<LibSqlStore>` actually is.
    ///
    /// The writes chosen are deliberate: `register_namespace` is one of the six
    /// entry points that issued its own `BEGIN IMMEDIATE` with no lock held. On
    /// the shared-connection store, these could land a second `BEGIN` on a
    /// connection already inside one — and reads could observe uncommitted rows
    /// from a transaction still open on the same handle.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn many_tasks_against_one_store_interleave_writes_and_reads_safely() -> TestResult {
        const WRITERS: usize = 8;
        const READS_PER_WRITER: usize = 4;

        let store = std::sync::Arc::new(
            LibSqlStore::connect(config(unique_temp_path("one-store"), Some(4), None)).await?,
        );

        let mut tasks = Vec::new();
        for index in 0..WRITERS {
            let store = std::sync::Arc::clone(&store);
            tasks.push(tokio::spawn(async move {
                store
                    .register_namespace(&format!("ns-{index}"), NamespaceOrigin::WorkerMint)
                    .await?;
                for _ in 0..READS_PER_WRITER {
                    store.list_namespaces().await?;
                }
                Ok::<(), aion_store::StoreError>(())
            }));
        }
        for task in tasks {
            task.await??;
        }

        let listed = store.list_namespaces().await?;
        assert_eq!(
            listed.len(),
            WRITERS,
            "every concurrent namespace mint must have committed exactly once — \
             a lost or duplicated row here is the shared-connection defect"
        );
        Ok(())
    }
}
