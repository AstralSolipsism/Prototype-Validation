#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::num::NonZeroU32;
use thiserror::Error;

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct WorldInstant(i64);

impl WorldInstant {
    pub const ZERO: Self = Self(0);

    pub const fn from_ticks(ticks: i64) -> Self {
        Self(ticks)
    }

    pub const fn ticks(self) -> i64 {
        self.0
    }

    pub fn checked_add(self, duration: WorldDuration) -> Option<Self> {
        self.0.checked_add(duration.0).map(Self)
    }
}

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct WorldDuration(i64);

impl WorldDuration {
    pub const ZERO: Self = Self(0);

    pub const fn from_ticks(ticks: i64) -> Self {
        Self(ticks)
    }

    pub const fn ticks(self) -> i64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TickRate(NonZeroU32);

impl TickRate {
    pub const fn new(ticks_per_second: NonZeroU32) -> Self {
        Self(ticks_per_second)
    }

    pub const fn ticks_per_second(self) -> NonZeroU32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FixedStepClock {
    now: WorldInstant,
    step: WorldDuration,
}

impl FixedStepClock {
    pub fn new(now: WorldInstant, step: WorldDuration) -> Result<Self, ClockError> {
        if step.ticks() <= 0 {
            return Err(ClockError::NonPositiveStep(step.ticks()));
        }
        Ok(Self { now, step })
    }

    pub const fn now(self) -> WorldInstant {
        self.now
    }

    pub const fn step(self) -> WorldDuration {
        self.step
    }

    pub fn advance(&mut self) -> Result<WorldInstant, ClockError> {
        self.now = self
            .now
            .checked_add(self.step)
            .ok_or(ClockError::Overflow)?;
        Ok(self.now)
    }

    pub fn advance_by(&mut self, steps: u64) -> Result<WorldInstant, ClockError> {
        let delta = i64::try_from(steps)
            .ok()
            .and_then(|steps| self.step.ticks().checked_mul(steps))
            .ok_or(ClockError::Overflow)?;
        self.now = self
            .now
            .checked_add(WorldDuration::from_ticks(delta))
            .ok_or(ClockError::Overflow)?;
        Ok(self.now)
    }
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum ClockError {
    #[error("fixed step must be positive, got {0}")]
    NonPositiveStep(i64),
    #[error("world time overflow")]
    Overflow,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_step_advances_discrete_world_time() {
        let mut clock =
            FixedStepClock::new(WorldInstant::from_ticks(100), WorldDuration::from_ticks(5))
                .expect("valid clock");

        assert_eq!(
            clock.advance().expect("advance"),
            WorldInstant::from_ticks(105)
        );
        assert_eq!(
            clock.advance_by(3).expect("advance by"),
            WorldInstant::from_ticks(120)
        );
    }

    #[test]
    fn rejects_non_positive_step() {
        let error = FixedStepClock::new(WorldInstant::ZERO, WorldDuration::ZERO)
            .expect_err("zero step must fail");
        assert_eq!(error, ClockError::NonPositiveStep(0));
    }
}
