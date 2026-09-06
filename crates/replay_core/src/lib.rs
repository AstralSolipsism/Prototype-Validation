#![forbid(unsafe_code)]

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::{collections::BTreeSet, fmt};
use thiserror::Error;
use world_ids::CommandId;
use world_time::WorldInstant;

const FNV_128_OFFSET_BASIS: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;
const FNV_128_PRIME: u128 = 0x0000_0000_0100_0000_0000_0000_0000_013b;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StateFingerprint(u128);

impl StateFingerprint {
    pub const fn as_u128(self) -> u128 {
        self.0
    }
}

impl Serialize for StateFingerprint {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("{:032x}", self.0))
    }
}

impl<'de> Deserialize<'de> for StateFingerprint {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct FingerprintVisitor;

        impl<'de> de::Visitor<'de> for FingerprintVisitor {
            type Value = StateFingerprint;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a 32-character lowercase hexadecimal fingerprint")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if value.len() != 32
                    || !value
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                {
                    return Err(E::custom("invalid state fingerprint"));
                }
                u128::from_str_radix(value, 16)
                    .map(StateFingerprint)
                    .map_err(E::custom)
            }
        }

        deserializer.deserialize_str(FingerprintVisitor)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FingerprintBuilder {
    state: u128,
}

impl Default for FingerprintBuilder {
    fn default() -> Self {
        Self {
            state: FNV_128_OFFSET_BASIS,
        }
    }
}

impl FingerprintBuilder {
    pub fn write_bytes(&mut self, bytes: &[u8]) {
        self.write_u64(bytes.len() as u64);
        for byte in bytes {
            self.state ^= u128::from(*byte);
            self.state = self.state.wrapping_mul(FNV_128_PRIME);
        }
    }

    pub fn write_u64(&mut self, value: u64) {
        for byte in value.to_le_bytes() {
            self.state ^= u128::from(byte);
            self.state = self.state.wrapping_mul(FNV_128_PRIME);
        }
    }

    pub const fn finish(self) -> StateFingerprint {
        StateFingerprint(self.state)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayEntry<C> {
    pub sequence: u64,
    pub command_id: CommandId,
    pub recorded_at: WorldInstant,
    pub command: C,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplayJournal<C> {
    entries: Vec<ReplayEntry<C>>,
    command_ids: BTreeSet<CommandId>,
}

impl<C> Default for ReplayJournal<C> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            command_ids: BTreeSet::new(),
        }
    }
}

impl<C> ReplayJournal<C> {
    pub fn append(
        &mut self,
        command_id: CommandId,
        recorded_at: WorldInstant,
        command: C,
    ) -> Result<&ReplayEntry<C>, ReplayError> {
        if !self.command_ids.insert(command_id) {
            return Err(ReplayError::DuplicateCommand(command_id));
        }
        let sequence = self.entries.len() as u64;
        self.entries.push(ReplayEntry {
            sequence,
            command_id,
            recorded_at,
            command,
        });
        Ok(self.entries.last().expect("entry was just appended"))
    }

    pub fn append_loaded(&mut self, entry: ReplayEntry<C>) -> Result<(), ReplayError> {
        let expected = self.entries.len() as u64;
        if entry.sequence != expected {
            return Err(ReplayError::NonContiguousSequence {
                expected,
                actual: entry.sequence,
            });
        }
        if !self.command_ids.insert(entry.command_id) {
            return Err(ReplayError::DuplicateCommand(entry.command_id));
        }
        self.entries.push(entry);
        Ok(())
    }

    pub fn entries(&self) -> &[ReplayEntry<C>] {
        &self.entries
    }
}

impl<C> Serialize for ReplayJournal<C>
where
    C: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.entries.serialize(serializer)
    }
}

impl<'de, C> Deserialize<'de> for ReplayJournal<C>
where
    C: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let entries = Vec::<ReplayEntry<C>>::deserialize(deserializer)?;
        let mut journal = Self::default();
        for entry in entries {
            journal.append_loaded(entry).map_err(de::Error::custom)?;
        }
        Ok(journal)
    }
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum ReplayError {
    #[error("replay contains duplicate command {0}")]
    DuplicateCommand(CommandId),
    #[error("replay sequence is not contiguous: expected {expected}, actual {actual}")]
    NonContiguousSequence { expected: u64, actual: u64 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_is_stable() {
        let mut builder = FingerprintBuilder::default();
        builder.write_bytes(b"prototype-validation");
        builder.write_u64(42);
        assert_eq!(
            builder.finish().as_u128(),
            0xfaea_2067_3264_f624_1d4c_a862_a850_01f1
        );
    }

    #[test]
    fn replay_rejects_duplicate_commands() {
        let mut journal = ReplayJournal::default();
        let id = CommandId::from_u128(1);
        journal
            .append(id, WorldInstant::from_ticks(10), "first")
            .expect("first command");
        assert_eq!(
            journal.append(id, WorldInstant::from_ticks(11), "duplicate"),
            Err(ReplayError::DuplicateCommand(id))
        );
    }

    #[test]
    fn deserialize_rebuilds_and_validates_indexes() {
        let json = r#"[
            {
                "sequence": 0,
                "command_id": "00000000000000000000000000000001",
                "recorded_at": 10,
                "command": "first"
            }
        ]"#;
        let journal: ReplayJournal<String> =
            serde_json::from_str(json).expect("valid replay must deserialize");
        assert_eq!(journal.entries().len(), 1);

        let invalid = json.replace("\"sequence\": 0", "\"sequence\": 2");
        assert!(serde_json::from_str::<ReplayJournal<String>>(&invalid).is_err());
    }
}
