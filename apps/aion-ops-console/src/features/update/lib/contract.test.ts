import { describe, expect, test } from 'bun:test';

import { UPDATE_CHECK_WORKFLOW_TYPE, updateCheckStartParams } from './contract';

describe('the update-check contract literal', () => {
  /**
   * The console half of the duplicated name (see contract.ts): the server's
   * embedded document compiles to `update_check`, verified at load on the
   * Rust side (`crates/aion-server/src/update_check/document.rs`). This pin
   * gives a CONSOLE-side drift a named place to fail; a Rust-side rename
   * must edit contract.ts and this expectation together, as one change.
   */
  test('names the workflow type the embedded document compiles to', () => {
    expect(UPDATE_CHECK_WORKFLOW_TYPE).toBe('update_check');
  });

  /**
   * The start must carry an explicit empty `input`: the start route refuses a
   * payload-less start outright ("payload is missing"), which the r1 live e2e
   * measured against the real route. A check-now that omits `input` is a
   * button that can never work.
   */
  test('the start parameters carry the explicit empty input object', () => {
    expect(updateCheckStartParams()).toEqual({
      workflowType: 'update_check',
      input: {},
    });
  });
});
