use agent_client_protocol_schema::v1::{ContentBlock, ContentChunk, ImageContent, TextContent};
use aion_core::{ActivityEventKind, MessageRole, ProgressDetail};

use super::MessageAccumulator;
use crate::translate::EventKind;

/// A text chunk, optionally tagged with a `messageId`.
fn chunk(text: &str, message_id: Option<&'static str>) -> ContentChunk {
    let chunk = ContentChunk::new(ContentBlock::Text(TextContent::new(text)));
    match message_id {
        Some(id) => chunk.message_id(id),
        None => chunk,
    }
}

/// The `(message_id, text_fragment)` of an [`ActivityEventKind::Delta`], or `None` otherwise.
fn delta(event: &EventKind) -> Option<(&str, &str)> {
    match &event.0 {
        ActivityEventKind::Delta {
            message_id,
            text_fragment,
        } => Some((message_id.as_str(), text_fragment.as_str())),
        _ => None,
    }
}

/// The `(role, text)` of a durable [`ActivityEventKind::Message`], or `None` otherwise.
fn message(event: &EventKind) -> Option<(MessageRole, &str)> {
    match &event.0 {
        ActivityEventKind::Message { role, text } => Some((*role, text.as_str())),
        _ => None,
    }
}

/// The text of an ephemeral `Progress::Note`, or `None` otherwise.
fn note(event: &EventKind) -> Option<&str> {
    match &event.0 {
        ActivityEventKind::Progress {
            detail: ProgressDetail::Note { text },
        } => Some(text.as_str()),
        _ => None,
    }
}

#[test]
fn each_agent_chunk_yields_one_ephemeral_delta() -> Result<(), String> {
    let mut accumulator = MessageAccumulator::new();
    let events = accumulator.push_agent_chunk(&chunk("hel", Some("m1")));
    let [only] = events.as_slice() else {
        return Err(format!("one event per chunk, got {events:?}"));
    };
    assert!(
        only.1,
        "a per-token chunk must be ephemeral, never persisted"
    );
    assert_eq!(delta(only), Some(("m1", "hel")));
    Ok(())
}

#[test]
fn a_changed_message_id_closes_the_previous_message_durably() -> Result<(), String> {
    let mut accumulator = MessageAccumulator::new();
    accumulator.push_agent_chunk(&chunk("hello ", Some("m1")));
    accumulator.push_agent_chunk(&chunk("world", Some("m1")));
    let events = accumulator.push_agent_chunk(&chunk("next", Some("m2")));

    let [closed, opened] = events.as_slice() else {
        return Err(format!("expected a close then a delta, got {events:?}"));
    };
    assert!(!closed.1, "a completed message is durable");
    assert_eq!(
        message(closed),
        Some((MessageRole::Assistant, "hello world"))
    );
    assert_eq!(delta(opened), Some(("m2", "next")));
    assert_eq!(accumulator.completed_text(), "hello world");
    Ok(())
}

#[test]
fn finish_closes_the_open_message_and_is_idempotent() -> Result<(), String> {
    let mut accumulator = MessageAccumulator::new();
    accumulator.push_agent_chunk(&chunk("done", Some("m1")));
    let events = accumulator.finish();
    let [closed] = events.as_slice() else {
        return Err(format!("finish closes the open message, got {events:?}"));
    };
    assert_eq!(message(closed), Some((MessageRole::Assistant, "done")));
    assert!(
        accumulator.finish().is_empty(),
        "a second finish must emit nothing"
    );
    assert_eq!(accumulator.completed_text(), "done");
    assert_eq!(
        accumulator.final_message(),
        accumulator.completed_text(),
        "one answer is honestly both the whole output and the final message"
    );
    Ok(())
}

#[test]
fn untagged_chunks_accumulate_as_one_message() -> Result<(), String> {
    // An agent that never sends `messageId` streams one implicit message per turn.
    let mut accumulator = MessageAccumulator::new();
    accumulator.push_agent_chunk(&chunk("a", None));
    let second = accumulator.push_agent_chunk(&chunk("b", None));
    let [only] = second.as_slice() else {
        return Err(format!("no boundary between untagged chunks: {second:?}"));
    };
    assert_eq!(delta(only), Some(("", "b")));
    accumulator.finish();
    assert_eq!(accumulator.completed_text(), "ab");
    Ok(())
}

