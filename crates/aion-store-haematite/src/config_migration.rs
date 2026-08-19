//! Pre-open adoption of `node_cache_budget` into stores that predate the field.
//!
//! haematite 0.8.2 made `node_cache_budget` a REQUIRED field of the persisted
//! database config: `Database::open` refuses a `config.json` that does not
//! rule on it. A store created by an earlier haematite therefore fails to open
//! after an engine upgrade even when the operator's configuration carries a
//! perfectly good ruling — an update that presents as data loss (aion#75; live
//! exhibit 2026-08-18: a healthy store refused with "database config is
//! missing `node_cache_budget`" while the server's own TOML held the value).
//!
//! The cure is adoption, not refusal: when an existing store's persisted
//! config does not rule on the budget and the caller supplies a ruling, write
//! the ruling into the persisted config ONCE — atomically, preserving every
//! other recorded field byte-for-byte at the JSON level — then open normally.
//! A store that already rules on its budget is never touched: the recorded
//! ruling wins over the caller's, always, so a restart can never silently
//! re-cut a ceiling the operator chose at creation. A caller that has no
//! ruling still meets haematite's own typed refusal — adoption only ever runs
//! with a value in hand, so the "somebody must decide" property is preserved.

use std::io::Write as _;
use std::path::Path;

use aion_store::StoreError;
use haematite::NodeCacheBudget;

/// The persisted database config file haematite reads back on open. One name,
/// used by the adoption check and by every open-vs-create predicate in this
/// crate ([`crate::store`] tests the same file name).
const CONFIG_FILE: &str = "config.json";

/// The adoption's scratch file, written and fsynced in full before an atomic
/// rename over [`CONFIG_FILE`]. Lives in the same directory so the rename
/// never crosses a filesystem boundary.
const CONFIG_ADOPT_TMP: &str = "config.json.adopt-tmp";

/// What [`adopt_node_cache_budget`] found and did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BudgetAdoption {
    /// The persisted config already rules on `node_cache_budget`; the file was
    /// not touched, and the recorded ruling is the one haematite reads back.
    AlreadyRuled,
    /// The persisted config predated the field (or carried an explicit `null`,
    /// which haematite reads as "nobody ruled"); the caller's ruling was
    /// written into it atomically.
    Adopted,
}

/// Adopt `budget` into `data_dir`'s persisted database config when — and only
/// when — that config does not already rule on `node_cache_budget`.
///
/// A present, non-`null` ruling always wins: the file is left untouched and
/// the caller's value is ignored, exactly as an unmigrated open would behave.
/// An absent key, or an explicit `null` (the serialization of "nobody ruled"),
/// is replaced by the caller's ruling with a write-fsync-rename-fsync sequence
/// so a crash at any point leaves either the old file or the new file, never a
/// torn one.
///
/// # Errors
///
/// Returns [`StoreError::Backend`] naming the file and the failing step when
/// the config cannot be read, is not a JSON object (a corrupt file is
/// repaired by the operator, never rewritten blind by this migration), the
/// ruling cannot be serialized, or the atomic replacement fails. The original
/// file is left untouched on every error path.
pub(crate) fn adopt_node_cache_budget(
    data_dir: &Path,
    budget: NodeCacheBudget,
) -> Result<BudgetAdoption, StoreError> {
    let config_path = data_dir.join(CONFIG_FILE);
    let bytes = std::fs::read(&config_path).map_err(|error| {
        StoreError::Backend(format!(
            "cannot read persisted database config `{}` for the node_cache_budget \
             adoption check: {error}",
            config_path.display()
        ))
    })?;
    let mut config: serde_json::Value = serde_json::from_slice(&bytes).map_err(|error| {
        StoreError::Backend(format!(
            "persisted database config `{}` is not valid JSON ({error}); refusing to \
             touch it — restore or repair the file, then reopen this store",
            config_path.display()
        ))
    })?;
    let serde_json::Value::Object(fields) = &mut config else {
        return Err(StoreError::Backend(format!(
            "persisted database config `{}` is not a JSON object; refusing to touch \
             it — restore or repair the file, then reopen this store",
            config_path.display()
        )));
    };
    if fields
        .get("node_cache_budget")
        .is_some_and(|ruling| !ruling.is_null())
    {
        return Ok(BudgetAdoption::AlreadyRuled);
    }
    let ruling = serde_json::to_value(budget).map_err(|error| {
        StoreError::Backend(format!(
            "cannot serialize the node_cache_budget ruling for adoption into `{}`: {error}",
            config_path.display()
        ))
    })?;
    fields.insert("node_cache_budget".to_owned(), ruling);
    replace_config_atomically(&config_path, &config)?;
    tracing::info!(
        config = %config_path.display(),
        budget = ?budget,
        "adopted node_cache_budget into a pre-budget store config (one-time migration; \
         the recorded ruling now wins over any later configured value)"
    );
    Ok(BudgetAdoption::Adopted)
}

