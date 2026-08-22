import { expect, test } from 'bun:test';

import { DISPLAY_NAME_ATTRIBUTE, type SearchAttributes } from '@/lib/workflowIdentity';
import {
  classifyKnownEvent,
  type EventVariantFamily,
  KNOWN_EVENT_TYPES,
  type KnownEventType,
  searchAttributesSummary,
} from '../lib/timelineVariants';

// The COUNT is deliberately not asserted. A hardcoded number is a second copy
// of a fact the generated union already states, and it goes stale the moment a
// variant lands — which is how it read as a failure about arithmetic rather
// than about a missing handler. What matters is that the list has no duplicate
// and no gap; the gap is proved at compile time by `ListIsExhaustive`, which
// only bites now that the list is `as const`.
test('KNOWN_EVENT_TYPES lists every generated variant exactly once', () => {
  expect(new Set(KNOWN_EVENT_TYPES).size).toBe(KNOWN_EVENT_TYPES.length);
});

test('classifyKnownEvent maps every known variant to a family + sub-kind', () => {
  for (const eventType of KNOWN_EVENT_TYPES) {
    const descriptor = classifyKnownEvent(eventType);
    expect(descriptor).not.toBeNull();
    expect(descriptor?.subKind).toBe(eventType);
  }
});

test('classifyKnownEvent assigns the expected families', () => {
  const expectations: Record<KnownEventType, EventVariantFamily> = {
    WorkflowStarted: 'lifecycle',
    WorkflowCompleted: 'lifecycle',
    WorkflowFailed: 'lifecycle',
    WorkflowCancelled: 'lifecycle',
    WorkflowTimedOut: 'lifecycle',
    WorkflowContinuedAsNew: 'marker',
    WorkflowReopened: 'marker',
    WorkflowPaused: 'marker',
    WorkflowResumed: 'marker',
    SearchAttributesUpdated: 'marker',
    ActivityScheduled: 'activity',
    ActivityStarted: 'activity',
    ActivityCompleted: 'activity',
    ActivityFailed: 'activity',
    ActivityAdvisoryExhausted: 'activity',
    ActivityFallbackRouted: 'activity',
    ActivityAdoptionOffered: 'activity',
    ActivityCancelled: 'activity',
    TimerStarted: 'timer',
    TimerFired: 'timer',
    TimerCancelled: 'timer',
    WithTimeoutCompleted: 'with-timeout',
    SignalReceived: 'signal',
    SignalSent: 'signal',
    ChildWorkflowStarted: 'child',
    ChildWorkflowCompleted: 'child',
    ChildWorkflowFailed: 'child',
    ChildWorkflowCancelled: 'child',
    ScheduleCreated: 'schedule',
    ScheduleUpdated: 'schedule',
    SchedulePaused: 'schedule',
    ScheduleResumed: 'schedule',
    ScheduleDeleted: 'schedule',
    ScheduleTriggered: 'schedule',
  };

  for (const [eventType, family] of Object.entries(expectations)) {
    expect(classifyKnownEvent(eventType)?.family).toBe(family);
  }
});

// #211. Catches: a naming update rendering as a generic key count, so a rename
// is invisible in the timeline that is supposed to record it.
test('a display-name update reads as the naming it is', () => {
  const attributes: SearchAttributes = {
    [DISPLAY_NAME_ATTRIBUTE]: { type: 'String', data: 'nightly digest' },
  };

  expect(searchAttributesSummary(attributes)).toBe('Display name set to "nightly digest"');
});

// Catches: the display-name arm swallowing the other attributes in the same
// recorded update, which would under-report what the event actually changed.
test('other attributes recorded alongside the name are still reported', () => {
  const attributes: SearchAttributes = {
    [DISPLAY_NAME_ATTRIBUTE]: { type: 'String', data: 'nightly digest' },
    customer: { type: 'String', data: 'acme' },
    region: { type: 'String', data: 'apse2' },
  };

  expect(searchAttributesSummary(attributes)).toBe(
    'Display name set to "nightly digest" (+2 other key(s))'
  );
});

// Catches: the generic case regressing — an update that never touched the name
// must keep reading as an ordinary search-attribute update.
test('an update with no display name keeps the generic key-count label', () => {
  expect(searchAttributesSummary({ customer: { type: 'String', data: 'acme' } })).toBe(
    'Search attributes updated (1 key(s))'
  );
  expect(searchAttributesSummary({})).toBe('Search attributes updated (0 key(s))');
});

// Catches: coercing a non-String value into a label. The server only ever
// records the name as a String, so anything else is foreign data, not a name.
test('a non-String value under the name attribute is not a name', () => {
  expect(searchAttributesSummary({ [DISPLAY_NAME_ATTRIBUTE]: { type: 'Int', data: 7 } })).toBe(
    'Search attributes updated (1 key(s))'
  );
});

test('classifyKnownEvent returns null for a genuinely-unknown future variant', () => {
  // A variant the server might add before the wire types are regenerated. The
  // runtime path must degrade safely (null), never throw.
  expect(classifyKnownEvent('SomeFutureUnknownVariant')).toBeNull();
});
