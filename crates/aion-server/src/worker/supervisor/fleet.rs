//! The managed-worker supervisor: desired state in, running processes out.
//!
//! Desired state is the DURABLE record (`WorkerDeployment.desired`); actual
//! state is a live [`InstanceHandle`]. This type is the only thing that reads
//! one and writes the other, and every answer it gives is a join of the two —
//! never one standing in for the other.
//!
//! Supervision is COMMISSIONED, not defaulted. A server whose operator has not
//! written a `[worker_supervision]` policy supervises nothing and says so with
//! the remedy attached; it does not pick a backoff on the operator's behalf
//! (ADR-001).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock};

use aion_core::ClusterEvent;
use aion_store::{
    DesiredState, WorkerArtifactRef, WorkerDeployment, WorkerDeploymentListing,
    WorkerDeploymentStore,
};

use crate::cluster_publisher::ClusterEventPublisher;

use super::error::SupervisionError;
use super::executable::ManagedExecutable;
use super::instance::{InstanceConfig, InstanceHandle, InstanceSnapshot};
use super::policy::{SupervisionPolicy, UNCOMMISSIONED_REMEDY};
use super::status::{ManagedWorkerReport, ManagedWorkerState, ManagedWorkerStatus};

/// What an operator commissioned: how to supervise, and what to launch.
///
/// Both halves arrive together and neither can be swapped afterwards. They
/// belong to the same decision — a restart discipline is meaningless without
/// knowing what is being restarted — and holding them in one write-once cell
/// is what makes "commissioned" a single, checkable fact.
#[derive(Clone, Debug)]
struct Commission {
    policy: SupervisionPolicy,
    executable: ManagedExecutable,
}

/// Server-owned supervision of the managed worker fleet.
pub struct WorkerSupervisor {
    store: Arc<dyn WorkerDeploymentStore>,
    commission: OnceLock<Commission>,
    instances: Mutex<BTreeMap<String, InstanceHandle>>,
    publisher: ClusterEventPublisher,
}

impl WorkerSupervisor {
    /// Build an UNCOMMISSIONED supervisor over the durable deployment store,
    /// publishing desired-state changes onto the deployment-global cluster
    /// channel.
    ///
    /// The publisher lives HERE — not in the transports — so every desired-state
    /// write the supervisor makes ([`Self::start`], [`Self::stop`], and
    /// [`Self::restart`] through them) reaches the console's live feed
    /// identically whichever transport asked, exactly as the
    /// worker-deployment desired-state endpoint publishes its own writes.
    ///
    /// Nothing is supervised until [`Self::commission`] installs an operator
    /// policy: construction is not commissioning, so a server that boots
    /// without the config section never spawns anything.
    #[must_use]
    pub fn new(store: Arc<dyn WorkerDeploymentStore>, publisher: ClusterEventPublisher) -> Self {
        Self {
            store,
            commission: OnceLock::new(),
            instances: Mutex::new(BTreeMap::new()),
            publisher,
        }
    }

    /// Install the operator's supervision policy and the executable a
    /// `builtin` deployment means on this server.
    ///
    /// Returns false when a commission was already installed, in which case
    /// the existing one stands: the restart discipline of a running fleet is
    /// not something a later caller gets to swap out underneath it.
    pub fn commission(&self, policy: SupervisionPolicy, executable: ManagedExecutable) -> bool {
        self.commission
            .set(Commission { policy, executable })
            .is_ok()
    }

    /// The installed policy, if this server has one.
    #[must_use]
    pub fn policy(&self) -> Option<SupervisionPolicy> {
        self.commission.get().map(|commission| commission.policy)
    }

    /// Whether an operator commission is installed.
    #[must_use]
    pub fn is_commissioned(&self) -> bool {
        self.commission.get().is_some()
    }

    fn require_commission(&self) -> Result<&Commission, SupervisionError> {
        self.commission
            .get()
            .ok_or(SupervisionError::NotCommissioned)
    }