/// Replace `config_path` with the serialized `config` atomically: write the
/// full new content to a sibling scratch file, fsync it, rename it over the
/// original, then fsync the directory so the rename itself is durable. A crash
/// at any point leaves either the complete old file or the complete new file.
fn replace_config_atomically(
    config_path: &Path,
    config: &serde_json::Value,
) -> Result<(), StoreError> {
    let dir = config_path.parent().ok_or_else(|| {
        StoreError::Backend(format!(
            "persisted database config `{}` has no parent directory to write through",
            config_path.display()
        ))
    })?;
    let tmp_path = dir.join(CONFIG_ADOPT_TMP);
    let serialized = serde_json::to_vec_pretty(config).map_err(|error| {
        StoreError::Backend(format!(
            "cannot serialize the adopted database config for `{}`: {error}",
            config_path.display()
        ))
    })?;
    let mut file = std::fs::File::create(&tmp_path).map_err(|error| {
        StoreError::Backend(format!(
            "cannot create adoption scratch file `{}`: {error}",
            tmp_path.display()
        ))
    })?;
    file.write_all(&serialized)
        .and_then(|()| file.sync_all())
        .map_err(|error| {
            StoreError::Backend(format!(
                "cannot write adoption scratch file `{}`: {error}",
                tmp_path.display()
            ))
        })?;
    drop(file);
    std::fs::rename(&tmp_path, config_path).map_err(|error| {
        StoreError::Backend(format!(
            "cannot rename adopted database config `{}` over `{}`: {error}",
            tmp_path.display(),
            config_path.display()
        ))
    })?;
    std::fs::File::open(dir)
        .and_then(|handle| handle.sync_all())
        .map_err(|error| {
            StoreError::Backend(format!(
                "cannot fsync `{}` after adopting the database config: {error}",
                dir.display()
            ))
        })
}

#[cfg(test)]
mod tests {
    use super::{BudgetAdoption, adopt_node_cache_budget};
    use aion_store::StoreError;
    use haematite::NodeCacheBudget;

