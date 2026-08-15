//! The streamed-message accumulator: turns ACP's per-chunk `session/update` streams into a
//! transcript that is both live and durable, and into the run's final text.
//!
//! # The problem this module exists to solve
//!
//! ACP streams content as `*_chunk` updates and **never sends a finalised message**. Two
//! consequences follow, and both have to be handled here or information is lost:
//!
//! 1. **Persisting every chunk is the delta-flood defect.** One frame per token, each written
//!    durably, rewrites the finished message thousands of times over. So a chunk's live event is
//!    *ephemeral* — forwarded to the live channel, never persisted.
//! 2. **But then nothing durable would carry the text at all.** So the accumulator buffers each
//!    message and, at its boundary, emits ONE durable [`ActivityEventKind::Message`] carrying the
//!    whole text. The boundary is a genuinely new `messageId` (see [`MessageAccumulator::finish`]
//!    for the other boundary: the end of the turn).
//!
//! # Three independent streams, accumulated the same way
//!
//! ACP has three chunk streams, and every one of them is a `ContentChunk` — the protocol makes no
//! promise about how many chunks any of them arrives in, so all three are accumulated:
//!
//! | stream | live event | durable event | in the run's result? |
//! |---|---|---|---|
//! | `agent_message_chunk` | ephemeral [`ActivityEventKind::Delta`] | `Message { role: Assistant }` | **yes: accumulated and last-message forms** |
//! | `agent_thought_chunk` | ephemeral `Progress::Note` | `Message { role: Assistant }` | no |
//! | `user_message_chunk` | none (see below) | `Message { role: User }` | no |
//!
//! - **Reasoning is durable.** A run whose reasoning existed only on an ephemeral channel would
//!   leave nothing behind to audit after the fact, which defeats the point of a durable transcript.
//!   [`ActivityEventKind::Message`] is documented as the home for "assistant text/thinking", so
//!   that is where accumulated reasoning lands. **Known limitation:** the neutral
//!   [`MessageRole`] vocabulary has no reasoning role, so a durable reasoning message and a durable
//!   answer message are both `Message { role: Assistant }` and a consumer cannot tell them apart.
//!   Inventing a role or a marker would be fabrication; the distinction that IS preserved is the
//!   one that matters for correctness — only the `agent_message_chunk` stream contributes to
//!   [`MessageAccumulator::completed_text`] or [`MessageAccumulator::final_message`], so reasoning
//!   can never leak into either result view.
//! - **A user chunk has no live event.** The neutral [`ActivityEventKind::Delta`] carries no role,
//!   so streaming the operator's own words through it would present them downstream as the agent's
//!   answer. Nothing is lost: every user chunk's text is in the durable message its boundary emits,
//!   and the end of the turn is always a boundary.
//!
//! The same buffer produces the run's terminal output: ACP's `session/prompt` Response carries only
//! a `stopReason`, so the agent's actual answer is the accumulated `agent_message_chunk` text and
//! nothing else. See [`MessageAccumulator::completed_text`].
//!
//! A chunk whose content block is not text (an image, audio, an embedded resource, a resource link)
//! is not appendable and is never silently dropped: it becomes a durable
//! [`ActivityEventKind::Raw`] carrying the block verbatim.
//!
//! [`ActivityEventKind::Delta`]: aion_core::ActivityEventKind::Delta
//! [`ActivityEventKind::Message`]: aion_core::ActivityEventKind::Message
//! [`ActivityEventKind::Raw`]: aion_core::ActivityEventKind::Raw
//! [`MessageAccumulator::completed_text`]: crate::messages::MessageAccumulator::completed_text
//! [`MessageAccumulator::finish`]: crate::messages::MessageAccumulator::finish
//! [`MessageRole`]: aion_core::MessageRole

use agent_client_protocol_schema::v1::{ContentBlock, ContentChunk, MessageId};
use aion_core::{ActivityEventKind, MessageRole, ProgressDetail};