#[test]
fn a_dropped_message_id_does_not_fabricate_a_boundary() {
    // `ContentChunk::message_id` is deserialised `DefaultOnError`, so ONE malformed id on the
    // wire arrives as `None`. The accumulated text is the activity's result: a boundary here
    // would insert a newline the agent never sent.
    let mut accumulator = MessageAccumulator::new();
    accumulator.push_agent_chunk(&chunk("a", Some("m1")));
    let untagged = accumulator.push_agent_chunk(&chunk("b", None));
    assert!(
        untagged.iter().all(|event| message(event).is_none()),
        "an absent id must not close the open message: {untagged:?}"
    );
    let retagged = accumulator.push_agent_chunk(&chunk("c", Some("m1")));
    assert!(
        retagged.iter().all(|event| message(event).is_none()),
        "returning to the SAME id is not a new message: {retagged:?}"
    );
    accumulator.finish();
    assert_eq!(
        accumulator.completed_text(),
        "abc",
        "one message's text, with no fabricated newline"
    );
}

#[test]
fn a_first_untagged_chunk_does_not_split_a_later_tagged_one() {
    // The mirror case: the malformed id lands on the FIRST chunk. `None -> Some(m1)` carries no
    // more evidence of a new message than `Some(m1) -> None` does.
    let mut accumulator = MessageAccumulator::new();
    accumulator.push_agent_chunk(&chunk("a", None));
    let tagged = accumulator.push_agent_chunk(&chunk("b", Some("m1")));
    assert!(
        tagged.iter().all(|event| message(event).is_none()),
        "an id appearing mid-message is not a boundary: {tagged:?}"
    );
    accumulator.finish();
    assert_eq!(accumulator.completed_text(), "ab");
}

#[test]
fn two_completed_messages_are_joined_by_a_newline() {
    let mut accumulator = MessageAccumulator::new();
    accumulator.push_agent_chunk(&chunk("first", Some("m1")));
    accumulator.push_agent_chunk(&chunk("second", Some("m2")));
    accumulator.finish();
    assert_eq!(accumulator.completed_text(), "first\nsecond");
}

#[test]
fn three_answer_messages_keep_the_last_message_distinct() {
    let mut accumulator = MessageAccumulator::new();
    accumulator.push_agent_chunk(&chunk("narration", Some("m1")));
    accumulator.push_agent_chunk(&chunk("more narration", Some("m2")));
    accumulator.push_agent_chunk(&chunk("fleet-judge-pass", Some("m3")));
    accumulator.finish();

    assert_eq!(
        accumulator.completed_text(),
        "narration\nmore narration\nfleet-judge-pass"
    );
    assert_eq!(accumulator.final_message(), "fleet-judge-pass");
}

#[test]
fn a_tool_only_turn_has_no_text_or_final_message() {
    let mut accumulator = MessageAccumulator::new();
    accumulator.finish();

    assert_eq!(accumulator.completed_text(), "");
    assert_eq!(accumulator.final_message(), "");
}

#[test]
fn a_non_text_chunk_survives_verbatim_and_is_not_appended() -> Result<(), String> {
    let mut accumulator = MessageAccumulator::new();
    let image = ContentChunk::new(ContentBlock::Image(ImageContent::new(
        "AAAA".to_owned(),
        "image/png".to_owned(),
    )));
    let events = accumulator.push_agent_chunk(&image);
    let [only] = events.as_slice() else {
        return Err(format!("one raw event for a non-text chunk: {events:?}"));
    };
    match &only.0 {
        ActivityEventKind::Raw { source, value } => {
            assert_eq!(source, "session/update/agent_message_chunk/content");
            assert_eq!(value["type"], serde_json::json!("image"));
        }
        other => return Err(format!("expected Raw, got {other:?}")),
    }
    assert!(!only.1, "a non-text block is durable, not a token delta");
    assert_eq!(
        accumulator.completed_text(),
        "",
        "a non-text block must not enter the run's text output"
    );
    Ok(())
}

#[test]
fn reasoning_streams_live_and_survives_durably() -> Result<(), String> {
    // The ruling: reasoning that vanishes at end-of-run defeats durable observability. It
    // streams as an ephemeral Note AND accumulates into a durable message.
    let mut accumulator = MessageAccumulator::new();
    let first = accumulator.push_thought_chunk(&chunk("weigh", Some("t1")));
    let [live] = first.as_slice() else {
        return Err(format!("one live note per thought chunk, got {first:?}"));
    };
    assert!(live.1, "the live reasoning stream is ephemeral");
    assert_eq!(note(live), Some("weigh"));

    accumulator.push_thought_chunk(&chunk("ing it", Some("t1")));
    let closed = accumulator.finish();
    let [durable] = closed.as_slice() else {
        return Err(format!("one durable reasoning message, got {closed:?}"));
    };
    assert!(!durable.1, "the accumulated reasoning is durable");
    assert_eq!(
        message(durable),
        Some((MessageRole::Assistant, "weighing it")),
        "every chunk's text, in one message"
    );
    Ok(())
}

