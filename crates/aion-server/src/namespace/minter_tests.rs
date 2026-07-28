//! Unit tests for [`super::NamespaceMinter`] — the `Created`-edge delta
//! discipline of the single mint choke-point.
//!
//! A sibling file rather than an inline `mod tests`, so `minter.rs` stays under
//! the 500-line law without the test bodies being thinned.

use std::num::NonZeroUsize;
use std::sync::Arc;

use aion_core::ClusterEvent;
use aion_store::{InMemoryStore, NamespaceOrigin, NamespaceStore};
use futures::StreamExt;

use super::NamespaceMinter;
use crate::cluster_publisher::ClusterEventPublisher;
use crate::config::AutoCreate;

type TestResult<T> = Result<T, Box<dyn std::error::Error>>;

fn publisher() -> TestResult<ClusterEventPublisher> {
    let capacity = NonZeroUsize::new(16).ok_or("delta channel capacity must be non-zero")?;
    Ok(ClusterEventPublisher::new(capacity))
}

fn open_minter(store: Arc<InMemoryStore>, publisher: ClusterEventPublisher) -> NamespaceMinter {
    let store: Arc<dyn NamespaceStore> = store;
    NamespaceMinter::new(store, AutoCreate::Open).with_cluster_publisher(publisher)
}

/// Pull the next delta off the stream, asserting it is a `NamespaceCreated`
/// with the `explicit` origin label and returning its name.
async fn next_created_name<S>(deltas: &mut S) -> TestResult<String>
where
    S: futures::Stream<Item = Result<ClusterEvent, crate::cluster_publisher::ClusterStreamLagged>>
        + Unpin,
{
    let event = deltas
        .next()
        .await
        .ok_or("expected a namespace-created delta")?
        .map_err(|lag| format!("unexpected lag: {lag:?}"))?;
    match event {
        ClusterEvent::NamespaceCreated { name, origin, .. } => {
            assert_eq!(origin, "explicit");
            Ok(name)
        }
        other => Err(format!("expected NamespaceCreated, got {other:?}").into()),
    }
}

/// The single `MintOutcome::Created` choke-point pushes exactly one durable
/// `NamespaceCreated` delta carrying the record's name + origin label, and an
/// idempotent re-reference of the SAME namespace (an `AlreadyExisted` touch)
/// pushes NOTHING — so the ops console appends each namespace exactly once
/// with no refresh and no duplicate row.
#[tokio::test]
async fn namespace_created_delta_emits_once_on_created_and_not_on_already_existed() -> TestResult<()>
{
    let store = Arc::new(InMemoryStore::default());
    let publisher = publisher()?;
    let mut deltas = publisher.subscribe(0);
    let minter = open_minter(Arc::clone(&store), publisher);

    // First mint of a brand-new namespace: the Created edge.
    minter
        .mint_or_gate(&["orders".to_owned()], NamespaceOrigin::WorkerMint)
        .await?;

    let first = deltas
        .next()
        .await
        .ok_or("expected one namespace-created delta")?
        .map_err(|lag| format!("unexpected lag: {lag:?}"))?;
    match first {
        ClusterEvent::NamespaceCreated {
            name,
            origin,
            created_at,
            ..
        } => {
            assert_eq!(name, "orders");
            assert_eq!(origin, "worker_mint");
            // The carried instant is the durable record's own created_at.
            let record = store
                .get_namespace("orders")
                .await?
                .ok_or("record must exist after a Created mint")?;
            assert_eq!(created_at, record.created_at);
        }
        other => return Err(format!("expected NamespaceCreated, got {other:?}").into()),
    }

    // Idempotent re-reference of the SAME namespace: an AlreadyExisted touch.
    // It must NOT emit a second delta. A different new namespace must, so we
    // can prove the channel is still live (the re-reference produced silence,
    // not a closed channel).
    minter
        .mint_or_gate(&["orders".to_owned()], NamespaceOrigin::WorkerMint)
        .await?;
    minter
        .mint_or_gate(&["billing".to_owned()], NamespaceOrigin::StartMint)
        .await?;

    let next = deltas
        .next()
        .await
        .ok_or("expected the second namespace's delta")?
        .map_err(|lag| format!("unexpected lag: {lag:?}"))?;
    match next {
        ClusterEvent::NamespaceCreated { name, origin, .. } => {
            // The very next delta is `billing`, proving the `orders`
            // re-reference emitted nothing in between (idempotent silence).
            assert_eq!(name, "billing");
            assert_eq!(origin, "start_mint");
        }
        other => return Err(format!("expected NamespaceCreated, got {other:?}").into()),
    }

    Ok(())
}

/// The explicit `POST /namespaces` path flows through the SAME choke-point, so
/// an operator-minted namespace emits the `NamespaceCreated` delta once on
/// create and is silent on an idempotent re-create.
#[tokio::test]
async fn explicit_create_emits_created_delta_once_then_silent_on_recreate() -> TestResult<()> {
    let store = Arc::new(InMemoryStore::default());
    let publisher = publisher()?;
    let mut deltas = publisher.subscribe(0);
    let minter = open_minter(Arc::clone(&store), publisher);

    let created = minter.create_explicit("tenant-a").await?;
    assert_eq!(created, aion_store::MintOutcome::Created);
    // Idempotent re-create: AlreadyExisted, and no second delta.
    let again = minter.create_explicit("tenant-a").await?;
    assert_eq!(again, aion_store::MintOutcome::AlreadyExisted);

    // Emit one more genuinely-new namespace to bound the read: the next delta
    // proves the re-create was silent.
    let _ = minter.create_explicit("tenant-b").await?;

    let first = next_created_name(&mut deltas).await?;
    let second = next_created_name(&mut deltas).await?;
    assert_eq!(
        vec![first, second],
        vec!["tenant-a".to_owned(), "tenant-b".to_owned()]
    );

    Ok(())
}

/// Without a publisher attached the minter is silent (durable record + audit
/// event only): the registry's other call sites that never wire the channel
/// stay byte-identical, and minting never depends on a live subscriber.
#[tokio::test]
async fn mint_without_publisher_creates_record_but_emits_no_delta() -> TestResult<()> {
    let store: Arc<dyn NamespaceStore> = Arc::new(InMemoryStore::default());
    let minter = NamespaceMinter::new(Arc::clone(&store), AutoCreate::Open);

    minter
        .mint_or_gate(&["orders".to_owned()], NamespaceOrigin::WorkerMint)
        .await?;

    assert!(
        store.get_namespace("orders").await?.is_some(),
        "the durable record is still minted without a publisher"
    );
    Ok(())
}