use crate::translate::{EventKind, update_raw};

/// The separator inserted between two completed agent messages in [`MessageAccumulator::completed_text`].
///
/// Two messages are distinct turns of output; concatenating them bare would run the last word of
/// one into the first word of the next. A newline is the least surprising join for text an operator
/// or a downstream activity will read.
const MESSAGE_JOIN: &str = "\n";

/// One of ACP's three independent content-chunk streams.
///
/// Named as an enum rather than three near-copies of the same accumulation logic, so a change to
/// the boundary rule cannot apply to one stream and not the others.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ChunkStream {
    /// `agent_message_chunk` — the agent's answer. The ONLY stream whose text becomes the result.
    AgentMessage,
    /// `agent_thought_chunk` — the agent's reasoning.
    AgentThought,
    /// `user_message_chunk` — an operator turn (the prompt, or the agent's echo of it).
    User,
}

impl ChunkStream {
    /// The `session/update` variant name this stream's chunks arrive under.
    ///
    /// Used as the `Raw` source suffix for a chunk whose content block is not text, so the recorded
    /// frame names the stream it came from.
    const fn variant(self) -> &'static str {
        match self {
            Self::AgentMessage => "agent_message_chunk",
            Self::AgentThought => "agent_thought_chunk",
            Self::User => "user_message_chunk",
        }
    }

    /// The neutral role a completed message from this stream is attributed to.
    ///
    /// Reasoning shares [`MessageRole::Assistant`] with the answer: the agent produced both, and
    /// the neutral vocabulary has no reasoning role to distinguish them (see the module docs).
    const fn role(self) -> MessageRole {
        match self {
            Self::AgentMessage | Self::AgentThought => MessageRole::Assistant,
            Self::User => MessageRole::User,
        }
    }

    /// Whether this stream's completed text is part of the run's result.
    ///
    /// Only the agent's answer is. Folding reasoning or an echoed user turn into the result would
    /// hand the workflow text the agent never offered as its answer.
    const fn feeds_the_result(self) -> bool {
        matches!(self, Self::AgentMessage)
    }
}

/// One message being streamed: the `messageId` it is tagged with (ACP allows none) and the text
/// accumulated so far.
#[derive(Debug)]
struct OpenMessage {
    /// The chunk `messageId` this message is accumulating under. `None` is a real state, not a
    /// missing value: an agent may stream a whole message without tagging it.
    id: Option<MessageId>,
    /// The text accumulated from every chunk seen for this message, in arrival order.
    text: String,
}

/// Accumulates ACP's streamed content chunks into durable messages and the run's final text.
#[derive(Debug, Default)]
pub struct MessageAccumulator {
    /// The agent answer currently being streamed, if any.
    agent_message: Option<OpenMessage>,
    /// The agent reasoning block currently being streamed, if any.
    agent_thought: Option<OpenMessage>,
    /// The user turn currently being streamed, if any.
    user: Option<OpenMessage>,
    /// Every completed AGENT ANSWER's text, joined by [`MESSAGE_JOIN`] — the run's output so far.
    completed: String,
    /// The LAST completed AGENT ANSWER alone, exactly as accumulated from its text chunks.
    final_message: String,
}

impl MessageAccumulator {
    /// An accumulator with nothing streamed yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The concatenated text of every agent ANSWER completed so far.
    ///
    /// This is the run's output: ACP's terminal Response carries only a stop reason, so the agent's
    /// answer exists nowhere else. Reasoning and user turns are deliberately absent — they are
    /// accumulated and recorded, but they are not the answer. Call [`Self::finish`] before reading
    /// it at the end of a turn, so the message still open when the Response arrived is included.
    #[must_use]
    pub fn completed_text(&self) -> &str {
        &self.completed
    }

