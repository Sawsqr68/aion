import { expect, test } from 'bun:test';

import type { ClusterEvent, ClusterSnapshot } from '@/types';

import {
  applyClusterEvent,
  applyClusterSnapshot,
  createClusterReducerState,
  projectClusterTopology,
} from './useClusterStream';

const SNAPSHOT: ClusterSnapshot = {
  node: 'node-a',
  as_of_seq: 1,
  peers: [],
  shards: [],
  workers: [],
  deployments: [
    {
      name: 'shells',
      desired_state: 'running',
      binary_version: '0.12.0',
      binary_content_hash: 'hash-a',
    },
  ],
};

type EventWithoutMeta = ClusterEvent extends infer Variant
  ? Variant extends ClusterEvent
    ? Omit<Variant, 'meta'>
    : never
  : never;

function event(event: EventWithoutMeta, clusterSeq: number): ClusterEvent {
  return {
    ...event,
    meta: {
      cluster_seq: clusterSeq,
      observed_at: `2026-08-08T12:00:0${clusterSeq}Z`,
    },
  } as ClusterEvent;
}

test('deployment put, desired-state, and delete deltas reduce onto the snapshot', () => {
  const state = createClusterReducerState();
  applyClusterSnapshot(state, SNAPSHOT);

  expect(projectClusterTopology(state).deployments).toEqual(SNAPSHOT.deployments);

  expect(
    applyClusterEvent(
      state,
      event(
        {
          type: 'WorkerDeploymentDesiredStateChanged',
          name: 'shells',
          desired_state: 'stopped',
        },
        2
      )
    )
  ).toBeTrue();
  expect(projectClusterTopology(state).deployments[0]?.desired_state).toBe('stopped');

  expect(
    applyClusterEvent(
      state,
      event(
        {
          type: 'WorkerDeploymentPut',
          name: 'norn',
          outcome: 'created',
          desired_state: 'running',
          binary_version: '0.12.0',
          binary_content_hash: 'hash-b',
        },
        3
      )
    )
  ).toBeTrue();
  expect(projectClusterTopology(state).deployments.map((deployment) => deployment.name)).toEqual([
    'norn',
    'shells',
  ]);

  expect(
    applyClusterEvent(state, event({ type: 'WorkerDeploymentDeleted', name: 'shells' }, 4))
  ).toBeTrue();
  expect(projectClusterTopology(state).deployments).toEqual([
    {
      name: 'norn',
      desired_state: 'running',
      binary_version: '0.12.0',
      binary_content_hash: 'hash-b',
    },
  ]);
});