    /// The LITERAL persisted shape of the 2026-08-18 exhibit store (aion#75):
    /// every field a pre-budget haematite recorded, and no `node_cache_budget`.
    const PRE_BUDGET_CONFIG: &str = r#"{
    "format_version": 2,
    "chunking_policy": {
        "chunking": "bytes-v2",
        "leaf_target_bytes": 65536,
        "internal_target_bytes": 49152
    },
    "data_dir": "/Users/tom/.aion/data",
    "shard_count": 64,
    "distributed": null,
    "executor_threads": null
}"#;

    fn budget() -> NodeCacheBudget {
        match NodeCacheBudget::bytes(1 << 30) {
            Ok(budget) => budget,
            Err(error) => unreachable!("1 GiB is a valid byte budget: {error}"),
        }
    }

    fn read_config(dir: &std::path::Path) -> serde_json::Value {
        let bytes = match std::fs::read(dir.join("config.json")) {
            Ok(bytes) => bytes,
            Err(error) => unreachable!("config.json must exist after the test arrange: {error}"),
        };
        match serde_json::from_slice(&bytes) {
            Ok(value) => value,
            Err(error) => unreachable!("config.json must hold valid JSON: {error}"),
        }
    }

    #[test]
    fn adopts_into_the_exhibits_literal_pre_budget_config() -> Result<(), StoreError> {
        let dir = tempfile::tempdir().map_err(|error| StoreError::Backend(error.to_string()))?;
        std::fs::write(dir.path().join("config.json"), PRE_BUDGET_CONFIG)
            .map_err(|error| StoreError::Backend(error.to_string()))?;

        let outcome = adopt_node_cache_budget(dir.path(), budget())?;
        assert_eq!(outcome, BudgetAdoption::Adopted);

        let config = read_config(dir.path());
        assert_eq!(
            config.get("node_cache_budget"),
            Some(&serde_json::json!({ "bytes": 1_073_741_824_u64 })),
            "the caller's ruling must be recorded"
        );
        // Every pre-existing field survives the adoption untouched.
        assert_eq!(
            config.get("shard_count"),
            Some(&serde_json::json!(64_u64)),
            "recorded fields must be preserved"
        );
        assert_eq!(
            config
                .get("chunking_policy")
                .and_then(|policy| policy.get("leaf_target_bytes")),
            Some(&serde_json::json!(65_536_u64)),
            "nested recorded fields must be preserved"
        );
        Ok(())
    }

    #[test]
    fn a_recorded_ruling_wins_and_the_file_is_not_touched() -> Result<(), StoreError> {
        let dir = tempfile::tempdir().map_err(|error| StoreError::Backend(error.to_string()))?;
        let recorded = r#"{"shard_count": 4, "node_cache_budget": {"bytes": 555}}"#;
        let path = dir.path().join("config.json");
        std::fs::write(&path, recorded).map_err(|error| StoreError::Backend(error.to_string()))?;

        let outcome = adopt_node_cache_budget(dir.path(), budget())?;
        assert_eq!(outcome, BudgetAdoption::AlreadyRuled);

        let bytes = std::fs::read(&path).map_err(|error| StoreError::Backend(error.to_string()))?;
        assert_eq!(
            bytes,
            recorded.as_bytes(),
            "a config that already rules must stay byte-identical — the recorded \
             ruling wins over the caller's differing value"
        );
        Ok(())
    }

    #[test]
    fn an_explicit_null_ruling_is_nobody_ruled_and_is_adopted() -> Result<(), StoreError> {
        let dir = tempfile::tempdir().map_err(|error| StoreError::Backend(error.to_string()))?;
        std::fs::write(
            dir.path().join("config.json"),
            r#"{"shard_count": 1, "node_cache_budget": null}"#,
        )
        .map_err(|error| StoreError::Backend(error.to_string()))?;

        let outcome = adopt_node_cache_budget(dir.path(), budget())?;
        assert_eq!(outcome, BudgetAdoption::Adopted);
        assert_eq!(
            read_config(dir.path()).get("node_cache_budget"),
            Some(&serde_json::json!({ "bytes": 1_073_741_824_u64 })),
            "an explicit null deserializes as None and would still refuse — it must \
             be adopted over, not treated as a ruling"
        );
        Ok(())
    }

    #[test]
    fn adoption_is_idempotent_across_reopens() -> Result<(), StoreError> {
        let dir = tempfile::tempdir().map_err(|error| StoreError::Backend(error.to_string()))?;
        std::fs::write(dir.path().join("config.json"), PRE_BUDGET_CONFIG)
            .map_err(|error| StoreError::Backend(error.to_string()))?;

        assert_eq!(
            adopt_node_cache_budget(dir.path(), budget())?,
            BudgetAdoption::Adopted
        );
        let after_first = std::fs::read(dir.path().join("config.json"))
            .map_err(|error| StoreError::Backend(error.to_string()))?;
        assert_eq!(
            adopt_node_cache_budget(dir.path(), budget())?,
            BudgetAdoption::AlreadyRuled
        );
        let after_second = std::fs::read(dir.path().join("config.json"))
            .map_err(|error| StoreError::Backend(error.to_string()))?;
        assert_eq!(
            after_first, after_second,
            "the second open must not rewrite the file"
        );
        Ok(())
    }

    #[test]
    fn corrupt_config_is_refused_and_left_untouched() -> Result<(), StoreError> {
        let dir = tempfile::tempdir().map_err(|error| StoreError::Backend(error.to_string()))?;
        let path = dir.path().join("config.json");
        let corrupt = b"{ this is not json";
        std::fs::write(&path, corrupt).map_err(|error| StoreError::Backend(error.to_string()))?;

        let error = match adopt_node_cache_budget(dir.path(), budget()) {
            Err(error) => error,
            Ok(outcome) => unreachable!("corrupt JSON must refuse, got {outcome:?}"),
        };
        assert!(
            error.to_string().contains("config.json"),
            "the refusal must name the file: {error}"
        );
        let bytes = std::fs::read(&path).map_err(|error| StoreError::Backend(error.to_string()))?;
        assert_eq!(
            bytes,
            corrupt.as_slice(),
            "a corrupt file is the operator's to repair — the migration must not write"
        );
        Ok(())
    }

    #[test]
    fn a_non_object_config_is_refused() -> Result<(), StoreError> {
        let dir = tempfile::tempdir().map_err(|error| StoreError::Backend(error.to_string()))?;
        std::fs::write(dir.path().join("config.json"), "[1, 2, 3]")
            .map_err(|error| StoreError::Backend(error.to_string()))?;

        let error = match adopt_node_cache_budget(dir.path(), budget()) {
            Err(error) => error,
            Ok(outcome) => unreachable!("a non-object config must refuse, got {outcome:?}"),
        };
        assert!(
            error.to_string().contains("not a JSON object"),
            "the refusal must say what is wrong: {error}"
        );
        Ok(())
    }

    #[test]
    fn a_missing_config_file_is_a_typed_error() {
        let dir = match tempfile::tempdir() {
            Ok(dir) => dir,
            Err(error) => unreachable!("tempdir: {error}"),
        };
        let error = match adopt_node_cache_budget(dir.path(), budget()) {
            Err(error) => error,
            Ok(outcome) => unreachable!(
                "adoption over a directory with no config.json must refuse, got {outcome:?}"
            ),
        };
        assert!(
            error.to_string().contains("cannot read"),
            "the refusal must name the failing step: {error}"
        );
    }
}
