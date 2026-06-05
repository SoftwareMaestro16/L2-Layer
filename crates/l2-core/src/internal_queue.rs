use crate::crypto::{hash_domain, Hash32};
use crate::tvm::TvmInternalMessage;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

pub const DEFAULT_MAX_INTERNAL_QUEUE_LEN: usize = 4_096;
pub const DEFAULT_MAX_INTERNAL_MESSAGES_PER_BLOCK: usize = 128;
pub const DEFAULT_INTERNAL_MESSAGE_GAS_LIMIT: u64 = 100_000;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct QueuedInternalMessage {
    pub message_id: Hash32,
    pub message: TvmInternalMessage,
    pub enqueue_height: u64,
    pub sequence: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InternalMessageQueue {
    pending: VecDeque<QueuedInternalMessage>,
    max_len: usize,
    next_sequence: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct InternalMessageQueueSnapshot {
    pub pending: Vec<QueuedInternalMessage>,
    pub next_sequence: u64,
}

impl InternalMessageQueue {
    pub fn new(max_len: usize) -> Self {
        Self {
            pending: VecDeque::new(),
            max_len,
            next_sequence: 0,
        }
    }

    pub fn from_snapshot(
        max_len: usize,
        snapshot: InternalMessageQueueSnapshot,
    ) -> Result<Self, InternalMessageQueueError> {
        if snapshot.pending.len() > max_len {
            return Err(InternalMessageQueueError::Full);
        }
        Ok(Self {
            pending: VecDeque::from(snapshot.pending),
            max_len,
            next_sequence: snapshot.next_sequence,
        })
    }

    pub fn snapshot(&self) -> InternalMessageQueueSnapshot {
        InternalMessageQueueSnapshot {
            pending: self.pending.iter().cloned().collect(),
            next_sequence: self.next_sequence,
        }
    }

    pub fn push_many(
        &mut self,
        enqueue_height: u64,
        messages: Vec<TvmInternalMessage>,
    ) -> Result<(), InternalMessageQueueError> {
        if self.pending.len().saturating_add(messages.len()) > self.max_len {
            return Err(InternalMessageQueueError::Full);
        }
        let mut queued = Vec::with_capacity(messages.len());
        for message in messages {
            let sequence = self.next_sequence;
            self.next_sequence = self
                .next_sequence
                .checked_add(1)
                .ok_or(InternalMessageQueueError::SequenceOverflow)?;
            queued.push(QueuedInternalMessage {
                message_id: internal_message_id(enqueue_height, sequence, &message),
                message,
                enqueue_height,
                sequence,
            });
        }
        self.pending.extend(queued);
        Ok(())
    }

    pub fn pop_front(&mut self) -> Option<QueuedInternalMessage> {
        self.pending.pop_front()
    }

    pub fn len(&self) -> usize {
        self.pending.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    pub fn max_len(&self) -> usize {
        self.max_len
    }
}

impl Default for InternalMessageQueue {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_INTERNAL_QUEUE_LEN)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum InternalMessageQueueError {
    #[error("internal message queue is full")]
    Full,
    #[error("internal message sequence overflow")]
    SequenceOverflow,
}

impl InternalMessageQueueError {
    pub fn rejection_reason(self) -> &'static str {
        match self {
            Self::Full => "internal_queue_full",
            Self::SequenceOverflow => "internal_queue_sequence_overflow",
        }
    }
}

pub fn internal_message_id(
    enqueue_height: u64,
    sequence: u64,
    message: &TvmInternalMessage,
) -> Hash32 {
    let value = message.value.to_be_bytes();
    let enqueue_height = enqueue_height.to_be_bytes();
    let sequence = sequence.to_be_bytes();
    let flags = [u8::from(message.bounce), u8::from(message.bounced)];
    hash_domain(
        "l2.internal.message.id.v1",
        &[
            message.from.as_bytes(),
            message.to.as_bytes(),
            &value,
            &message.body_boc,
            &flags,
            &enqueue_height,
            &sequence,
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::sha256_bytes;

    fn message(label: &[u8]) -> TvmInternalMessage {
        TvmInternalMessage {
            from: sha256_bytes(b"from"),
            to: sha256_bytes(b"to"),
            value: 0,
            body_boc: label.to_vec(),
            bounce: true,
            bounced: false,
        }
    }

    #[test]
    fn queue_ids_are_deterministic_and_fifo() {
        let mut queue = InternalMessageQueue::new(4);
        queue
            .push_many(7, vec![message(b"a"), message(b"b")])
            .expect("queue messages");

        let first = queue.pop_front().expect("first");
        let second = queue.pop_front().expect("second");

        assert_eq!(first.sequence, 0);
        assert_eq!(second.sequence, 1);
        assert_ne!(first.message_id, second.message_id);
        assert_eq!(
            first.message_id,
            internal_message_id(first.enqueue_height, first.sequence, &first.message)
        );
        assert!(queue.is_empty());
        let restored =
            InternalMessageQueue::from_snapshot(4, queue.snapshot()).expect("restore snapshot");
        assert!(restored.is_empty());
    }

    #[test]
    fn queue_capacity_is_bounded() {
        let mut queue = InternalMessageQueue::new(1);
        let error = queue
            .push_many(1, vec![message(b"a"), message(b"b")])
            .expect_err("too many messages");

        assert_eq!(error, InternalMessageQueueError::Full);
        assert!(queue.is_empty());
    }
}