    /// The LAST completed agent ANSWER alone, exactly as the agent streamed it.
    ///
    /// Reasoning and user streams never touch this value. It is empty when no answer text was
    /// produced. Call [`Self::finish`] before reading it at the end of a turn so an answer still open
    /// when the terminal Response arrived is folded in.
    #[must_use]
    pub fn final_message(&self) -> &str {
        &self.final_message
    }

    /// Absorbs one `agent_message_chunk`, returning the events it produces in emission order.
    ///
    /// Emits the durable [`ActivityEventKind::Message`] for a message that just ENDED (when this
    /// chunk starts a new one) before the ephemeral [`ActivityEventKind::Delta`] for this chunk, so
    /// the transcript reads in the order things happened.
    pub fn push_agent_chunk(&mut self, chunk: &ContentChunk) -> Vec<EventKind> {
        let (mut events, text) = self.absorb(ChunkStream::AgentMessage, chunk);
        if let Some(text) = text {
            events.push((
                ActivityEventKind::Delta {
                    message_id: message_id_label(chunk.message_id.as_ref()),
                    text_fragment: text.to_owned(),
                },
                true,
            ));
        }
        events
    }

    /// Absorbs one `agent_thought_chunk`, returning the events it produces in emission order.
    ///
    /// The live event is an ephemeral free-form `Progress::Note` rather than a
    /// [`ActivityEventKind::Delta`], because a `Delta` is the answer's token stream and a console
    /// appending reasoning to it would corrupt the answer on screen. The durable record is the
    /// accumulated message the boundary emits.
    pub fn push_thought_chunk(&mut self, chunk: &ContentChunk) -> Vec<EventKind> {
        let (mut events, text) = self.absorb(ChunkStream::AgentThought, chunk);
        if let Some(text) = text {
            events.push((
                ActivityEventKind::Progress {
                    detail: ProgressDetail::Note {
                        text: text.to_owned(),
                    },
                },
                true,
            ));
        }
        events
    }

    /// Absorbs one `user_message_chunk`, returning the events it produces in emission order.
    ///
    /// There is no live event for a user chunk (see the module docs): the accumulated durable
    /// message its boundary emits is the whole record. A single-chunk user turn therefore produces
    /// no event until the turn's [`Self::finish`], which is guaranteed to happen.
    pub fn push_user_chunk(&mut self, chunk: &ContentChunk) -> Vec<EventKind> {
        let (events, _text) = self.absorb(ChunkStream::User, chunk);
        events
    }

    /// Closes every open message, emitting each one's durable [`ActivityEventKind::Message`].
    ///
    /// Called at the end of the turn, which is a boundary for all three streams. The order is
    /// user, reasoning, then answer, so the agent's answer is the last durable message before the
    /// terminal `Stop` event — where a reader looking for the run's output expects it.
    ///
    /// Returns no event for a stream with nothing open or nothing accumulated — an empty message is
    /// not a transcript entry. Idempotent, so the caller may finish a turn without tracking what
    /// was open.
    pub fn finish(&mut self) -> Vec<EventKind> {
        let mut events = Vec::new();
        for stream in [
            ChunkStream::User,
            ChunkStream::AgentThought,
            ChunkStream::AgentMessage,
        ] {
            events.extend(self.close(stream));
        }
        events
    }