#[test]
fn reasoning_never_enters_the_runs_result() {
    // The one distinction that must survive: reasoning is not the answer.
    let mut accumulator = MessageAccumulator::new();
    accumulator.push_thought_chunk(&chunk("I should refuse", Some("t1")));
    accumulator.push_agent_chunk(&chunk("42", Some("m1")));
    accumulator.finish();
    assert_eq!(
        accumulator.completed_text(),
        "42",
        "only the agent's answer is the activity's result"
    );
    assert_eq!(
        accumulator.final_message(),
        "42",
        "thought chunks never contaminate the final answer"
    );
}

#[test]
fn a_streamed_user_turn_becomes_one_durable_message() -> Result<(), String> {
    // The type is a ContentChunk: an echoing agent may stream a user turn in as many chunks as
    // it likes, and each one must not become its own durable event.
    let mut accumulator = MessageAccumulator::new();
    for fragment in ["do ", "the ", "thing"] {
        let events = accumulator.push_user_chunk(&chunk(fragment, Some("u1")));
        assert!(
            events.is_empty(),
            "a user chunk emits nothing until its boundary: {events:?}"
        );
    }
    let closed = accumulator.finish();
    let [durable] = closed.as_slice() else {
        return Err(format!("one durable user message, got {closed:?}"));
    };
    assert!(!durable.1, "a user turn is durable");
    assert_eq!(message(durable), Some((MessageRole::User, "do the thing")));
    assert_eq!(
        accumulator.completed_text(),
        "",
        "a user turn is not the agent's answer"
    );
    Ok(())
}

#[test]
fn a_new_user_message_id_closes_the_previous_user_turn() -> Result<(), String> {
    let mut accumulator = MessageAccumulator::new();
    accumulator.push_user_chunk(&chunk("first", Some("u1")));
    let events = accumulator.push_user_chunk(&chunk("second", Some("u2")));
    let [closed] = events.as_slice() else {
        return Err(format!("the previous user turn must close, got {events:?}"));
    };
    assert_eq!(message(closed), Some((MessageRole::User, "first")));
    Ok(())
}

#[test]
fn the_three_streams_do_not_interleave_into_each_other() {
    // Each stream carries its own open message: a thought chunk arriving mid-answer must not
    // close the answer, and must not be appended to it.
    let mut accumulator = MessageAccumulator::new();
    accumulator.push_agent_chunk(&chunk("the ", Some("m1")));
    accumulator.push_thought_chunk(&chunk("hmm", Some("t1")));
    accumulator.push_user_chunk(&chunk("asked", Some("u1")));
    accumulator.push_agent_chunk(&chunk("answer", Some("m1")));

    let closed = accumulator.finish();
    let texts: Vec<(MessageRole, &str)> = closed.iter().filter_map(message).collect();
    assert_eq!(
        texts,
        vec![
            (MessageRole::User, "asked"),
            (MessageRole::Assistant, "hmm"),
            (MessageRole::Assistant, "the answer"),
        ],
        "user, then reasoning, then the answer last — got {closed:?}"
    );
    assert_eq!(accumulator.completed_text(), "the answer");
}

#[test]
fn a_non_text_thought_or_user_block_is_recorded_under_its_own_stream() -> Result<(), String> {
    let image = ContentChunk::new(ContentBlock::Image(ImageContent::new(
        "AAAA".to_owned(),
        "image/png".to_owned(),
    )));
    let mut accumulator = MessageAccumulator::new();
    let cases = [
        (
            accumulator.push_thought_chunk(&image),
            "session/update/agent_thought_chunk/content",
        ),
        (
            accumulator.push_user_chunk(&image),
            "session/update/user_message_chunk/content",
        ),
    ];
    for (events, expected) in cases {
        let [only] = events.as_slice() else {
            return Err(format!("one raw event, got {events:?}"));
        };
        match &only.0 {
            ActivityEventKind::Raw { source, .. } => assert_eq!(source, expected),
            other => return Err(format!("expected Raw, got {other:?}")),
        }
    }
    Ok(())
}
