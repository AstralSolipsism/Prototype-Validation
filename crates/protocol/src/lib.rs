#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;
use world_ids::{CommandId, EventId, PersonId};
use world_time::WorldInstant;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SchemaVersion(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EntityVersion(pub u64);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandEnvelope<P> {
    pub command_id: CommandId,
    pub actor_id: PersonId,
    pub issued_at: WorldInstant,
    pub expected_entity_version: Option<EntityVersion>,
    pub schema_version: SchemaVersion,
    pub payload: P,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomainEventEnvelope<E> {
    pub event_id: EventId,
    pub caused_by: CommandId,
    pub occurred_at: WorldInstant,
    pub schema_version: SchemaVersion,
    pub payload: E,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandOutcome<T> {
    Accepted {
        resulting_version: EntityVersion,
        value: T,
    },
    Rejected(CommandRejection),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandRejection {
    pub code: RejectionCode,
    pub message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RejectionCode {
    VersionConflict,
    PermissionDenied,
    InvalidState,
    InvalidPayload,
    ResourceUnavailable,
    UnsupportedSchema,
}

#[derive(Clone, Debug, Default)]
pub struct IdempotencyLedger<R> {
    outcomes: BTreeMap<CommandId, R>,
}

impl<R: Clone> IdempotencyLedger<R> {
    pub fn execute_once<F>(&mut self, command_id: CommandId, operation: F) -> Execution<R>
    where
        F: FnOnce() -> R,
    {
        if let Some(existing) = self.outcomes.get(&command_id) {
            return Execution::Duplicate(existing.clone());
        }

        let outcome = operation();
        self.outcomes.insert(command_id, outcome.clone());
        Execution::First(outcome)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Execution<R> {
    First(R),
    Duplicate(R),
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum VersionCheckError {
    #[error("entity version mismatch: expected {expected:?}, actual {actual:?}")]
    Conflict {
        expected: EntityVersion,
        actual: EntityVersion,
    },
}

pub fn require_version(
    expected: Option<EntityVersion>,
    actual: EntityVersion,
) -> Result<(), VersionCheckError> {
    match expected {
        Some(expected) if expected != actual => {
            Err(VersionCheckError::Conflict { expected, actual })
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct TestCommand {
        amount: u32,
    }

    #[test]
    fn command_envelope_round_trips() {
        let envelope = CommandEnvelope {
            command_id: CommandId::from_u128(1),
            actor_id: PersonId::from_u128(2),
            issued_at: WorldInstant::from_ticks(300),
            expected_entity_version: Some(EntityVersion(7)),
            schema_version: SchemaVersion(1),
            payload: TestCommand { amount: 4 },
        };
        let json = serde_json::to_string(&envelope).expect("serialize command");
        let decoded: CommandEnvelope<TestCommand> =
            serde_json::from_str(&json).expect("deserialize command");
        assert_eq!(decoded, envelope);
    }

    #[test]
    fn duplicate_command_reuses_original_outcome() {
        let id = CommandId::from_u128(99);
        let mut ledger = IdempotencyLedger::default();
        let first = ledger.execute_once(id, || 10_u32);
        let duplicate = ledger.execute_once(id, || 999_u32);
        assert_eq!(first, Execution::First(10));
        assert_eq!(duplicate, Execution::Duplicate(10));
    }

    #[test]
    fn expected_version_is_enforced() {
        assert!(require_version(Some(EntityVersion(3)), EntityVersion(4)).is_err());
        assert!(require_version(Some(EntityVersion(4)), EntityVersion(4)).is_ok());
        assert!(require_version(None, EntityVersion(4)).is_ok());
    }
}
