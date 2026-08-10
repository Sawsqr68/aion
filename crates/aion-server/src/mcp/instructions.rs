//! The `server/discover` instructions.
//!
//! This is the only prose a model reliably reads before it starts calling, so
//! it is written to prevent the specific mistakes an agent makes against a
//! durable workflow engine: inventing identifiers, reading one generation's
//! transcript as another's, waiting on a read that will never block, and
//! cancelling a run when it meant to stop waiting for one.

use aion_mcp::TASKS_EXTENSION_PIN;

/// Build the instructions text.
///
/// The tasks-extension pin is interpolated rather than restated so the prose
/// and the advertised capability can never name two different drafts.
#[must_use]
pub(crate) fn instructions() -> String {
    format!(
        "You are driving Aion, a durable workflow engine. A workflow here is not a script that \
runs once: it is an event-sourced execution that can crash on step nine of ten and resume \
exactly where it was, or sleep for three months and wake in the same state. Everything you \
can see is projected from an append-only history, and that history is the truth.\n\
\n\
START WITH describe_run. Before you act on a run — before you signal it, cancel it, or draw \
any conclusion about it — call describe_run. One call tells you the run's projected status, \
which step it is on right now, whether anything in the fleet can actually serve that step, \
and the exact handles for reading each step's agent transcript. Acting on a run you have not \
described is guessing.\n\
\n\
NEVER INVENT AN IDENTIFIER. Namespaces, workflow ids, run ids, activity ordinals and attempt \
numbers all come from a previous result. Use list_runs to find a run you do not already hold. \
A workflow id you constructed will not be found, and — because a namespace you do not hold \
answers exactly like a workflow that does not exist — you will not be able to tell which \
mistake you made. That ambiguity is deliberate: it is what stops a caller mapping another \
tenant's namespace.\n\
\n\
RUN IDS ARE NOT DECORATION. A workflow that continues-as-new has several runs, and the \
generations do NOT share step numbering: activity ordinal 0 attempt 1 exists in every one of \
them. read_transcript therefore REQUIRES a run_id, and it will refuse a run that never \
dispatched the step you named. The run is an axis of the durable stream key itself, so the \
transcript you read is that run's alone — a sibling generation's events are excluded by the \
key range, not by a filter you have to trust.\n\
\n\
READS RETURN IMMEDIATELY. read_transcript and read_history are cursor pages over what exists \
right now; neither ever waits for more. To follow a step that is still running, call again \
with from_seq set to the next_from_seq you were handed. Do not sit in a tight loop: honour \
the poll interval you are given, and prefer describe_run to see whether anything has \
changed at all.\n\
\n\
STATUS DOES NOT MEAN PROGRESS. A run whose activity is being worked on and a run whose \
activity is parked with no worker to take it both project Running — history genuinely holds \
no terminal event for either. describe_run's `unserved` list is what separates them. An \
empty `unserved` is the healthy answer; a non-empty one names what an operator has to fix.\n\
\n\
START AND WAIT. start_run returns handles immediately. Set await_completion to be handed a \
task instead: you get a taskId at once, the run proceeds, and tasks/get carries the outcome \
when it lands. That needs the tasks extension declared on your request \
(io.modelcontextprotocol/tasks, implemented against draft revision {TASKS_EXTENSION_PIN}); \
without it the call is refused rather than quietly turned into a plain start. Cancelling that \
task stops your WAIT, not the run. Stopping the run is the cancel tool, and cancel is \
destructive: it records a terminal cancellation the run cannot be talked out of.\n\
\n\
AUTHORING IS A LOOP, NOT A GUESS. The server holds one AWL workspace. list_documents and \
read_document show what exists; check_document runs the checker over source you are drafting \
(pass its workspace path so schema imports resolve); save_document writes it and hands back a \
content_hash; deploy_document takes that path and hash and deploys exactly the SAVED document \
— there is no way to deploy unsaved source. Check before saving, save before deploying, and \
always deploy the hash you were just handed: a hash that no longer matches the saved document \
means it changed under you and is refused. save_document and deploy_document need the deploy \
grant; without it they refuse and name what is missing.\n\
\n\
FAILURES ARE INFORMATION. A tool that refuses tells you what went wrong and what to do about \
it, in the result. Read it and correct the call; do not retry the same call with the same \
arguments hoping for a different answer, except where the failure explicitly says the fault \
is a retryable race."
    )
}

#[cfg(test)]
mod tests {
    use super::instructions;

    #[test]
    fn the_instructions_name_the_first_call_and_the_hard_rules() {
        let text = instructions();
        for phrase in [
            "describe_run",
            "NEVER INVENT AN IDENTIFIER",
            "REQUIRES a run_id",
            "unserved",
            "await_completion",
            "AUTHORING IS A LOOP",
            "deploys exactly the SAVED document",
            "deploy grant",
        ] {
            assert!(text.contains(phrase), "instructions must mention {phrase}");
        }
    }

    #[test]
    fn the_pinned_tasks_revision_is_interpolated_not_restated() {
        assert!(instructions().contains(aion_mcp::TASKS_EXTENSION_PIN));
    }

    /// A few hundred words, per the brief. Both bounds matter: too short and it
    /// teaches nothing, too long and a model skims past the rules that prevent
    /// the damage.
    #[test]
    fn the_instructions_are_a_few_hundred_words() {
        let words = instructions().split_whitespace().count();
        assert!(
            (200..=800).contains(&words),
            "instructions are {words} words"
        );
    }
}
