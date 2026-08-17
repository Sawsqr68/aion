//! Unit tests for [`super::HaematiteStore::shard_for_namespace`].

use aion_store::StoreError;

use crate::HaematiteStore;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// A store over a fresh private temp dir with `shard_count` shards.
fn store(
    shard_count: usize,
) -> Result<(HaematiteStore, tempfile::TempDir), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    // A generous 1 GiB node-cache ceiling: roomy enough that no fixture here can
    // reach it, so these stay shard-routing tests rather than accidental
    // cache-eviction tests. Not a default — production states its own.
    let node_cache_budget = haematite::NodeCacheBudget::bytes(1 << 30)
        .map_err(|error| format!("test node cache budget rejected: {error}"))?;
    let store =
        HaematiteStore::create_with_shard_count(dir.path().join("db"), shard_count, node_cache_budget)
            .map_err(|error: StoreError| format!("store create failed: {error}"))?;
    Ok((store, dir))
}

/// THE measurement the failover start-path investigation turned on: with the
/// zero-config default namespace and a 3-shard cluster, the namespace registry
/// record for `default` lives on shard **1**. Node 0 of that cluster owns shard
/// 0, so its unsteered starts were fenced `NotOwner { shard: 1 }` forever —
/// which is why routing the mint (rather than the start) is the fix.
///
/// This is a hash-stability pin as much as a routing pin: if haematite's
/// `shard_for` ever changes, this constant moves and every cluster's namespace
/// placement moves with it.
#[test]
fn default_namespace_lands_on_shard_one_at_three_shards() -> TestResult {
    let (store, _dir) = store(3)?;
    assert_eq!(
        store.shard_for_namespace("default"),
        1,
        "the investigation's measured constant: namespace_key(\"default\") % 3 == 1"
    );
    Ok(())
}

/// Contrast: a different namespace name hashes to a different shard, so the
/// accessor is genuinely name-dependent and not a constant. Both answers are
/// stable across repeated calls on the same store.
#[test]
fn distinct_namespaces_resolve_distinct_stable_shards() -> TestResult {
    let (store, _dir) = store(3)?;
    let default_shard = store.shard_for_namespace("default");
    let orders_shard = store.shard_for_namespace("orders");
    assert_eq!(default_shard, 1);
    assert_eq!(orders_shard, 2);
    assert_ne!(
        default_shard, orders_shard,
        "`default` and `orders` land on different registry shards, so the \
         accessor is genuinely name-dependent and one node cannot own the \
         registry for every namespace"
    );
    // Stable across repeated calls on the same store (a pure hash, no state).
    assert_eq!(default_shard, store.shard_for_namespace("default"));
    assert_eq!(orders_shard, store.shard_for_namespace("orders"));
    Ok(())
}

/// Every namespace shard is inside the store's shard space — the invariant a
/// routing decision built on this number depends on.
#[test]
fn namespace_shard_is_within_the_shard_count() -> TestResult {
    let (store, _dir) = store(3)?;
    for name in ["default", "tenant-a", "orders", "billing", ""] {
        let shard = store.shard_for_namespace(name);
        assert!(
            shard < store.shard_count(),
            "namespace {name:?} resolved shard {shard} outside the {}-shard space",
            store.shard_count()
        );
    }
    Ok(())
}

/// A single-shard store routes every namespace to shard 0 — the own-all boot
/// where the mint is always local and routing is never consulted.
#[test]
fn single_shard_store_routes_every_namespace_to_shard_zero() -> TestResult {
    let (store, _dir) = store(1)?;
    for name in ["default", "tenant-a", "orders"] {
        assert_eq!(store.shard_for_namespace(name), 0);
    }
    Ok(())
}
