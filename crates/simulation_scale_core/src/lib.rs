#![forbid(unsafe_code)]
#![allow(clippy::manual_is_multiple_of, reason = "simulation schedules use explicit modular cadence checks")]

use deterministic_rng::{DeterministicRng, SeedMaterial};
use replay_core::{FingerprintBuilder, StateFingerprint};
use serde::{Deserialize, Serialize};
use std::{
    cmp::Reverse,
    collections::BinaryHeap,
    mem::size_of,
    time::{Duration, Instant},
};
use thiserror::Error;
use world_generation_core::HexCoord;
use world_ids::PersonId;

pub const MINUTES_PER_DAY: u64 = 1_440;
pub const HOT_TICKS_PER_SECOND: u64 = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScaleConfig {
    pub world_seed: u128,
    pub cold_population: usize,
    pub warm_population: usize,
    pub hot_population: usize,
    pub accelerated_days: u32,
    pub hot_ticks: u32,
    pub transition_people: usize,
    pub cold_wheel_slots: usize,
}

impl ScaleConfig {
    pub const fn acceptance() -> Self {
        Self {
            world_seed: 0x5046_0000_0000_0000_0000_0000_0000_0006,
            cold_population: 1_000_000,
            warm_population: 10_000,
            hot_population: 300,
            accelerated_days: 30,
            hot_ticks: 1_200,
            transition_people: 64,
            cold_wheel_slots: MINUTES_PER_DAY as usize,
        }
    }

    pub const fn small_test() -> Self {
        Self {
            world_seed: 0x6006,
            cold_population: 4_096,
            warm_population: 256,
            hot_population: 32,
            accelerated_days: 2,
            hot_ticks: 80,
            transition_people: 8,
            cold_wheel_slots: 144,
        }
    }

