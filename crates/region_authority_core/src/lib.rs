#![forbid(unsafe_code)]

use replay_core::{FingerprintBuilder, StateFingerprint};
use serde::{Deserialize, Serialize};
use simulation_scale_core::{PersonFacts, SimTier};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc, Barrier,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
};
use thiserror::Error;
use world_generation_core::HexCoord;
use world_ids::{ClientId, PersonId, RegionId, VehicleId};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WorkerId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TransferId(pub u128);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorityToken {
    pub region_id: RegionId,
    pub worker_id: WorkerId,
    pub epoch: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegionKind {
    Static { cells: BTreeSet<HexCoord> },
    Mobile {
        vehicle_id: VehicleId,
        current_cell: HexCoord,
        future_cells: Vec<HexCoord>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum EntityKey {
    Person(PersonId),
    Cargo(u64),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersonRecord {
    pub facts: PersonFacts,
    pub tier: SimTier,
    pub owner_region: RegionId,
    pub runtime_generation: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CargoRecord {
    pub cargo_id: u64,
    pub manifest_hash: u64,
    pub owner_region: RegionId,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegionEntity {
    Person(PersonRecord),
    Cargo(CargoRecord),
}

impl RegionEntity {
    pub const fn key(&self) -> EntityKey {
        match self {
            Self::Person(record) => EntityKey::Person(record.facts.id),
            Self::Cargo(record) => EntityKey::Cargo(record.cargo_id),
        }
    }

    pub const fn owner_region(&self) -> RegionId {
        match self {
            Self::Person(record) => record.owner_region,
            Self::Cargo(record) => record.owner_region,
        }
    }

    fn set_owner_region(&mut self, region_id: RegionId) {
        match self {
            Self::Person(record) => record.owner_region = region_id,
            Self::Cargo(record) => record.owner_region = region_id,
        }
    }

    fn set_current_cell(&mut self, cell: HexCoord) {
        if let Self::Person(record) = self {
            record.facts.current_cell = cell;
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegionShard {
    pub id: RegionId,
    pub kind: RegionKind,
    pub writer: WorkerId,
    pub epoch: u64,
    pub revision: u64,
    pub entities: BTreeMap<EntityKey, RegionEntity>,
}

impl RegionShard {
    pub fn token(&self) -> AuthorityToken {
        AuthorityToken {
            region_id: self.id,
            worker_id: self.writer,
            epoch: self.epoch,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransferStage {
    Prepared,
    DestinationAccepted,
    Committed,
    Acknowledged,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferRecord {
    pub id: TransferId,
    pub entity_key: EntityKey,
    pub source_region: RegionId,
    pub destination_region: RegionId,
    pub snapshot: RegionEntity,
    pub stage: TransferStage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransferDisposition {
    Applied,
    Duplicate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransferFault {
    DestinationCopyBeforeCommit,
    SourceCopyAfterCommit,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemotePersonSummary {
    pub person_id: PersonId,
    pub tier: SimTier,
    pub current_cell: HexCoord,
    pub next_due_minute: u64,
    pub headline_code: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterestRequest {
    pub client_id: ClientId,
    pub actor_region: RegionId,
    pub camera_cell: HexCoord,
    pub adjacent_visual_cells: BTreeSet<HexCoord>,
    pub junction_exit_cells: BTreeSet<HexCoord>,
    pub window_view_cells: BTreeSet<HexCoord>,
    pub vehicle_future_cells: BTreeSet<HexCoord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientInterest {
    pub client_id: ClientId,
    pub visual_cells: BTreeSet<HexCoord>,
    pub simulation_regions: BTreeSet<RegionId>,
    pub prefetch_cells: BTreeSet<HexCoord>,
    pub mobile_regions: BTreeSet<RegionId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientProjection {
    pub interest: ClientInterest,
    pub detailed_entities: BTreeSet<EntityKey>,
    pub remote_summaries: Vec<RemotePersonSummary>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HotspotCommand {
    pub arrival: u64,
    pub client_id: ClientId,
    pub operation_code: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HotspotMetrics {
    pub command_count: usize,
    pub unique_clients: usize,
    pub unique_writers: usize,
    pub first_revision: u64,
    pub final_revision: u64,
    pub revisions_contiguous: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegionWorkload {
    pub region_id: RegionId,
    pub seed: u64,
    pub steps: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegionTickResult {
    pub region_id: RegionId,
    pub accumulator: u64,
    pub revision_delta: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParallelComparison {
    pub sequential: Vec<RegionTickResult>,
    pub parallel: Vec<RegionTickResult>,
    pub sequential_fingerprint: StateFingerprint,
    pub parallel_fingerprint: StateFingerprint,
    pub max_parallel_workers: usize,
    pub identical: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RegionAuthorityWorld {
    regions: BTreeMap<RegionId, RegionShard>,
    cell_to_region: BTreeMap<HexCoord, RegionId>,
    transfers: BTreeMap<TransferId, TransferRecord>,
    remote_summaries: BTreeMap<PersonId, RemotePersonSummary>,
}

impl RegionAuthorityWorld {
    pub fn add_region(&mut self, shard: RegionShard) -> Result<(), RegionAuthorityError> {
        if self.regions.contains_key(&shard.id) {
            return Err(RegionAuthorityError::DuplicateRegion(shard.id));
        }
        if shard.epoch == 0 {
            return Err(RegionAuthorityError::InvalidEpoch);
        }
        if let RegionKind::Static { cells } = &shard.kind {
            for cell in cells {
                if let Some(existing) = self.cell_to_region.insert(*cell, shard.id) {
                    return Err(RegionAuthorityError::CellAlreadyOwned {
                        cell: *cell,
                        existing,
                    });
                }
            }
        }
        self.regions.insert(shard.id, shard);
        Ok(())
    }

    pub fn region(&self, region_id: RegionId) -> Result<&RegionShard, RegionAuthorityError> {
        self.regions
            .get(&region_id)
            .ok_or(RegionAuthorityError::UnknownRegion(region_id))
    }

    pub fn token(&self, region_id: RegionId) -> Result<AuthorityToken, RegionAuthorityError> {
        Ok(self.region(region_id)?.token())
    }

    pub fn static_region_for_cell(&self, cell: HexCoord) -> Option<RegionId> {
        self.cell_to_region.get(&cell).copied()
    }

    pub fn region_count(&self) -> usize {
        self.regions.len()
    }

    pub fn static_region_count(&self) -> usize {
        self.regions
            .values()
            .filter(|region| matches!(&region.kind, RegionKind::Static { .. }))
            .count()
    }

    pub fn mobile_region_count(&self) -> usize {
        self.regions
            .values()
            .filter(|region| matches!(&region.kind, RegionKind::Mobile { .. }))
            .count()
    }

    pub fn transfer_count(&self) -> usize {
        self.transfers.len()
    }

    pub fn remote_summary_count(&self) -> usize {
        self.remote_summaries.len()
    }

    pub fn insert_entity(
        &mut self,
        token: AuthorityToken,
        mut entity: RegionEntity,
    ) -> Result<(), RegionAuthorityError> {
        self.assert_token(token)?;
        let key = entity.key();
        if self.entity_locations(key).next().is_some() {
            return Err(RegionAuthorityError::DuplicateEntity(key));
        }
        entity.set_owner_region(token.region_id);
        let shard = self
            .regions
            .get_mut(&token.region_id)
            .ok_or(RegionAuthorityError::UnknownRegion(token.region_id))?;
        shard.entities.insert(key, entity);
        shard.revision += 1;
        Ok(())
    }

    pub fn entity_locations(&self, key: EntityKey) -> impl Iterator<Item = RegionId> + '_ {
        self.regions.iter().filter_map(move |(region_id, shard)| {
            shard.entities.contains_key(&key).then_some(*region_id)
        })
    }

    pub fn entity(&self, key: EntityKey) -> Option<&RegionEntity> {
        self.regions
            .values()
            .find_map(|shard| shard.entities.get(&key))
    }

    pub fn rebalance_region(
        &mut self,
        region_id: RegionId,
        new_worker: WorkerId,
    ) -> Result<(AuthorityToken, AuthorityToken), RegionAuthorityError> {
        let shard = self
            .regions
            .get_mut(&region_id)
            .ok_or(RegionAuthorityError::UnknownRegion(region_id))?;
        let previous = shard.token();
        shard.writer = new_worker;
        shard.epoch = shard
            .epoch
            .checked_add(1)
            .ok_or(RegionAuthorityError::InvalidEpoch)?;
        let current = shard.token();
        Ok((previous, current))
    }

    pub fn guarded_write(
        &mut self,
        token: AuthorityToken,
        operation_code: u16,
    ) -> Result<u64, RegionAuthorityError> {
        self.assert_token(token)?;
        let shard = self
            .regions
            .get_mut(&token.region_id)
            .ok_or(RegionAuthorityError::UnknownRegion(token.region_id))?;
        shard.revision = shard
            .revision
            .wrapping_add(u64::from(operation_code).max(1));
        Ok(shard.revision)
    }

    pub fn register_remote_summary(&mut self, summary: RemotePersonSummary) {
        self.remote_summaries.insert(summary.person_id, summary);
    }

    pub fn compute_interest(
        &self,
        request: InterestRequest,
    ) -> Result<ClientProjection, RegionAuthorityError> {
        let actor_region = self.region(request.actor_region)?;
        let mut visual_cells = request.adjacent_visual_cells.clone();
        visual_cells.insert(request.camera_cell);
        visual_cells.extend(request.window_view_cells.iter().copied());

        let mut prefetch_cells = request.junction_exit_cells.clone();
        prefetch_cells.extend(request.vehicle_future_cells.iter().copied());

        let mut simulation_regions = BTreeSet::new();
        simulation_regions.insert(request.actor_region);
        let mut mobile_regions = BTreeSet::new();
        if matches!(&actor_region.kind, RegionKind::Mobile { .. }) {
            mobile_regions.insert(request.actor_region);
        }

        let interest = ClientInterest {
            client_id: request.client_id,
            visual_cells,
            simulation_regions,
            prefetch_cells,
            mobile_regions,
        };

        let mut detailed_entities = BTreeSet::new();
        for region_id in &interest.simulation_regions {
            if let Some(shard) = self.regions.get(region_id) {
                detailed_entities.extend(shard.entities.keys().copied());
            }
        }

        let remote_summaries = self.remote_summaries.values().cloned().collect();
        Ok(ClientProjection {
            interest,
            detailed_entities,
            remote_summaries,
        })
    }

    pub fn prepare_transfer(
        &mut self,
        source_token: AuthorityToken,
        transfer_id: TransferId,
        key: EntityKey,
        destination_region: RegionId,
    ) -> Result<TransferDisposition, RegionAuthorityError> {
        self.assert_token(source_token)?;
        self.region(destination_region)?;
        if source_token.region_id == destination_region {
            return Err(RegionAuthorityError::SameRegionTransfer);
        }
        if let Some(existing) = self.transfers.get(&transfer_id) {
            if existing.entity_key == key
                && existing.source_region == source_token.region_id
                && existing.destination_region == destination_region
            {
                return Ok(TransferDisposition::Duplicate);
            }
            return Err(RegionAuthorityError::TransferIdConflict(transfer_id));
        }
        let snapshot = self
            .regions
            .get(&source_token.region_id)
            .and_then(|shard| shard.entities.get(&key))
            .cloned()
            .ok_or(RegionAuthorityError::UnknownEntity(key))?;
        self.transfers.insert(
            transfer_id,
            TransferRecord {
                id: transfer_id,
                entity_key: key,
                source_region: source_token.region_id,
                destination_region,
                snapshot,
                stage: TransferStage::Prepared,
            },
        );
        Ok(TransferDisposition::Applied)
    }

    pub fn accept_transfer(
        &mut self,
        destination_token: AuthorityToken,
        transfer_id: TransferId,
    ) -> Result<TransferDisposition, RegionAuthorityError> {
        self.assert_token(destination_token)?;
        let record = self
            .transfers
            .get_mut(&transfer_id)
            .ok_or(RegionAuthorityError::UnknownTransfer(transfer_id))?;
        if record.destination_region != destination_token.region_id {
            return Err(RegionAuthorityError::WrongTransferRegion);
        }
        match record.stage {
            TransferStage::Prepared => {
                record.stage = TransferStage::DestinationAccepted;
                Ok(TransferDisposition::Applied)
            }
            TransferStage::DestinationAccepted
            | TransferStage::Committed
            | TransferStage::Acknowledged => Ok(TransferDisposition::Duplicate),
        }
    }

    pub fn commit_transfer(
        &mut self,
        source_token: AuthorityToken,
        destination_token: AuthorityToken,
        transfer_id: TransferId,
    ) -> Result<TransferDisposition, RegionAuthorityError> {
        self.assert_token(source_token)?;
        self.assert_token(destination_token)?;
        let record = self
            .transfers
            .get(&transfer_id)
            .cloned()
            .ok_or(RegionAuthorityError::UnknownTransfer(transfer_id))?;
        if record.source_region != source_token.region_id
            || record.destination_region != destination_token.region_id
        {
            return Err(RegionAuthorityError::WrongTransferRegion);
        }
        match record.stage {
            TransferStage::Committed | TransferStage::Acknowledged => {
                return Ok(TransferDisposition::Duplicate);
            }
            TransferStage::Prepared => return Err(RegionAuthorityError::DestinationNotAccepted),
            TransferStage::DestinationAccepted => {}
        }

        let destination_cell = self.region_anchor_cell(record.destination_region)?;
        let mut entity = {
            let source = self
                .regions
                .get_mut(&record.source_region)
                .ok_or(RegionAuthorityError::UnknownRegion(record.source_region))?;
            let entity = source
                .entities
                .remove(&record.entity_key)
                .ok_or(RegionAuthorityError::UnknownEntity(record.entity_key))?;
            source.revision += 1;
            entity
        };
        entity.set_owner_region(record.destination_region);
        entity.set_current_cell(destination_cell);
        {
            let destination = self
                .regions
                .get_mut(&record.destination_region)
                .ok_or(RegionAuthorityError::UnknownRegion(record.destination_region))?;
            if destination.entities.insert(record.entity_key, entity).is_some() {
                return Err(RegionAuthorityError::DuplicateEntity(record.entity_key));
            }
            destination.revision += 1;
        }
        self.transfers
            .get_mut(&transfer_id)
            .expect("transfer was validated")
            .stage = TransferStage::Committed;
        Ok(TransferDisposition::Applied)
    }

    pub fn acknowledge_transfer(
        &mut self,
        transfer_id: TransferId,
    ) -> Result<TransferDisposition, RegionAuthorityError> {
        let record = self
            .transfers
            .get_mut(&transfer_id)
            .ok_or(RegionAuthorityError::UnknownTransfer(transfer_id))?;
        match record.stage {
            TransferStage::Committed => {
                record.stage = TransferStage::Acknowledged;
                Ok(TransferDisposition::Applied)
            }
            TransferStage::Acknowledged => Ok(TransferDisposition::Duplicate),
            _ => Err(RegionAuthorityError::TransferNotCommitted),
        }
    }

    pub fn inject_transfer_fault(
        &mut self,
        transfer_id: TransferId,
        fault: TransferFault,
    ) -> Result<(), RegionAuthorityError> {
        let record = self
            .transfers
            .get(&transfer_id)
            .cloned()
            .ok_or(RegionAuthorityError::UnknownTransfer(transfer_id))?;
        match fault {
            TransferFault::DestinationCopyBeforeCommit => {
                let destination = self
                    .regions
                    .get_mut(&record.destination_region)
                    .ok_or(RegionAuthorityError::UnknownRegion(record.destination_region))?;
                destination
                    .entities
                    .insert(record.entity_key, record.snapshot.clone());
            }
            TransferFault::SourceCopyAfterCommit => {
                let source = self
                    .regions
                    .get_mut(&record.source_region)
                    .ok_or(RegionAuthorityError::UnknownRegion(record.source_region))?;
                source
                    .entities
                    .insert(record.entity_key, record.snapshot.clone());
            }
        }
        Ok(())
    }

    pub fn recover_transfer(
        &mut self,
        transfer_id: TransferId,
    ) -> Result<RegionId, RegionAuthorityError> {
        let record = self
            .transfers
            .get(&transfer_id)
            .cloned()
            .ok_or(RegionAuthorityError::UnknownTransfer(transfer_id))?;
        match record.stage {
            TransferStage::Prepared | TransferStage::DestinationAccepted => {
                self.regions
                    .get_mut(&record.destination_region)
                    .expect("destination exists")
                    .entities
                    .remove(&record.entity_key);
                let source = self
                    .regions
                    .get_mut(&record.source_region)
                    .expect("source exists");
                source
                    .entities
                    .entry(record.entity_key)
                    .or_insert(record.snapshot.clone());
                Ok(record.source_region)
            }
            TransferStage::Committed | TransferStage::Acknowledged => {
                self.regions
                    .get_mut(&record.source_region)
                    .expect("source exists")
                    .entities
                    .remove(&record.entity_key);
                let mut destination_entity = record.snapshot.clone();
                destination_entity.set_owner_region(record.destination_region);
                let destination = self
                    .regions
                    .get_mut(&record.destination_region)
                    .expect("destination exists");
                destination
                    .entities
                    .entry(record.entity_key)
                    .or_insert(destination_entity);
                Ok(record.destination_region)
            }
        }
    }

    pub fn move_mobile_region(
        &mut self,
        token: AuthorityToken,
        current_cell: HexCoord,
        future_cells: Vec<HexCoord>,
    ) -> Result<(), RegionAuthorityError> {
        self.assert_token(token)?;
        let shard = self
            .regions
            .get_mut(&token.region_id)
            .ok_or(RegionAuthorityError::UnknownRegion(token.region_id))?;
        let RegionKind::Mobile {
            current_cell: stored_cell,
            future_cells: stored_future,
            ..
        } = &mut shard.kind
        else {
            return Err(RegionAuthorityError::NotMobileRegion(token.region_id));
        };
        *stored_cell = current_cell;
        *stored_future = future_cells;
        for entity in shard.entities.values_mut() {
            entity.set_current_cell(current_cell);
        }
        shard.revision += 1;
        Ok(())
    }

    pub fn process_hotspot(
        &mut self,
        token: AuthorityToken,
        mut commands: Vec<HotspotCommand>,
    ) -> Result<HotspotMetrics, RegionAuthorityError> {
        self.assert_token(token)?;
        commands.sort_by_key(|command| (command.arrival, command.client_id));
        let first_revision = self.region(token.region_id)?.revision;
        let mut clients = BTreeSet::new();
        let mut writers = BTreeSet::new();
        let mut expected = first_revision;
        let mut contiguous = true;
        for command in &commands {
            clients.insert(command.client_id);
            writers.insert(token.worker_id);
            let revision = self.guarded_write(token, 1)?;
            expected += 1;
            contiguous &= revision == expected;
        }
        Ok(HotspotMetrics {
            command_count: commands.len(),
            unique_clients: clients.len(),
            unique_writers: writers.len(),
            first_revision,
            final_revision: self.region(token.region_id)?.revision,
            revisions_contiguous: contiguous,
        })
    }

    pub fn transfer(&self, id: TransferId) -> Option<&TransferRecord> {
        self.transfers.get(&id)
    }

    pub fn semantic_fingerprint(&self) -> StateFingerprint {
        let mut builder = FingerprintBuilder::default();
        builder.write_u64(self.regions.len() as u64);
        for (region_id, shard) in &self.regions {
            write_u128(&mut builder, region_id.as_u128());
            builder.write_u64(shard.writer.0);
            builder.write_u64(shard.epoch);
            builder.write_u64(shard.revision);
            match &shard.kind {
                RegionKind::Static { cells } => {
                    builder.write_u64(0);
                    for cell in cells {
                        write_cell(&mut builder, *cell);
                    }
                }
                RegionKind::Mobile {
                    vehicle_id,
                    current_cell,
                    future_cells,
                } => {
                    builder.write_u64(1);
                    write_u128(&mut builder, vehicle_id.as_u128());
                    write_cell(&mut builder, *current_cell);
                    for cell in future_cells {
                        write_cell(&mut builder, *cell);
                    }
                }
            }
            for (key, entity) in &shard.entities {
                write_entity_key(&mut builder, *key);
                write_entity(&mut builder, entity);
            }
        }
        for (id, transfer) in &self.transfers {
            write_u128(&mut builder, id.0);
            write_entity_key(&mut builder, transfer.entity_key);
            write_u128(&mut builder, transfer.source_region.as_u128());
            write_u128(&mut builder, transfer.destination_region.as_u128());
            builder.write_u64(match transfer.stage {
                TransferStage::Prepared => 0,
                TransferStage::DestinationAccepted => 1,
                TransferStage::Committed => 2,
                TransferStage::Acknowledged => 3,
            });
        }
        for summary in self.remote_summaries.values() {
            write_u128(&mut builder, summary.person_id.as_u128());
            write_cell(&mut builder, summary.current_cell);
            builder.write_u64(summary.next_due_minute);
            builder.write_u64(u64::from(summary.headline_code));
            builder.write_u64(tier_code(summary.tier));
        }
        builder.finish()
    }

    fn region_anchor_cell(&self, region_id: RegionId) -> Result<HexCoord, RegionAuthorityError> {
        let region = self.region(region_id)?;
        match &region.kind {
            RegionKind::Static { cells } => cells
                .iter()
                .next()
                .copied()
                .ok_or(RegionAuthorityError::EmptyStaticRegion(region_id)),
            RegionKind::Mobile { current_cell, .. } => Ok(*current_cell),
        }
    }

    fn assert_token(&self, token: AuthorityToken) -> Result<(), RegionAuthorityError> {
        let shard = self.region(token.region_id)?;
        if shard.writer != token.worker_id || shard.epoch != token.epoch {
            return Err(RegionAuthorityError::StaleWriter {
                region_id: token.region_id,
                expected_worker: shard.writer,
                expected_epoch: shard.epoch,
                actual_worker: token.worker_id,
                actual_epoch: token.epoch,
            });
        }
        Ok(())
    }
}

pub fn compare_sequential_and_parallel(workloads: &[RegionWorkload]) -> ParallelComparison {
    let mut sequential = workloads
        .iter()
        .copied()
        .map(run_workload)
        .collect::<Vec<_>>();
    sequential.sort_by_key(|result| result.region_id);

    let count = workloads.len().max(1);
    let start_barrier = Arc::new(Barrier::new(count));
    let overlap_barrier = Arc::new(Barrier::new(count));
    let active = Arc::new(AtomicUsize::new(0));
    let max_active = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::with_capacity(workloads.len());
    for workload in workloads.iter().copied() {
        let start = Arc::clone(&start_barrier);
        let overlap = Arc::clone(&overlap_barrier);
        let active_count = Arc::clone(&active);
        let max_count = Arc::clone(&max_active);
        handles.push(thread::spawn(move || {
            start.wait();
            let current = active_count.fetch_add(1, Ordering::SeqCst) + 1;
            max_count.fetch_max(current, Ordering::SeqCst);
            overlap.wait();
            let result = run_workload(workload);
            active_count.fetch_sub(1, Ordering::SeqCst);
            result
        }));
    }
    let mut parallel = handles
        .into_iter()
        .map(|handle| handle.join().expect("region worker must not panic"))
        .collect::<Vec<_>>();
    parallel.sort_by_key(|result| result.region_id);
    let sequential_fingerprint = fingerprint_tick_results(&sequential);
    let parallel_fingerprint = fingerprint_tick_results(&parallel);
    ParallelComparison {
        identical: sequential == parallel && sequential_fingerprint == parallel_fingerprint,
        sequential,
        parallel,
        sequential_fingerprint,
        parallel_fingerprint,
        max_parallel_workers: max_active.load(Ordering::SeqCst),
    }
}

fn run_workload(workload: RegionWorkload) -> RegionTickResult {
    let mut accumulator = workload.seed ^ workload.region_id.low();
    for step in 0..workload.steps {
        accumulator = mix64(accumulator ^ u64::from(step).wrapping_mul(0x9e37_79b9));
    }
    RegionTickResult {
        region_id: workload.region_id,
        accumulator,
        revision_delta: u64::from(workload.steps),
    }
}

fn fingerprint_tick_results(results: &[RegionTickResult]) -> StateFingerprint {
    let mut builder = FingerprintBuilder::default();
    for result in results {
        write_u128(&mut builder, result.region_id.as_u128());
        builder.write_u64(result.accumulator);
        builder.write_u64(result.revision_delta);
    }
    builder.finish()
}

fn write_entity(builder: &mut FingerprintBuilder, entity: &RegionEntity) {
    match entity {
        RegionEntity::Person(record) => {
            builder.write_u64(0);
            write_u128(builder, record.facts.id.as_u128());
            write_cell(builder, record.facts.home_cell);
            write_cell(builder, record.facts.current_cell);
            builder.write_u64(u64::from(record.facts.household_id));
            builder.write_u64(u64::from(record.facts.profession));
            builder.write_u64(u64::from(record.facts.wealth));
            builder.write_u64(record.facts.commitment_id);
            builder.write_u64(tier_code(record.tier));
            write_u128(builder, record.owner_region.as_u128());
            builder.write_u64(record.runtime_generation);
        }
        RegionEntity::Cargo(record) => {
            builder.write_u64(1);
            builder.write_u64(record.cargo_id);
            builder.write_u64(record.manifest_hash);
            write_u128(builder, record.owner_region.as_u128());
        }
    }
}

fn write_entity_key(builder: &mut FingerprintBuilder, key: EntityKey) {
    match key {
        EntityKey::Person(person_id) => {
            builder.write_u64(0);
            write_u128(builder, person_id.as_u128());
        }
        EntityKey::Cargo(cargo_id) => {
            builder.write_u64(1);
            builder.write_u64(cargo_id);
        }
    }
}

fn write_cell(builder: &mut FingerprintBuilder, cell: HexCoord) {
    builder.write_u64(i64::from(cell.q) as u64);
    builder.write_u64(i64::from(cell.r) as u64);
}

fn write_u128(builder: &mut FingerprintBuilder, value: u128) {
    builder.write_u64(value as u64);
    builder.write_u64((value >> 64) as u64);
}

const fn tier_code(tier: SimTier) -> u64 {
    match tier {
        SimTier::Cold => 0,
        SimTier::Warm => 1,
        SimTier::Hot => 2,
    }
}

fn mix64(mut value: u64) -> u64 {
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum RegionAuthorityError {
    #[error("unknown region {0}")]
    UnknownRegion(RegionId),
    #[error("duplicate region {0}")]
    DuplicateRegion(RegionId),
    #[error("region epoch must be positive")]
    InvalidEpoch,
    #[error("cell {cell:?} is already owned by region {existing}")]
    CellAlreadyOwned { cell: HexCoord, existing: RegionId },
    #[error("stale writer for region {region_id}: expected worker {expected_worker:?} epoch {expected_epoch}, got worker {actual_worker:?} epoch {actual_epoch}")]
    StaleWriter {
        region_id: RegionId,
        expected_worker: WorkerId,
        expected_epoch: u64,
        actual_worker: WorkerId,
        actual_epoch: u64,
    },
    #[error("duplicate entity {0:?}")]
    DuplicateEntity(EntityKey),
    #[error("unknown entity {0:?}")]
    UnknownEntity(EntityKey),
    #[error("transfer source and destination cannot be the same region")]
    SameRegionTransfer,
    #[error("transfer id conflict {0:?}")]
    TransferIdConflict(TransferId),
    #[error("unknown transfer {0:?}")]
    UnknownTransfer(TransferId),
    #[error("transfer token refers to the wrong source or destination region")]
    WrongTransferRegion,
    #[error("destination has not accepted the transfer")]
    DestinationNotAccepted,
    #[error("transfer has not committed")]
    TransferNotCommitted,
    #[error("region {0} is not a mobile region")]
    NotMobileRegion(RegionId),
    #[error("static region {0} has no cells")]
    EmptyStaticRegion(RegionId),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn region(id: u128, worker: u64, cell: HexCoord) -> RegionShard {
        RegionShard {
            id: RegionId::from_u128(id),
            kind: RegionKind::Static {
                cells: BTreeSet::from([cell]),
            },
            writer: WorkerId(worker),
            epoch: 1,
            revision: 0,
            entities: BTreeMap::new(),
        }
    }

    fn person(id: u128, region_id: RegionId, cell: HexCoord) -> RegionEntity {
        RegionEntity::Person(PersonRecord {
            facts: PersonFacts {
                id: PersonId::from_u128(id),
                home_cell: cell,
                current_cell: cell,
                household_id: 7,
                profession: 3,
                wealth: 99,
                commitment_id: 123,
            },
            tier: SimTier::Hot,
            owner_region: region_id,
            runtime_generation: 1,
        })
    }

    #[test]
    fn stale_writer_is_rejected_after_rebalance() {
        let mut world = RegionAuthorityWorld::default();
        let region_id = RegionId::from_u128(1);
        world
            .add_region(region(1, 10, HexCoord::ZERO))
            .expect("region");
        let old = world.token(region_id).expect("token");
        let (_, new) = world
            .rebalance_region(region_id, WorkerId(11))
            .expect("rebalance");
        assert!(matches!(
            world.guarded_write(old, 1),
            Err(RegionAuthorityError::StaleWriter { .. })
        ));
        assert!(world.guarded_write(new, 1).is_ok());
    }

    #[test]
    fn transfer_recovery_keeps_exactly_one_copy() {
        let mut world = RegionAuthorityWorld::default();
        let source = RegionId::from_u128(1);
        let destination = RegionId::from_u128(2);
        world
            .add_region(region(1, 10, HexCoord::ZERO))
            .expect("source");
        world
            .add_region(region(2, 20, HexCoord::new(1, 0)))
            .expect("destination");
        let source_token = world.token(source).expect("source token");
        let destination_token = world.token(destination).expect("destination token");
        let key = EntityKey::Person(PersonId::from_u128(9));
        world
            .insert_entity(source_token, person(9, source, HexCoord::ZERO))
            .expect("insert");
        let transfer_id = TransferId(1);
        world
            .prepare_transfer(source_token, transfer_id, key, destination)
            .expect("prepare");
        world
            .accept_transfer(destination_token, transfer_id)
            .expect("accept");
        world
            .inject_transfer_fault(transfer_id, TransferFault::DestinationCopyBeforeCommit)
            .expect("fault");
        assert_eq!(world.entity_locations(key).count(), 2);
        assert_eq!(world.recover_transfer(transfer_id).expect("recover"), source);
        assert_eq!(world.entity_locations(key).collect::<Vec<_>>(), vec![source]);

        world
            .commit_transfer(source_token, destination_token, transfer_id)
            .expect("commit");
        world
            .inject_transfer_fault(transfer_id, TransferFault::SourceCopyAfterCommit)
            .expect("fault");
        assert_eq!(world.entity_locations(key).count(), 2);
        assert_eq!(
            world.recover_transfer(transfer_id).expect("recover"),
            destination
        );
        assert_eq!(
            world.entity_locations(key).collect::<Vec<_>>(),
            vec![destination]
        );
    }

    #[test]
    fn parallel_regions_match_sequential_regions() {
        let workloads = [
            RegionWorkload {
                region_id: RegionId::from_u128(1),
                seed: 7,
                steps: 10_000,
            },
            RegionWorkload {
                region_id: RegionId::from_u128(2),
                seed: 9,
                steps: 12_000,
            },
        ];
        let comparison = compare_sequential_and_parallel(&workloads);
        assert!(comparison.identical);
        assert_eq!(comparison.max_parallel_workers, 2);
    }
}
