/**
 * The workflow type "check now" starts.
 *
 * PLAINLY: this literal is a DUPLICATE of the Rust constant
 * `UPDATE_CHECK_WORKFLOW_TYPE` in
 * `crates/aion-server/src/update_check/document.rs`, and NO build step pins
 * the two together — TypeScript cannot see Rust, and nothing generates this
 * file. What each side has instead: the Rust constant's load-time
 * verification proves the embedded document compiles to ITS value (surfaced
 * at boot and in unit tests — a runtime check, not a build failure), its doc
 * comment names this file as the place a rename must also touch, and
 * `contract.test.ts` beside this file pins this literal so a console-side
 * drift has a named place to fail. A Rust-side rename that forgets this file
 * is caught by nothing but that doc comment — treat the two as one edit.
 */
export const UPDATE_CHECK_WORKFLOW_TYPE = 'update_check';

/**
 * The exact start parameters "check now" sends. The `input` is the EXPLICIT
 * empty object: the document declares no inputs, and the start route refuses
 * a start with no payload at all ("payload is missing", 500) — the live e2e
 * (`crates/aion-server/tests/update_check_live_e2e.rs`) caught the omission
 * in r1, and `contract.test.ts` pins it here.
 */
export function updateCheckStartParams(): {
  workflowType: string;
  input: Record<string, never>;
} {
  return { workflowType: UPDATE_CHECK_WORKFLOW_TYPE, input: {} };
}