    fn instances(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, BTreeMap<String, InstanceHandle>>, SupervisionError> {
        self.instances
            .lock()
            .map_err(|poison| SupervisionError::StatePoisoned {
                detail: poison.to_string(),
            })
    }

    async fn record(&self, name: &str) -> Result<WorkerDeployment, SupervisionError> {
        self.store
            .get_worker_deployment(name)
            .await
            .map_err(|source| SupervisionError::Store { source })?
            .ok_or_else(|| SupervisionError::UnknownDeployment {
                name: name.to_owned(),
            })
    }

    async fn listing(&self) -> Result<WorkerDeploymentListing, SupervisionError> {
        self.store
            .list_worker_deployments()
            .await
            .map_err(|source| SupervisionError::Store { source })
    }

    /// Start (or adopt) supervision of one deployment, recording the intent
    /// durably first. A durable desired-state flip is published onto the
    /// cluster channel, so the console's live feed sees it whichever transport
    /// asked.
    ///
    /// Idempotent: a deployment already being supervised is reported as it is,
    /// without a second process (and, already desiring `Running`, without a
    /// second event).
    ///
    /// # Errors
    ///
    /// Returns [`SupervisionError::NotCommissioned`] when no policy is
    /// installed — carrying the remedy — [`SupervisionError::UnknownDeployment`]
    /// for a name with no durable record, and [`SupervisionError::Store`] when
    /// the record cannot be read or the intent cannot be written.
    pub async fn start(&self, name: &str) -> Result<ManagedWorkerStatus, SupervisionError> {
        let commission = self.require_commission()?.clone();
        let record = self.record(name).await?;
        let record = if record.desired == DesiredState::Running {
            record
        } else {
            let record = self
                .store
                .set_desired_state(name, DesiredState::Running)
                .await
                .map_err(|source| SupervisionError::Store { source })?
                .ok_or_else(|| SupervisionError::UnknownDeployment {
                    name: name.to_owned(),
                })?;
            self.publish_desired_state(&record);
            record
        };

        let snapshot = {
            let mut instances = self.instances()?;
            let live = instances
                .get(name)
                .is_some_and(|handle| !handle.is_finished());
            if !live {
                let handle = InstanceHandle::start(self.instance_config(&record, &commission));
                drop(instances.insert(record.name.clone(), handle));
            }
            instances
                .get(name)
                .map(InstanceHandle::snapshot)
                .transpose()?
        };
        Ok(status_of(&record, snapshot, self.is_commissioned()))
    }

    /// Stop one deployment and record the intent durably. The durable
    /// desired-state write is published onto the cluster channel, so the
    /// console's live feed sees it whichever transport asked.
    ///
    /// The returned status is written only once the process group has been
    /// probed empty; a group that survives the termination ladder produces
    /// [`SupervisionError::StopIncomplete`] instead, so a caller can never read
    /// "stopped" off bookkeeping alone.
    ///
    /// # Errors
    ///
    /// Returns [`SupervisionError::UnknownDeployment`] for an absent record,
    /// [`SupervisionError::Store`] when the intent cannot be written,
    /// [`SupervisionError::StopIncomplete`] when the group cannot be proven
    /// empty, and [`SupervisionError::TaskLost`] when the supervision task
    /// ended abnormally.
    pub async fn stop(&self, name: &str) -> Result<ManagedWorkerStatus, SupervisionError> {
        let record = self
            .store
            .set_desired_state(name, DesiredState::Stopped)
            .await
            .map_err(|source| SupervisionError::Store { source })?
            .ok_or_else(|| SupervisionError::UnknownDeployment {
                name: name.to_owned(),
            })?;
        self.publish_desired_state(&record);
        let handle = self.instances()?.remove(name);
        if let Some(handle) = handle {
            handle.stop(name).await?;
        }
        Ok(status_of(&record, None, self.is_commissioned()))
    }

    /// Stop and start one deployment, leaving desired state at `Running`.
    ///
    /// Publishes exactly what it durably writes: a restart of an
    /// already-`Running` deployment changes no desired state and emits no
    /// desired-state event; one that flips it inherits [`Self::start`]'s.
    ///
    /// # Errors
    ///
    /// Returns the same failures as [`Self::stop`] and [`Self::start`].
    pub async fn restart(&self, name: &str) -> Result<ManagedWorkerStatus, SupervisionError> {
        self.require_commission()?;
        self.record(name).await?;
        let handle = self.instances()?.remove(name);
        if let Some(handle) = handle {
            handle.stop(name).await?;
        }
        self.start(name).await
    }

    /// Bring the fleet to its durable desired state.
    ///
    /// Called at boot and safe to call again: deployments already supervised
    /// are left alone. Returns the number of deployments now supervised.
    ///
    /// # Errors
    ///
    /// Returns [`SupervisionError::NotCommissioned`] when no policy is
    /// installed, and [`SupervisionError::Store`] when the durable listing
    /// cannot be read.
    pub async fn reconcile(&self) -> Result<usize, SupervisionError> {
        let commission = self.require_commission()?.clone();
        let listing = self.listing().await?;
        let mut supervised = 0_usize;
        for record in listing
            .deployments
            .iter()
            .filter(|record| record.desired == DesiredState::Running)
        {
            let mut instances = self.instances()?;
            let live = instances
                .get(&record.name)
                .is_some_and(|handle| !handle.is_finished());
            if !live {
                let handle = InstanceHandle::start(self.instance_config(record, &commission));
                drop(instances.insert(record.name.clone(), handle));
            }
            supervised = supervised.saturating_add(1);
        }
        for poisoned in &listing.undecodable {
            tracing::error!(
                worker = poisoned.name.as_str(),
                error = poisoned.error.as_str(),
                "worker deployment record could not be decoded; it is not being supervised"
            );
        }
        Ok(supervised)
    }

    /// Join durable intent with live supervision for every deployment.
    ///
    /// # Errors
    ///
    /// Returns [`SupervisionError::Store`] when the durable listing cannot be
    /// read, and [`SupervisionError::StatePoisoned`] when a status cell was
    /// poisoned by a panicking holder.
    pub async fn report(&self) -> Result<ManagedWorkerReport, SupervisionError> {
        let listing = self.listing().await?;
        let commissioned = self.is_commissioned();
        let mut workers = Vec::with_capacity(listing.deployments.len());
        {
            let instances = self.instances()?;
            for record in &listing.deployments {
                let snapshot = instances
                    .get(&record.name)
                    .map(InstanceHandle::snapshot)
                    .transpose()?;
                workers.push(status_of(record, snapshot, commissioned));
            }
        }
        Ok(ManagedWorkerReport {
            commissioned,
            remedy: (!commissioned).then(|| UNCOMMISSIONED_REMEDY.to_owned()),
            workers,
            undecodable: listing
                .undecodable
                .iter()
                .map(|poisoned| poisoned.name.clone())
                .collect(),
        })
    }

    /// Stop every supervised instance, for server shutdown.
    ///
    /// Durable desired state is deliberately NOT changed: a server going down
    /// is not an operator asking for the fleet to stay down, and the next boot
    /// reconciles it back up. Returns one failure per instance that could not
    /// be proven stopped — an empty vector is the proof that no worker was
    /// orphaned.
    pub async fn shutdown(&self) -> Vec<SupervisionError> {
        let handles = match self.instances() {
            Ok(mut instances) => std::mem::take(&mut *instances),
            Err(error) => return vec![error],
        };
        let mut failures = Vec::new();
        for (name, handle) in handles {
            if let Err(error) = handle.stop(&name).await {
                failures.push(error);
            }
        }
        failures
    }

    /// Publish one durable desired-state write onto the cluster channel — the
    /// SAME event the worker-deployment desired-state endpoint publishes for
    /// its writes, so the live feed cannot tell the transports apart.
    ///
    /// Called exactly where a write happened, never speculatively: an event
    /// here is proof of a durable change. The emitted event's return value is
    /// dropped deliberately — a channel with no subscribers is the calm state,
    /// not a failure.
    fn publish_desired_state(&self, record: &WorkerDeployment) {
        let name = record.name.clone();
        let desired_state = record.desired;
        drop(
            self.publisher
                .emit(|meta| ClusterEvent::WorkerDeploymentDesiredStateChanged {
                    meta,
                    name,
                    desired_state,
                }),
        );
    }

    fn instance_config(
        &self,
        record: &WorkerDeployment,
        commission: &Commission,
    ) -> InstanceConfig {
        let WorkerArtifactRef::Builtin { verb } = &record.artifact;
        InstanceConfig {
            name: record.name.clone(),
            verb: verb.clone(),
            executable: commission.executable.clone(),
            policy: commission.policy,
            store: Arc::clone(&self.store),
        }
    }
}

/// Join one durable record with one live snapshot.
fn status_of(
    record: &WorkerDeployment,
    snapshot: Option<InstanceSnapshot>,
    commissioned: bool,
) -> ManagedWorkerStatus {
    // Three different absences, three different answers. An instance that
    // exists but has not spoken yet is STARTING; a server that could not have
    // supervised is UNSUPERVISED; only genuinely nothing-to-run is STOPPED.
    // Collapsing any of them into another is how a status surface comes to
    // report a terminal state about work that is under way.
    let supervised = snapshot.is_some();
    let snapshot = snapshot.unwrap_or_default();
    let state = snapshot.state.unwrap_or({
        if supervised {
            ManagedWorkerState::Starting
        } else if record.desired == DesiredState::Running && !commissioned {
            ManagedWorkerState::Uncommissioned
        } else {
            ManagedWorkerState::Stopped
        }
    });
    ManagedWorkerStatus {
        name: record.name.clone(),
        task_queue: record.task_queue.clone(),
        desired: record.desired,
        state,
        pid: snapshot.pid,
        process_group: snapshot.process_group,
        restarts: snapshot.restarts,
        last_exit: snapshot.last_exit,
        last_error: snapshot.last_error,
        deployed_binary: record.binary.clone(),
        spawn_binary: snapshot.spawn_binary,
    }
}