    /// Appends one chunk's text to `stream`'s buffer.
    ///
    /// Returns the boundary events (the durable message of the PREVIOUS message on this stream, if
    /// this chunk started a new one) and the chunk's text — or, for a chunk whose content block is
    /// not text, the verbatim `Raw` event recording it and no text.
    fn absorb<'chunk>(
        &mut self,
        stream: ChunkStream,
        chunk: &'chunk ContentChunk,
    ) -> (Vec<EventKind>, Option<&'chunk str>) {
        let Some(text) = block_text(&chunk.content) else {
            // Not text: nothing to append, and nothing may be dropped either.
            let source = format!("{}/content", stream.variant());
            return (vec![update_raw(&source, &chunk.content)], None);
        };
        let mut events = Vec::new();
        if self.starts_a_new_message(stream, chunk.message_id.as_ref()) {
            events.extend(self.close(stream));
        }
        let open = self.slot_mut(stream).get_or_insert_with(|| OpenMessage {
            id: chunk.message_id.clone(),
            text: String::new(),
        });
        open.text.push_str(text);
        (events, Some(text))
    }

    /// Whether `incoming` is the protocol's "a new message has started" signal for `stream`.
    ///
    /// It requires **two present ids that differ**. An ABSENT id is never a boundary, in either
    /// direction:
    ///
    /// - ACP allows an agent to stream a whole message untagged, so absence carries no
    ///   message-identity information to compare against.
    /// - `ContentChunk::message_id` is deserialised `DefaultOnError`, so ONE malformed id on the
    ///   wire arrives as `None`. Treating that as a boundary would split one message in two and
    ///   insert a [`MESSAGE_JOIN`] newline into [`Self::completed_text`] — and that text is the
    ///   activity's replay-authoritative result. A fabricated newline in the result is a
    ///   correctness bug, so the boundary rule refuses to infer one from a missing value.
    fn starts_a_new_message(&self, stream: ChunkStream, incoming: Option<&MessageId>) -> bool {
        let open = self.slot(stream).and_then(|open| open.id.as_ref());
        match (open, incoming) {
            (Some(open), Some(incoming)) => open != incoming,
            _ => false,
        }
    }

    /// Closes `stream`'s open message, emitting its durable [`ActivityEventKind::Message`] and — for
    /// the answer stream only — folding its text into [`Self::completed_text`] and replacing
    /// [`Self::final_message`].
    fn close(&mut self, stream: ChunkStream) -> Vec<EventKind> {
        let Some(open) = self.slot_mut(stream).take() else {
            return Vec::new();
        };
        if open.text.is_empty() {
            return Vec::new();
        }
        if stream.feeds_the_result() {
            if !self.completed.is_empty() {
                self.completed.push_str(MESSAGE_JOIN);
            }
            self.completed.push_str(&open.text);
            self.final_message.clone_from(&open.text);
        }
        vec![(
            ActivityEventKind::Message {
                role: stream.role(),
                text: open.text,
            },
            false,
        )]
    }

    /// The message `stream` is currently accumulating, if any.
    const fn slot(&self, stream: ChunkStream) -> Option<&OpenMessage> {
        match stream {
            ChunkStream::AgentMessage => self.agent_message.as_ref(),
            ChunkStream::AgentThought => self.agent_thought.as_ref(),
            ChunkStream::User => self.user.as_ref(),
        }
    }

    /// The buffer slot `stream` accumulates into, mutably.
    const fn slot_mut(&mut self, stream: ChunkStream) -> &mut Option<OpenMessage> {
        match stream {
            ChunkStream::AgentMessage => &mut self.agent_message,
            ChunkStream::AgentThought => &mut self.agent_thought,
            ChunkStream::User => &mut self.user,
        }
    }
}

/// The text a content block carries, or `None` for a block that is not text.
///
/// [`ContentBlock`] is `#[non_exhaustive]`: a variant added by a future schema release is treated
/// as not-text and reaches the transcript verbatim rather than being mistaken for text.
fn block_text(block: &ContentBlock) -> Option<&str> {
    match block {
        ContentBlock::Text(text) => Some(&text.text),
        _ => None,
    }
}

/// Renders a chunk's optional `messageId` for the neutral [`ActivityEventKind::Delta`] field.
///
/// The neutral field is a plain `String`, so an untagged chunk renders as the empty label. That is
/// the honest representation of "the agent did not tag this message" — it is never substituted with
/// an invented id, because a fabricated id would falsely split or merge messages downstream.
fn message_id_label(id: Option<&MessageId>) -> String {
    id.map(ToString::to_string).unwrap_or_default()
}

#[cfg(test)]
mod tests;
