import type { JsonRecord } from '@/lib/api';
import type { WorkflowFilter } from '@/types';

/**
 * The assistant workflow contract (built in the parallel backend track as a
 * Gleam package). One place pins the type + signal names + input shape so the
 * UI and the workflow can only drift in one visible spot.
 */
export const ASSISTANT_WORKFLOW_TYPE = 'assistant';

/**
 * The ONE control signal the session listens on — the engine's selective
 * receive parks on exactly this name, discriminating in the payload. There is
 * deliberately NO separate end signal.
 *
 * The workflow declares `type Continuation { message: String, end: Bool }`.
 * NEITHER field is optional, so the package contract rejects a payload carrying
 * only one of them. Sending `{ message }` alone was refused with `"end" is a
 * required property` and `{ end: true }` alone would have been refused with the
 * mirror of it — the console could neither continue a session nor finish one.
 *
 * Build every payload through the two functions below and never inline an
 * object literal at a call site; `assistant.contract.test.ts` reads the field
 * list off the workflow document itself and fails if either builder drifts
 * from it.
 */
export const ASSISTANT_CONTINUE_SIGNAL = 'assistant_continue';

/**
 * Continue the session with the operator's next prompt.
 *
 * `end: false` is not padding — the contract requires the field, and its value
 * is the discriminator that keeps the session open.
 */
export function assistantContinuePayload(message: string): JsonRecord {
  return { message, end: false };
}

/**
 * Finish the session cleanly as `OperatorEnded`.
 *
 * The empty message is the workflow's own documented no-op: "Blank is a no-op
 * nudge that does not spend a round." Ending carries no prompt, but the field
 * is still required, so it travels blank rather than absent.
 */
export function assistantEndPayload(): JsonRecord {
  return { message: '', end: true };
}

/** The `POST /workflows/start` input for an assistant session. */
export type AssistantSessionInput = {
  objective: string;
  repo_path: string;
};

export function assistantStartInput(objective: string, repoPath: string): JsonRecord {
  // The contract shape carries repo_path unconditionally; a blank entry travels
  // as the empty string (the workflow treats it as "no repo pinned").
  const input: AssistantSessionInput = { objective, repo_path: repoPath };
  return input;
}

/** The session-list filter: exactly the assistant workflow type, any status. */
export function assistantSessionsFilter(): WorkflowFilter {
  return {
    workflow_type: ASSISTANT_WORKFLOW_TYPE,
    status: null,
    started_after: null,
    started_before: null,
    parent: null,
  };
}