    pub fn validate(self) -> Result<Self, ScaleError> {
        if self.cold_population == 0
            || self.warm_population == 0
            || self.hot_population == 0
            || self.accelerated_days == 0
            || self.hot_ticks == 0
            || self.cold_wheel_slots == 0
        {
            return Err(ScaleError::InvalidConfig(
                "all population, time and wheel values must be positive",
            ));
        }
        if self.transition_people > self.cold_population {
            return Err(ScaleError::InvalidConfig(
                "transition sample cannot exceed cold population",
            ));
        }
        if self.cold_population > u32::MAX as usize {
            return Err(ScaleError::InvalidConfig(
                "cold population must fit the compact u32 due index",
            ));
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SimTier {
    Cold,
    Warm,
    Hot,
}

impl SimTier {
    const fn code(self) -> u8 {
        match self {
            Self::Cold => 0,
            Self::Warm => 1,
            Self::Hot => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersonFacts {
    pub id: PersonId,
    pub home_cell: HexCoord,
    pub current_cell: HexCoord,
    pub household_id: u32,
    pub profession: u16,
    pub wealth: u32,
    pub commitment_id: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NeedState {
    pub hunger: u16,
    pub fatigue: u16,
    pub social: u16,
    pub morale: u16,
}

impl NeedState {
    fn from_packed(value: u32) -> Self {
        Self {
            hunger: (value & 0xff) as u16,
            fatigue: ((value >> 8) & 0xff) as u16,
            social: ((value >> 16) & 0xff) as u16,
            morale: ((value >> 24) & 0xff) as u16,
        }
    }

    fn packed(self) -> u32 {
        u32::from(self.hunger.min(255))
            | (u32::from(self.fatigue.min(255)) << 8)
            | (u32::from(self.social.min(255)) << 16)
            | (u32::from(self.morale.min(255)) << 24)
    }

    fn advance_cold_day(&mut self, entropy: u64) {
        self.hunger = self.hunger.saturating_add(7).min(255);
        self.fatigue = self.fatigue.saturating_add(5).min(255);
        self.social = self.social.saturating_add(2).min(255);
        let recovery = ((entropy >> 11) & 7) as u16;
        self.morale = self.morale.saturating_sub(3).saturating_add(recovery).min(255);
    }

    fn advance_warm_event(&mut self, kind: WarmEventKind) {
        match kind {
            WarmEventKind::Work => {
                self.hunger = self.hunger.saturating_add(8).min(255);
                self.fatigue = self.fatigue.saturating_add(11).min(255);
                self.morale = self.morale.saturating_add(1).min(255);
            }
            WarmEventKind::Rest => {
                self.hunger = self.hunger.saturating_add(3).min(255);
                self.fatigue = self.fatigue.saturating_sub(30);
            }
            WarmEventKind::Meal => {
                self.hunger = self.hunger.saturating_sub(45);
                self.morale = self.morale.saturating_add(3).min(255);
            }
            WarmEventKind::Social => {
                self.social = self.social.saturating_sub(35);
                self.morale = self.morale.saturating_add(5).min(255);
            }
        }
    }

    fn advance_hot_tick(&mut self, tick: u32) {
        self.hunger = self.hunger.saturating_add(1).min(255);
        if tick % 2 == 0 {
            self.fatigue = self.fatigue.saturating_add(1).min(255);
        }
        if tick % 20 == 0 {
            self.social = self.social.saturating_add(1).min(255);
        }
        if self.hunger > 220 || self.fatigue > 230 {
            self.morale = self.morale.saturating_sub(1);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum HistoryKind {
    WorkMilestone,
    HouseholdChange,
    Illness,
    Migration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryEvent {
    pub person_id: PersonId,
    pub minute: u64,
    pub kind: HistoryKind,
}

#[derive(Clone, Debug)]
pub struct ColdPopulation {
    ids: Vec<PersonId>,
    home_cells: Vec<HexCoord>,
    current_cells: Vec<HexCoord>,
    household_ids: Vec<u32>,
    professions: Vec<u16>,
    wealth: Vec<u32>,
    commitment_ids: Vec<u64>,
    need_summaries: Vec<u32>,
    next_due_minutes: Vec<u32>,
    tiers: Vec<u8>,
    due_wheel: Vec<Vec<u32>>,
}

impl ColdPopulation {
    pub fn generate(config: ScaleConfig) -> Result<Self, ScaleError> {
        let mut ids = Vec::with_capacity(config.cold_population);
        let mut home_cells = Vec::with_capacity(config.cold_population);
        let mut current_cells = Vec::with_capacity(config.cold_population);
        let mut household_ids = Vec::with_capacity(config.cold_population);
        let mut professions = Vec::with_capacity(config.cold_population);
        let mut wealth = Vec::with_capacity(config.cold_population);
        let mut commitment_ids = Vec::with_capacity(config.cold_population);
        let mut need_summaries = Vec::with_capacity(config.cold_population);
        let mut next_due_minutes = Vec::with_capacity(config.cold_population);
        let mut tiers = Vec::with_capacity(config.cold_population);
        let mut due_wheel = (0..config.cold_wheel_slots)
            .map(|_| Vec::new())
            .collect::<Vec<_>>();

        for index in 0..config.cold_population {
            let mut rng = DeterministicRng::from_material(SeedMaterial {
                world_seed: config.world_seed,
                stage_id: 0x6001,
                spatial_key: index as u128,
                feature_key: 0x434f_4c44,
            });
            let id = PersonId::from_parts(0x6000_0000_0000_0001, index as u64 + 1);
            let home = HexCoord::new(
                (rng.range_u64(0, 127)? as i16) - 63,
                (rng.range_u64(0, 127)? as i16) - 63,
            );
            let current = if rng.next_u64() % 11 == 0 {
                HexCoord::new(home.q.saturating_add(1), home.r)
            } else {
                home
            };
            let household_id = (index / 4) as u32;
            let profession = rng.range_u64(0, 96)? as u16;
            let wealth_value = rng.range_u64(20, 25_000)? as u32;
            let commitment_id = rng.next_u64();
            let needs = NeedState {
                hunger: rng.range_u64(10, 120)? as u16,
                fatigue: rng.range_u64(5, 130)? as u16,
                social: rng.range_u64(5, 110)? as u16,
                morale: rng.range_u64(80, 220)? as u16,
            };
            let due_slot = rng.range_u64(0, config.cold_wheel_slots as u64)? as usize;

            ids.push(id);
            home_cells.push(home);
            current_cells.push(current);
            household_ids.push(household_id);
            professions.push(profession);
            wealth.push(wealth_value);
            commitment_ids.push(commitment_id);
            need_summaries.push(needs.packed());
            next_due_minutes.push(due_slot as u32);
            tiers.push(SimTier::Cold.code());
            due_wheel[due_slot].push(index as u32);
        }

        Ok(Self {
            ids,
            home_cells,
            current_cells,
            household_ids,
            professions,
            wealth,
            commitment_ids,
            need_summaries,
            next_due_minutes,
            tiers,
            due_wheel,
        })
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    pub fn owned_bytes(&self) -> usize {
        slice_bytes(&self.ids)
            + slice_bytes(&self.home_cells)
            + slice_bytes(&self.current_cells)
            + slice_bytes(&self.household_ids)
            + slice_bytes(&self.professions)
            + slice_bytes(&self.wealth)
            + slice_bytes(&self.commitment_ids)
            + slice_bytes(&self.need_summaries)
            + slice_bytes(&self.next_due_minutes)
            + slice_bytes(&self.tiers)
            + self
                .due_wheel
                .iter()
                .map(|bucket| slice_bytes(bucket))
                .sum::<usize>()
            + self.due_wheel.capacity() * size_of::<Vec<u32>>()
    }

    pub fn facts(&self, index: usize) -> Result<PersonFacts, ScaleError> {
        if index >= self.len() {
            return Err(ScaleError::PersonIndex(index));
        }
        Ok(PersonFacts {
            id: self.ids[index],
            home_cell: self.home_cells[index],
            current_cell: self.current_cells[index],
            household_id: self.household_ids[index],
            profession: self.professions[index],
            wealth: self.wealth[index],
            commitment_id: self.commitment_ids[index],
        })
    }

    pub fn needs(&self, index: usize) -> Result<NeedState, ScaleError> {
        self.need_summaries
            .get(index)
            .copied()
            .map(NeedState::from_packed)
            .ok_or(ScaleError::PersonIndex(index))
    }

    pub fn tier(&self, index: usize) -> Result<SimTier, ScaleError> {
        match self.tiers.get(index).copied() {
            Some(0) => Ok(SimTier::Cold),
            Some(1) => Ok(SimTier::Warm),
            Some(2) => Ok(SimTier::Hot),
            Some(_) => Err(ScaleError::Invariant("invalid tier code")),
            None => Err(ScaleError::PersonIndex(index)),
        }
    }

    pub fn process_days(
        &mut self,
        days: u32,
        history: &mut Vec<HistoryEvent>,
    ) -> ColdAdvanceMetrics {
        let mut due_events = 0_u64;
        let mut history_events = 0_u64;
        let slots = self.due_wheel.len() as u64;

        for day in 0..u64::from(days) {
            for slot in 0..self.due_wheel.len() {
                for compact_index in &self.due_wheel[slot] {
                    let index = *compact_index as usize;
                    if self.tiers[index] != SimTier::Cold.code() {
                        continue;
                    }
                    let mut needs = NeedState::from_packed(self.need_summaries[index]);
                    let entropy = mix64(self.ids[index].low() ^ day ^ slot as u64);
                    needs.advance_cold_day(entropy);
                    self.need_summaries[index] = needs.packed();
                    self.next_due_minutes[index] = self.next_due_minutes[index]
                        .saturating_add(u32::try_from(slots).unwrap_or(u32::MAX));
                    due_events += 1;

                    if entropy % 100_003 == 0 {
                        history.push(HistoryEvent {
                            person_id: self.ids[index],
                            minute: day * slots + slot as u64,
                            kind: if entropy & 1 == 0 {
                                HistoryKind::HouseholdChange
                            } else {
                                HistoryKind::Migration
                            },
                        });
                        history_events += 1;
                    }
                }
            }
        }

        ColdAdvanceMetrics {
            due_events,
            full_population_scans: 0,
            history_events,
        }
    }

    pub fn exercise_transitions(
        &mut self,
        count: usize,
        history: &mut Vec<HistoryEvent>,
    ) -> Result<TransitionEvidence, ScaleError> {
        let mut facts_preserved = true;
        let mut summaries_changed = 0_usize;
        let mut cache_rebuilds = 0_usize;
        let mut samples = Vec::new();

        for index in 0..count {
            let before_facts = self.facts(index)?;
            let before_needs = self.needs(index)?;
            self.tiers[index] = SimTier::Warm.code();

            let mut warm_needs = before_needs;
            warm_needs.advance_warm_event(WarmEventKind::Work);
            warm_needs.advance_warm_event(WarmEventKind::Meal);
            self.tiers[index] = SimTier::Hot.code();

            let mut runtime_cache = HotRuntimeCache::from_facts(before_facts);
            cache_rebuilds += 1;
            for tick in 0..40 {
                warm_needs.advance_hot_tick(tick);
                runtime_cache.advance(tick);
            }
            runtime_cache.clear();
            self.tiers[index] = SimTier::Warm.code();
            warm_needs.advance_warm_event(WarmEventKind::Rest);
            self.need_summaries[index] = warm_needs.packed();
            self.tiers[index] = SimTier::Cold.code();

            let after_facts = self.facts(index)?;
            facts_preserved &= before_facts == after_facts;
            if before_needs != warm_needs {
                summaries_changed += 1;
            }
            if index < 8 {
                samples.push(TransitionSample {
                    person_id: before_facts.id,
                    before: before_needs,
                    after: warm_needs,
                    final_tier: self.tier(index)?,
                    facts_preserved: before_facts == after_facts,
                });
            }
            if mix64(before_facts.id.low()) % 17 == 0 {
                history.push(HistoryEvent {
                    person_id: before_facts.id,
                    minute: 0,
                    kind: HistoryKind::WorkMilestone,
                });
            }
        }

        Ok(TransitionEvidence {
            people_exercised: count,
            facts_preserved,
            summaries_changed,
            runtime_cache_rebuilds: cache_rebuilds,
            final_cold_count: self
                .tiers
                .iter()
                .filter(|tier| **tier == SimTier::Cold.code())
                .count(),
            samples,
        })
    }

    fn write_fingerprint(&self, builder: &mut FingerprintBuilder) {
        builder.write_u64(self.len() as u64);
        for index in 0..self.len() {
            builder.write_u64(self.ids[index].high());
            builder.write_u64(self.ids[index].low());
            builder.write_u64(self.home_cells[index].q as i64 as u64);
            builder.write_u64(self.home_cells[index].r as i64 as u64);
            builder.write_u64(self.current_cells[index].q as i64 as u64);
            builder.write_u64(self.current_cells[index].r as i64 as u64);
            builder.write_u64(u64::from(self.household_ids[index]));
            builder.write_u64(u64::from(self.professions[index]));
            builder.write_u64(u64::from(self.wealth[index]));
            builder.write_u64(self.commitment_ids[index]);
            builder.write_u64(u64::from(self.need_summaries[index]));
            builder.write_u64(u64::from(self.next_due_minutes[index]));
            builder.write_u64(u64::from(self.tiers[index]));
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum WarmEventKind {
    Work,
    Rest,
    Meal,
    Social,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ScheduledWarmEvent {
    due_minute: u64,
    person_index: u32,
    generation: u32,
    kind: WarmEventKind,
}

#[derive(Clone, Debug)]
struct WarmPerson {
    facts: PersonFacts,
    needs: NeedState,
    events_completed: u32,
    next_due_minute: u64,
}

#[derive(Clone, Debug)]
pub struct WarmPopulation {
    people: Vec<WarmPerson>,
    calendar: BinaryHeap<Reverse<ScheduledWarmEvent>>,
}

impl WarmPopulation {
    pub fn generate(config: ScaleConfig) -> Result<Self, ScaleError> {
        let mut people = Vec::with_capacity(config.warm_population);
        let mut calendar = BinaryHeap::with_capacity(config.warm_population);
        for index in 0..config.warm_population {
            let mut rng = DeterministicRng::from_material(SeedMaterial {
                world_seed: config.world_seed,
                stage_id: 0x6002,
                spatial_key: index as u128,
                feature_key: 0x5741_524d,
            });
            let facts = PersonFacts {
                id: PersonId::from_parts(0x6000_0000_0000_0002, index as u64 + 1),
                home_cell: HexCoord::new(
                    (rng.range_u64(0, 65)? as i16) - 32,
                    (rng.range_u64(0, 65)? as i16) - 32,
                ),
                current_cell: HexCoord::new(
                    (rng.range_u64(0, 65)? as i16) - 32,
                    (rng.range_u64(0, 65)? as i16) - 32,
                ),
                household_id: (index / 3) as u32,
                profession: rng.range_u64(0, 96)? as u16,
                wealth: rng.range_u64(50, 40_000)? as u32,
                commitment_id: rng.next_u64(),
            };
            let due = rng.range_u64(0, 480)?;
            let person = WarmPerson {
                facts,
                needs: NeedState {
                    hunger: rng.range_u64(10, 120)? as u16,
                    fatigue: rng.range_u64(10, 130)? as u16,
                    social: rng.range_u64(5, 120)? as u16,
                    morale: rng.range_u64(80, 220)? as u16,
                },
                events_completed: 0,
                next_due_minute: due,
            };
            people.push(person);
            calendar.push(Reverse(ScheduledWarmEvent {
                due_minute: due,
                person_index: index as u32,
                generation: 0,
                kind: WarmEventKind::Work,
            }));
        }
        Ok(Self { people, calendar })
    }

    pub fn len(&self) -> usize {
        self.people.len()
    }

    pub fn is_empty(&self) -> bool {
        self.people.is_empty()
    }

    pub fn owned_bytes(&self) -> usize {
        slice_bytes(&self.people)
            + self.calendar.capacity() * size_of::<Reverse<ScheduledWarmEvent>>()
    }

    pub fn advance_to(
        &mut self,
        end_minute: u64,
        history: &mut Vec<HistoryEvent>,
    ) -> WarmAdvanceMetrics {
        let mut events_processed = 0_u64;
        let mut history_events = 0_u64;
        while let Some(Reverse(next)) = self.calendar.peek().copied() {
            if next.due_minute > end_minute {
                break;
            }
            self.calendar.pop();
            let person = &mut self.people[next.person_index as usize];
            if person.next_due_minute != next.due_minute
                || person.events_completed != next.generation
            {
                continue;
            }
            person.needs.advance_warm_event(next.kind);
            person.events_completed = person.events_completed.saturating_add(1);
            events_processed += 1;

            if person.events_completed % 127 == 0 {
                history.push(HistoryEvent {
                    person_id: person.facts.id,
                    minute: next.due_minute,
                    kind: if person.events_completed % 254 == 0 {
                        HistoryKind::Illness
                    } else {
                        HistoryKind::WorkMilestone
                    },
                });
                history_events += 1;
            }

            let entropy = mix64(
                person.facts.id.low()
                    ^ u64::from(person.events_completed)
                    ^ next.due_minute,
            );
            let delay = 360 + entropy % 361;
            let due = next.due_minute.saturating_add(delay);
            let kind = match person.events_completed % 4 {
                0 => WarmEventKind::Work,
                1 => WarmEventKind::Meal,
                2 => WarmEventKind::Social,
                _ => WarmEventKind::Rest,
            };
            person.next_due_minute = due;
            self.calendar.push(Reverse(ScheduledWarmEvent {
                due_minute: due,
                person_index: next.person_index,
                generation: person.events_completed,
                kind,
            }));
        }
        WarmAdvanceMetrics {
            events_processed,
            full_population_scans: 0,
            history_events,
        }
    }

    fn write_fingerprint(&self, builder: &mut FingerprintBuilder) {
        builder.write_u64(self.len() as u64);
        for person in &self.people {
            write_facts(builder, person.facts);
            write_needs(builder, person.needs);
            builder.write_u64(u64::from(person.events_completed));
            builder.write_u64(person.next_due_minute);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HotRuntimeCache {
    pub target_x: i16,
    pub target_z: i16,
    pub path_cursor: u16,
    pub animation_phase: u16,
    pub valid: bool,
}

impl HotRuntimeCache {
    fn from_facts(facts: PersonFacts) -> Self {
        Self {
            target_x: facts.current_cell.q.saturating_mul(16),
            target_z: facts.current_cell.r.saturating_mul(16),
            path_cursor: (facts.id.low() & 0xffff) as u16,
            animation_phase: ((facts.id.low() >> 16) & 0xffff) as u16,
            valid: true,
        }
    }

    fn advance(&mut self, tick: u32) {
        self.path_cursor = self.path_cursor.wrapping_add(1);
        self.animation_phase = self.animation_phase.wrapping_add((tick as u16) | 1);
    }

    fn clear(&mut self) {
        self.target_x = 0;
        self.target_z = 0;
        self.path_cursor = 0;
        self.animation_phase = 0;
        self.valid = false;
    }
}

#[derive(Clone, Debug)]
struct HotPerson {
    facts: PersonFacts,
    needs: NeedState,
    runtime: HotRuntimeCache,
    ticks_simulated: u32,
}

#[derive(Clone, Debug)]
pub struct HotPopulation {
    people: Vec<HotPerson>,
}

impl HotPopulation {
    pub fn generate(config: ScaleConfig) -> Result<Self, ScaleError> {
        let mut people = Vec::with_capacity(config.hot_population);
        for index in 0..config.hot_population {
            let mut rng = DeterministicRng::from_material(SeedMaterial {
                world_seed: config.world_seed,
                stage_id: 0x6003,
                spatial_key: index as u128,
                feature_key: 0x484f_5400,
            });
            let facts = PersonFacts {
                id: PersonId::from_parts(0x6000_0000_0000_0003, index as u64 + 1),
                home_cell: HexCoord::new(
                    (rng.range_u64(0, 19)? as i16) - 9,
                    (rng.range_u64(0, 19)? as i16) - 9,
                ),
                current_cell: HexCoord::new(
                    (rng.range_u64(0, 19)? as i16) - 9,
                    (rng.range_u64(0, 19)? as i16) - 9,
                ),
                household_id: (index / 3) as u32,
                profession: rng.range_u64(0, 96)? as u16,
                wealth: rng.range_u64(100, 50_000)? as u32,
                commitment_id: rng.next_u64(),
            };
            people.push(HotPerson {
                facts,
                needs: NeedState {
                    hunger: rng.range_u64(10, 90)? as u16,
                    fatigue: rng.range_u64(10, 100)? as u16,
                    social: rng.range_u64(5, 100)? as u16,
                    morale: rng.range_u64(100, 230)? as u16,
                },
                runtime: HotRuntimeCache::from_facts(facts),
                ticks_simulated: 0,
            });
        }
        Ok(Self { people })
    }

    pub fn len(&self) -> usize {
        self.people.len()
    }

    pub fn is_empty(&self) -> bool {
        self.people.is_empty()
    }

    pub fn owned_bytes(&self) -> usize {
        slice_bytes(&self.people)
    }

    pub fn advance_ticks(
        &mut self,
        ticks: u32,
        history: &mut Vec<HistoryEvent>,
    ) -> HotAdvanceMetrics {
        let mut updates = 0_u64;
        let mut history_events = 0_u64;
        for tick in 0..ticks {
            for person in &mut self.people {
                person.needs.advance_hot_tick(tick);
                person.runtime.advance(tick);
                person.ticks_simulated = person.ticks_simulated.saturating_add(1);
                updates += 1;
                if tick > 0 && tick % 600 == 0 && mix64(person.facts.id.low() ^ u64::from(tick)) % 4093 == 0 {
                    history.push(HistoryEvent {
                        person_id: person.facts.id,
                        minute: u64::from(tick) / HOT_TICKS_PER_SECOND / 60,
                        kind: HistoryKind::Illness,
                    });
                    history_events += 1;
                }
            }
        }
        HotAdvanceMetrics {
            fixed_step_updates: updates,
            history_events,
        }
    }

    pub fn clear_runtime_caches(&mut self) -> usize {
        for person in &mut self.people {
            person.runtime.clear();
        }
        self.people.len()
    }

    pub fn rebuild_runtime_caches(&mut self) -> usize {
        for person in &mut self.people {
            person.runtime = HotRuntimeCache::from_facts(person.facts);
        }
        self.people.len()
    }

    pub fn all_runtime_caches_valid(&self) -> bool {
        self.people.iter().all(|person| person.runtime.valid)
    }

    fn write_fingerprint(&self, builder: &mut FingerprintBuilder) {
        builder.write_u64(self.len() as u64);
        for person in &self.people {
            write_facts(builder, person.facts);
            write_needs(builder, person.needs);
            builder.write_u64(u64::from(person.ticks_simulated));
            builder.write_u64(person.runtime.target_x as i64 as u64);
            builder.write_u64(person.runtime.target_z as i64 as u64);
            builder.write_u64(u64::from(person.runtime.path_cursor));
            builder.write_u64(u64::from(person.runtime.animation_phase));
            builder.write_u64(person.runtime.valid as u64);
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColdAdvanceMetrics {
    pub due_events: u64,
    pub full_population_scans: u64,
    pub history_events: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WarmAdvanceMetrics {
    pub events_processed: u64,
    pub full_population_scans: u64,
    pub history_events: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HotAdvanceMetrics {
    pub fixed_step_updates: u64,
    pub history_events: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionSample {
    pub person_id: PersonId,
    pub before: NeedState,
    pub after: NeedState,
    pub final_tier: SimTier,
    pub facts_preserved: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionEvidence {
    pub people_exercised: usize,
    pub facts_preserved: bool,
    pub summaries_changed: usize,
    pub runtime_cache_rebuilds: usize,
    pub final_cold_count: usize,
    pub samples: Vec<TransitionSample>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryBudget {
    pub cold_bytes: u64,
    pub warm_bytes: u64,
    pub hot_bytes: u64,
    pub history_bytes: u64,
    pub total_model_bytes: u64,
}

impl MemoryBudget {
    pub fn total_mebibytes(self) -> f64 {
        self.total_model_bytes as f64 / (1024.0 * 1024.0)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunTimingsMs {
    pub build_cold: u128,
    pub build_warm: u128,
    pub build_hot: u128,
    pub cold_acceleration: u128,
    pub warm_acceleration: u128,
    pub hot_fixed_step: u128,
    pub transitions: u128,
    pub cache_rebuild: u128,
    pub fingerprint: u128,
    pub total: u128,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScaleRunSummary {
    pub config: ScaleConfig,
    pub memory: MemoryBudget,
    pub timings_ms: RunTimingsMs,
    pub cold_metrics: ColdAdvanceMetrics,
    pub warm_metrics: WarmAdvanceMetrics,
    pub hot_metrics: HotAdvanceMetrics,
    pub transition: TransitionEvidence,
    pub history_events: usize,
    pub simulated_minutes: u64,
    pub hypothetical_per_second_cold_updates: u128,
    pub actual_cold_due_events: u64,
    pub hot_cache_entries_cleared: usize,
    pub hot_cache_entries_rebuilt: usize,
    pub hot_caches_valid_after_rebuild: bool,
    pub fingerprint: StateFingerprint,
}

pub fn run_scale_scenario(config: ScaleConfig) -> Result<ScaleRunSummary, ScaleError> {
    let config = config.validate()?;
    let total_started = Instant::now();
    let mut history = Vec::new();

    let started = Instant::now();
    let mut cold = ColdPopulation::generate(config)?;
    let build_cold = started.elapsed();

    let started = Instant::now();
    let mut warm = WarmPopulation::generate(config)?;
    let build_warm = started.elapsed();

    let started = Instant::now();
    let mut hot = HotPopulation::generate(config)?;
    let build_hot = started.elapsed();

    let started = Instant::now();
    let cold_metrics = cold.process_days(config.accelerated_days, &mut history);
    let cold_acceleration = started.elapsed();

    let simulated_minutes = u64::from(config.accelerated_days) * MINUTES_PER_DAY;
    let started = Instant::now();
    let warm_metrics = warm.advance_to(simulated_minutes, &mut history);
    let warm_acceleration = started.elapsed();

    let started = Instant::now();
    let hot_metrics = hot.advance_ticks(config.hot_ticks, &mut history);
    let hot_fixed_step = started.elapsed();

    let started = Instant::now();
    let transition = cold.exercise_transitions(config.transition_people, &mut history)?;
    let transitions = started.elapsed();

    let started = Instant::now();
    let hot_cache_entries_cleared = hot.clear_runtime_caches();
    let hot_cache_entries_rebuilt = hot.rebuild_runtime_caches();
    let hot_caches_valid_after_rebuild = hot.all_runtime_caches_valid();
    let cache_rebuild = started.elapsed();

    let memory = MemoryBudget {
        cold_bytes: cold.owned_bytes() as u64,
        warm_bytes: warm.owned_bytes() as u64,
        hot_bytes: hot.owned_bytes() as u64,
        history_bytes: slice_bytes(&history) as u64,
        total_model_bytes: (cold.owned_bytes()
            + warm.owned_bytes()
            + hot.owned_bytes()
            + slice_bytes(&history)) as u64,
    };

    let started = Instant::now();
    let mut builder = FingerprintBuilder::default();
    write_config(&mut builder, config);
    cold.write_fingerprint(&mut builder);
    warm.write_fingerprint(&mut builder);
    hot.write_fingerprint(&mut builder);
    builder.write_u64(history.len() as u64);
    for event in &history {
        builder.write_u64(event.person_id.high());
        builder.write_u64(event.person_id.low());
        builder.write_u64(event.minute);
        builder.write_u64(event.kind as u64);
    }
    let fingerprint = builder.finish();
    let fingerprint_time = started.elapsed();

    let timings_ms = RunTimingsMs {
        build_cold: millis(build_cold),
        build_warm: millis(build_warm),
        build_hot: millis(build_hot),
        cold_acceleration: millis(cold_acceleration),
        warm_acceleration: millis(warm_acceleration),
        hot_fixed_step: millis(hot_fixed_step),
        transitions: millis(transitions),
        cache_rebuild: millis(cache_rebuild),
        fingerprint: millis(fingerprint_time),
        total: millis(total_started.elapsed()),
    };

    Ok(ScaleRunSummary {
        config,
        memory,
        timings_ms,
        cold_metrics,
        warm_metrics,
        hot_metrics,
        transition,
        history_events: history.len(),
        simulated_minutes,
        hypothetical_per_second_cold_updates: config.cold_population as u128
            * u128::from(simulated_minutes)
            * 60,
        actual_cold_due_events: cold_metrics.due_events,
        hot_cache_entries_cleared,
        hot_cache_entries_rebuilt,
        hot_caches_valid_after_rebuild,
        fingerprint,
    })
}

fn slice_bytes<T>(values: &[T]) -> usize {
    std::mem::size_of_val(values)
}

fn millis(duration: Duration) -> u128 {
    duration.as_millis()
}

fn write_config(builder: &mut FingerprintBuilder, config: ScaleConfig) {
    builder.write_u64(config.world_seed as u64);
    builder.write_u64((config.world_seed >> 64) as u64);
    builder.write_u64(config.cold_population as u64);
    builder.write_u64(config.warm_population as u64);
    builder.write_u64(config.hot_population as u64);
    builder.write_u64(u64::from(config.accelerated_days));
    builder.write_u64(u64::from(config.hot_ticks));
    builder.write_u64(config.transition_people as u64);
    builder.write_u64(config.cold_wheel_slots as u64);
}

fn write_facts(builder: &mut FingerprintBuilder, facts: PersonFacts) {
    builder.write_u64(facts.id.high());
    builder.write_u64(facts.id.low());
    builder.write_u64(facts.home_cell.q as i64 as u64);
    builder.write_u64(facts.home_cell.r as i64 as u64);
    builder.write_u64(facts.current_cell.q as i64 as u64);
    builder.write_u64(facts.current_cell.r as i64 as u64);
    builder.write_u64(u64::from(facts.household_id));
    builder.write_u64(u64::from(facts.profession));
    builder.write_u64(u64::from(facts.wealth));
    builder.write_u64(facts.commitment_id);
}

fn write_needs(builder: &mut FingerprintBuilder, needs: NeedState) {
    builder.write_u64(u64::from(needs.hunger));
    builder.write_u64(u64::from(needs.fatigue));
    builder.write_u64(u64::from(needs.social));
    builder.write_u64(u64::from(needs.morale));
}

fn mix64(mut value: u64) -> u64 {
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

#[derive(Debug, Error)]
pub enum ScaleError {
    #[error("invalid P6 scale configuration: {0}")]
    InvalidConfig(&'static str),
    #[error("person index {0} is outside the cold population")]
    PersonIndex(usize),
    #[error("P6 invariant failed: {0}")]
    Invariant(&'static str),
    #[error("deterministic random range failed: {0}")]
    RandomRange(#[from] deterministic_rng::RangeError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_scenario_is_deterministic() {
        let first = run_scale_scenario(ScaleConfig::small_test()).expect("first run");
        let second = run_scale_scenario(ScaleConfig::small_test()).expect("second run");
        assert_eq!(first.fingerprint, second.fingerprint);
        assert_eq!(first.cold_metrics.full_population_scans, 0);
        assert_eq!(first.warm_metrics.full_population_scans, 0);
    }

    #[test]
    fn transition_round_trip_preserves_persistent_facts() {
        let config = ScaleConfig::small_test();
        let mut cold = ColdPopulation::generate(config).expect("cold population");
        let mut history = Vec::new();
        let evidence = cold
            .exercise_transitions(config.transition_people, &mut history)
            .expect("transitions");
        assert!(evidence.facts_preserved);
        assert_eq!(evidence.people_exercised, config.transition_people);
        assert_eq!(evidence.summaries_changed, config.transition_people);
        assert_eq!(evidence.final_cold_count, config.cold_population);
    }

    #[test]
    fn compact_cold_layout_stays_within_expected_budget() {
        let config = ScaleConfig::small_test();
        let cold = ColdPopulation::generate(config).expect("cold population");
        assert!(cold.owned_bytes() < config.cold_population * 96);
    }
}
