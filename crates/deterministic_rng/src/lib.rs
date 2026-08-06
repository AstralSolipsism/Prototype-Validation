#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use thiserror::Error;

const GOLDEN_GAMMA: u64 = 0x9e37_79b9_7f4a_7c15;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SeedMaterial {
    pub world_seed: u128,
    pub stage_id: u64,
    pub spatial_key: u128,
    pub feature_key: u128,
}

impl SeedMaterial {
    pub fn derive(self) -> u64 {
        let parts = [
            self.world_seed as u64,
            (self.world_seed >> 64) as u64,
            self.stage_id,
            self.spatial_key as u64,
            (self.spatial_key >> 64) as u64,
            self.feature_key as u64,
            (self.feature_key >> 64) as u64,
        ];

        parts.into_iter().enumerate().fold(
            0x6a09_e667_f3bc_c909,
            |state, (index, part)| {
                mix64(state ^ mix64(part.wrapping_add(GOLDEN_GAMMA.wrapping_mul(index as u64 + 1))))
            },
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeterministicRng {
    state: u64,
}

impl DeterministicRng {
    pub const fn from_seed(seed: u64) -> Self {
        Self { state: seed }
    }

    pub fn from_material(material: SeedMaterial) -> Self {
        Self::from_seed(material.derive())
    }

    pub const fn state(self) -> u64 {
        self.state
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(GOLDEN_GAMMA);
        mix64(self.state)
    }

    pub fn next_unit_f64(&mut self) -> f64 {
        const SCALE: f64 = 1.0 / ((1_u64 << 53) as f64);
        ((self.next_u64() >> 11) as f64) * SCALE
    }

    pub fn range_u64(&mut self, start: u64, end_exclusive: u64) -> Result<u64, RangeError> {
        let width = end_exclusive
            .checked_sub(start)
            .filter(|width| *width > 0)
            .ok_or(RangeError::EmptyOrReversed {
                start,
                end_exclusive,
            })?;

        let threshold = width.wrapping_neg() % width;
        loop {
            let value = self.next_u64();
            if value >= threshold {
                return Ok(start + value % width);
            }
        }
    }

    pub fn fork(&self, stream_key: u128) -> Self {
        let seed =
            mix64(self.state ^ stream_key as u64 ^ mix64((stream_key >> 64) as u64));
        Self::from_seed(seed)
    }
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum RangeError {
    #[error("invalid integer range {start}..{end_exclusive}")]
    EmptyOrReversed { start: u64, end_exclusive: u64 },
}

fn mix64(mut value: u64) -> u64 {
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitmix_sequence_is_stable() {
        let mut rng = DeterministicRng::from_seed(0x0123_4567_89ab_cdef);
        assert_eq!(rng.next_u64(), 0x157a_3807_a48f_aa9d);
        assert_eq!(rng.next_u64(), 0xd573_529b_34a1_d093);
        assert_eq!(rng.next_u64(), 0x2f90_b72e_996d_ccbe);
    }

    #[test]
    fn feature_streams_do_not_depend_on_other_stream_consumption() {
        let root = DeterministicRng::from_seed(99);
        let mut vegetation = root.fork(1);
        let mut buildings = root.fork(2);

        let first_building = buildings.next_u64();
        for _ in 0..100 {
            vegetation.next_u64();
        }

        let mut buildings_again = root.fork(2);
        assert_eq!(first_building, buildings_again.next_u64());
    }

    #[test]
    fn rejects_empty_range() {
        let mut rng = DeterministicRng::from_seed(0);
        assert!(matches!(
            rng.range_u64(4, 4),
            Err(RangeError::EmptyOrReversed { .. })
        ));
    }
}
