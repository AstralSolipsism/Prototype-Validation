#![forbid(unsafe_code)]

use region_authority_core::{
    AuthorityToken, CargoRecord, ClientProjection, EntityKey, HotspotCommand, HotspotMetrics,
    InterestRequest, ParallelComparison, PersonRecord, RegionAuthorityError, RegionAuthorityWorld,
    RegionEntity, RegionKind, RegionShard, RegionWorkload, RemotePersonSummary, TransferDisposition,
    TransferFault, TransferId, TransferStage, WorkerId, compare_sequential_and_parallel,
};
use replay_core::StateFingerprint;
use serde::{Deserialize, Serialize};
use simulation_scale_core::{PersonFacts, SimTier};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;
use world_generation_core::HexCoord;
use world_ids::{ClientId, PersonId, RegionId, VehicleId};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationStep {
    pub index: usize,
    pub title: String,
    pub detail: String,
    pub passed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnerEvidence {
    pub region_id: RegionId,
    pub worker_id: WorkerId,
    pub epoch: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferEvidence {
    pub static_transfer_id: TransferId,
    pub boarding_transfer_id: TransferId,
    pub cargo_transfer_id: TransferId,
    pub disembark_transfer_id: TransferId,
    pub duplicate_operations: usize,
    pub fault_recoveries: usize,
    pub unique_entity_locations: bool,
    pub persistent_facts_preserved: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MobileRegionEvidence {
    pub region_id: RegionId,
    pub vehicle_id: VehicleId,
    pub start_cell: HexCoord,
    pub end_cell: HexCoord,
    pub transfer_count_before_move: usize,
    pub transfer_count_after_move: usize,
    pub passenger_ids_before: BTreeSet<PersonId>,
    pub passenger_ids_after: BTreeSet<PersonId>,
    pub cargo_stayed_mobile: bool,
    pub internal_authority_preserved: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScenarioEvidence {
    pub client_count: usize,
    pub static_region_count: usize,
    pub mobile_region_count: usize,
    pub projections: BTreeMap<ClientId, ClientProjection>,
    pub owners_before_rebalance: Vec<OwnerEvidence>,
    pub owners_after_rebalance: Vec<OwnerEvidence>,
    pub stale_writer_rejected: bool,
    pub transfer: TransferEvidence,
    pub mobile: MobileRegionEvidence,
    pub remote_summary_count: usize,
    pub remote_person_is_not_detailed: bool,
    pub parallel: ParallelComparison,
    pub hotspot: HotspotMetrics,
    pub final_fingerprint: StateFingerprint,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct P7ValidationReport {
    pub schema_version: u32,
    pub checks: Vec<ValidationStep>,
    pub first_run: ScenarioEvidence,
    pub repeat_run: ScenarioEvidence,
    pub fingerprints_match: bool,
    pub all_passed: bool,
}

#[derive(Clone, Copy)]
struct ScenarioIds {
    west_region: RegionId,
    port_region: RegionId,
    east_region: RegionId,
    ship_region: RegionId,
    ship_vehicle: VehicleId,
    player_a: PersonId,
    player_b: PersonId,
    player_c: PersonId,
    player_d: PersonId,
    remote_person: PersonId,
    client_a: ClientId,
    client_b: ClientId,
    client_c: ClientId,
    client_d: ClientId,
}

impl ScenarioIds {
    fn new() -> Self {
        Self {
            west_region: RegionId::from_u128(0x7000_0001),
            port_region: RegionId::from_u128(0x7000_0002),
            east_region: RegionId::from_u128(0x7000_0003),
            ship_region: RegionId::from_u128(0x7000_0004),
            ship_vehicle: VehicleId::from_u128(0x7000_1001),
            player_a: PersonId::from_u128(0x7000_a001),
            player_b: PersonId::from_u128(0x7000_b002),
            player_c: PersonId::from_u128(0x7000_c003),
            player_d: PersonId::from_u128(0x7000_d004),
            remote_person: PersonId::from_u128(0x7000_f999),
            client_a: ClientId::from_u128(0x7000_ca01),
            client_b: ClientId::from_u128(0x7000_cb02),
            client_c: ClientId::from_u128(0x7000_cc03),
            client_d: ClientId::from_u128(0x7000_cd04),
        }
    }
}

pub fn run_validation(
    mut observe: impl FnMut(&ValidationStep),
) -> Result<P7ValidationReport, P7HarnessError> {
    let first_run = run_scenario()?;
    let repeat_run = run_scenario()?;
    let fingerprints_match = first_run.final_fingerprint == repeat_run.final_fingerprint;
    let mut checks = Vec::new();

    record(
        &mut checks,
        &mut observe,
        "four clients and four authority regions",
        format!(
            "clients={}, static_regions={}, mobile_regions={}",
            first_run.client_count,
            first_run.static_region_count,
            first_run.mobile_region_count
        ),
        first_run.client_count == 4
            && first_run.static_region_count == 3
            && first_run.mobile_region_count == 1,
    );

    let ids = ScenarioIds::new();
    let projection_a = &first_run.projections[&ids.client_a];
    let projection_b = &first_run.projections[&ids.client_b];
    let projection_c = &first_run.projections[&ids.client_c];
    let projection_d = &first_run.projections[&ids.client_d];
    record(
        &mut checks,
        &mut observe,
        "visual, simulation and prefetch interests are independent",
        "each client has one authoritative simulation region while visual and prefetch cells can extend beyond it"
            .into(),
        first_run.projections.values().all(|projection| {
            projection.interest.simulation_regions.len() == 1
                && projection.interest.visual_cells != projection.interest.prefetch_cells
        }),
    );
    record(
        &mut checks,
        &mut observe,
        "junction exits are prefetched without high-detail simulation",
        format!(
            "port client prefetches {} exits and simulates {} region",
            projection_b.interest.prefetch_cells.len(),
            projection_b.interest.simulation_regions.len()
        ),
        projection_b.interest.prefetch_cells
            == BTreeSet::from([
                HexCoord::new(-1, 0),
                HexCoord::new(1, 0),
                HexCoord::new(0, -1),
            ])
            && projection_b.interest.simulation_regions == BTreeSet::from([ids.port_region])
            && !projection_b
                .detailed_entities
                .contains(&EntityKey::Person(ids.player_d)),
    );
    record(
        &mut checks,
        &mut observe,
        "ship window expands visual interest without changing authority",
        "client C sees the port through the ship window but detailed authority remains MobileRegion"
            .into(),
        projection_c
            .interest
            .visual_cells
            .contains(&HexCoord::ZERO)
            && projection_c.interest.simulation_regions == BTreeSet::from([ids.ship_region])
            && projection_c.interest.mobile_regions == BTreeSet::from([ids.ship_region]),
    );
    record(
        &mut checks,
        &mut observe,
        "vehicle trajectory drives forward prefetch",
        format!(
            "ship client prefetch cells={:?}",
            projection_c.interest.prefetch_cells
        ),
        projection_c
            .interest
            .prefetch_cells
            .contains(&HexCoord::new(1, 0))
            && projection_c
                .interest
                .prefetch_cells
                .contains(&HexCoord::new(2, 0)),
    );
    record(
        &mut checks,
        &mut observe,
        "remote people remain summaries instead of disappearing",
        format!(
            "remote summaries={}, remote detailed leak={}",
            first_run.remote_summary_count, !first_run.remote_person_is_not_detailed
        ),
        first_run.remote_summary_count == 1
            && first_run.remote_person_is_not_detailed
            && [projection_a, projection_b, projection_c, projection_d]
                .into_iter()
                .all(|projection| projection.remote_summaries.len() == 1),
    );
    record(
        &mut checks,
        &mut observe,
        "each region has one writer and epoch",
        format!(
            "owners before={}, owners after={}",
            first_run.owners_before_rebalance.len(),
            first_run.owners_after_rebalance.len()
        ),
        unique_regions_and_tokens(&first_run.owners_before_rebalance)
            && unique_regions_and_tokens(&first_run.owners_after_rebalance),
    );
    record(
        &mut checks,
        &mut observe,
        "stale writer is rejected after region rebalance",
        "east region epoch advanced and the previous writer token could not mutate it".into(),
        first_run.stale_writer_rejected,
    );
    record(
        &mut checks,
        &mut observe,
        "cross-region transfer is idempotent and recoverable",
        format!(
            "duplicates={}, fault_recoveries={}, unique_locations={}",
            first_run.transfer.duplicate_operations,
            first_run.transfer.fault_recoveries,
            first_run.transfer.unique_entity_locations
        ),
        first_run.transfer.duplicate_operations >= 4
            && first_run.transfer.fault_recoveries == 2
            && first_run.transfer.unique_entity_locations
            && first_run.transfer.persistent_facts_preserved,
    );
    record(
        &mut checks,
        &mut observe,
        "boarding and cargo transfer enter the mobile authority domain",
        "player B and cargo move from the port shard to the ship shard without duplicate ownership"
            .into(),
        first_run.transfer.unique_entity_locations
            && first_run.mobile.passenger_ids_before.contains(&ids.player_b)
            && first_run.mobile.cargo_stayed_mobile,
    );
    record(
        &mut checks,
        &mut observe,
        "mobile region crosses static cells without migrating its interior",
        format!(
            "ship {:?}->{:?}, transfers {}->{}",
            first_run.mobile.start_cell,
            first_run.mobile.end_cell,
            first_run.mobile.transfer_count_before_move,
            first_run.mobile.transfer_count_after_move
        ),
        first_run.mobile.internal_authority_preserved
            && first_run.mobile.transfer_count_before_move
                == first_run.mobile.transfer_count_after_move,
    );
    record(
        &mut checks,
        &mut observe,
        "disembark performs explicit MobileRegion to StaticRegion transfer",
        "player B ends in the east static region while player C and cargo remain on the ship"
            .into(),
        !first_run.mobile.passenger_ids_after.contains(&ids.player_b)
            && first_run.mobile.passenger_ids_after.contains(&ids.player_c)
            && first_run.mobile.cargo_stayed_mobile,
    );
    record(
        &mut checks,
        &mut observe,
        "independent regions execute in parallel with deterministic output",
        format!(
            "max_parallel_workers={}, identical={}",
            first_run.parallel.max_parallel_workers, first_run.parallel.identical
        ),
        first_run.parallel.identical && first_run.parallel.max_parallel_workers >= 2,
    );
    record(
        &mut checks,
        &mut observe,
        "hotspot remains single-writer and revision-contiguous",
        format!(
            "commands={}, clients={}, writers={}, revisions={}..{}",
            first_run.hotspot.command_count,
            first_run.hotspot.unique_clients,
            first_run.hotspot.unique_writers,
            first_run.hotspot.first_revision,
            first_run.hotspot.final_revision
        ),
        first_run.hotspot.command_count == 256
            && first_run.hotspot.unique_clients == 4
            && first_run.hotspot.unique_writers == 1
            && first_run.hotspot.revisions_contiguous,
    );
    record(
        &mut checks,
        &mut observe,
        "independent complete runs are deterministic",
        format!(
            "first={:?}, repeat={:?}",
            first_run.final_fingerprint, repeat_run.final_fingerprint
        ),
        fingerprints_match && first_run == repeat_run,
    );

    let all_passed = checks.iter().all(|step| step.passed);
    Ok(P7ValidationReport {
        schema_version: 1,
        checks,
        first_run,
        repeat_run,
        fingerprints_match,
        all_passed,
    })
}

fn run_scenario() -> Result<ScenarioEvidence, P7HarnessError> {
    let ids = ScenarioIds::new();
    let west_cell = HexCoord::new(-1, 0);
    let port_cell = HexCoord::ZERO;
    let east_cell = HexCoord::new(1, 0);
    let north_cell = HexCoord::new(0, -1);
    let remote_cell = HexCoord::new(4, -2);
    let mut world = RegionAuthorityWorld::default();
    world.add_region(static_region(ids.west_region, WorkerId(1), west_cell))?;
    world.add_region(static_region(
        ids.port_region,
        WorkerId(2),
        port_cell,
    ))?;
    world.add_region(static_region(ids.east_region, WorkerId(3), east_cell))?;
    world.add_region(RegionShard {
        id: ids.ship_region,
        kind: RegionKind::Mobile {
            vehicle_id: ids.ship_vehicle,
            current_cell: port_cell,
            future_cells: vec![east_cell, HexCoord::new(2, 0)],
        },
        writer: WorkerId(4),
        epoch: 1,
        revision: 0,
        entities: BTreeMap::new(),
    })?;

    let west_token = world.token(ids.west_region)?;
    let port_token = world.token(ids.port_region)?;
    let east_token_initial = world.token(ids.east_region)?;
    let ship_token = world.token(ids.ship_region)?;
    world.insert_entity(
        west_token,
        person(ids.player_a, ids.west_region, west_cell, SimTier::Hot, 11),
    )?;
    world.insert_entity(
        port_token,
        person(ids.player_b, ids.port_region, port_cell, SimTier::Hot, 22),
    )?;
    world.insert_entity(
        ship_token,
        person(ids.player_c, ids.ship_region, port_cell, SimTier::Hot, 33),
    )?;
    world.insert_entity(
        east_token_initial,
        person(ids.player_d, ids.east_region, east_cell, SimTier::Warm, 44),
    )?;
    world.insert_entity(
        port_token,
        RegionEntity::Cargo(CargoRecord {
            cargo_id: 0x7000_7001,
            manifest_hash: 0x51a7_5eed,
            owner_region: ids.port_region,
        }),
    )?;
    world.register_remote_summary(RemotePersonSummary {
        person_id: ids.remote_person,
        tier: SimTier::Cold,
        current_cell: remote_cell,
        next_due_minute: 86_400,
        headline_code: 7,
    });

    let projections = BTreeMap::from([
        (
            ids.client_a,
            world.compute_interest(InterestRequest {
                client_id: ids.client_a,
                actor_region: ids.west_region,
                camera_cell: west_cell,
                adjacent_visual_cells: BTreeSet::from([port_cell]),
                junction_exit_cells: BTreeSet::from([port_cell, north_cell]),
                window_view_cells: BTreeSet::new(),
                vehicle_future_cells: BTreeSet::new(),
            })?,
        ),
        (
            ids.client_b,
            world.compute_interest(InterestRequest {
                client_id: ids.client_b,
                actor_region: ids.port_region,
                camera_cell: port_cell,
                adjacent_visual_cells: BTreeSet::from([west_cell, east_cell]),
                junction_exit_cells: BTreeSet::from([west_cell, east_cell, north_cell]),
                window_view_cells: BTreeSet::new(),
                vehicle_future_cells: BTreeSet::new(),
            })?,
        ),
        (
            ids.client_c,
            world.compute_interest(InterestRequest {
                client_id: ids.client_c,
                actor_region: ids.ship_region,
                camera_cell: port_cell,
                adjacent_visual_cells: BTreeSet::new(),
                junction_exit_cells: BTreeSet::new(),
                window_view_cells: BTreeSet::from([port_cell]),
                vehicle_future_cells: BTreeSet::from([east_cell, HexCoord::new(2, 0)]),
            })?,
        ),
        (
            ids.client_d,
            world.compute_interest(InterestRequest {
                client_id: ids.client_d,
                actor_region: ids.east_region,
                camera_cell: east_cell,
                adjacent_visual_cells: BTreeSet::from([port_cell]),
                junction_exit_cells: BTreeSet::new(),
                window_view_cells: BTreeSet::new(),
                vehicle_future_cells: BTreeSet::new(),
            })?,
        ),
    ]);

    let owners_before_rebalance = owner_evidence(&world, &ids)?;
    let (stale_east_token, east_token) =
        world.rebalance_region(ids.east_region, WorkerId(5))?;
    let stale_writer_rejected = matches!(
        world.guarded_write(stale_east_token, 1),
        Err(RegionAuthorityError::StaleWriter { .. })
    );
    world.guarded_write(east_token, 1)?;
    let owners_after_rebalance = owner_evidence(&world, &ids)?;

    let original_a = world
        .entity(EntityKey::Person(ids.player_a))
        .cloned()
        .ok_or(P7HarnessError::MissingEntity(EntityKey::Person(ids.player_a)))?;
    let static_transfer_id = TransferId(0x7000_0000_0000_0001);
    let mut duplicate_operations = 0_usize;
    let mut fault_recoveries = 0_usize;
    world.prepare_transfer(
        west_token,
        static_transfer_id,
        EntityKey::Person(ids.player_a),
        ids.port_region,
    )?;
    if world.prepare_transfer(
        west_token,
        static_transfer_id,
        EntityKey::Person(ids.player_a),
        ids.port_region,
    )? == TransferDisposition::Duplicate
    {
        duplicate_operations += 1;
    }
    world.accept_transfer(port_token, static_transfer_id)?;
    if world.accept_transfer(port_token, static_transfer_id)? == TransferDisposition::Duplicate {
        duplicate_operations += 1;
    }
    world.inject_transfer_fault(
        static_transfer_id,
        TransferFault::DestinationCopyBeforeCommit,
    )?;
    world.recover_transfer(static_transfer_id)?;
    fault_recoveries += 1;
    world.commit_transfer(west_token, port_token, static_transfer_id)?;
    world.inject_transfer_fault(static_transfer_id, TransferFault::SourceCopyAfterCommit)?;
    world.recover_transfer(static_transfer_id)?;
    fault_recoveries += 1;
    world.acknowledge_transfer(static_transfer_id)?;
    if world.acknowledge_transfer(static_transfer_id)? == TransferDisposition::Duplicate {
        duplicate_operations += 1;
    }

    let boarding_transfer_id = TransferId(0x7000_0000_0000_0002);
    transfer_entity(
        &mut world,
        port_token,
        ship_token,
        boarding_transfer_id,
        EntityKey::Person(ids.player_b),
    )?;
    let cargo_transfer_id = TransferId(0x7000_0000_0000_0003);
    transfer_entity(
        &mut world,
        port_token,
        ship_token,
        cargo_transfer_id,
        EntityKey::Cargo(0x7000_7001),
    )?;
    if world.commit_transfer(port_token, ship_token, cargo_transfer_id)?
        == TransferDisposition::Duplicate
    {
        duplicate_operations += 1;
    }

    let passenger_ids_before = people_in_region(&world, ids.ship_region)?;
    let transfer_count_before_move = world.transfer_count();
    world.move_mobile_region(
        ship_token,
        east_cell,
        vec![HexCoord::new(2, 0), HexCoord::new(3, -1)],
    )?;
    let transfer_count_after_move = world.transfer_count();

    let disembark_transfer_id = TransferId(0x7000_0000_0000_0004);
    transfer_entity(
        &mut world,
        ship_token,
        east_token,
        disembark_transfer_id,
        EntityKey::Person(ids.player_b),
    )?;
    let passenger_ids_after = people_in_region(&world, ids.ship_region)?;
    let cargo_stayed_mobile = world
        .entity_locations(EntityKey::Cargo(0x7000_7001))
        .collect::<Vec<_>>()
        == vec![ids.ship_region];
    let internal_authority_preserved = world
        .entity_locations(EntityKey::Person(ids.player_c))
        .collect::<Vec<_>>()
        == vec![ids.ship_region]
        && world
            .entity(EntityKey::Person(ids.player_c))
            .is_some_and(|entity| entity.owner_region() == ids.ship_region);

    let keys = [
        EntityKey::Person(ids.player_a),
        EntityKey::Person(ids.player_b),
        EntityKey::Person(ids.player_c),
        EntityKey::Person(ids.player_d),
        EntityKey::Cargo(0x7000_7001),
    ];
    let unique_entity_locations = keys
        .into_iter()
        .all(|key| world.entity_locations(key).count() == 1);
    let after_a = world
        .entity(EntityKey::Person(ids.player_a))
        .cloned()
        .ok_or(P7HarnessError::MissingEntity(EntityKey::Person(ids.player_a)))?;
    let persistent_facts_preserved = persistent_signature(&original_a)
        == persistent_signature(&after_a)
        && world.transfer(static_transfer_id).is_some_and(|record| {
            record.stage == TransferStage::Acknowledged
                && record.destination_region == ids.port_region
        });

    let remote_person_is_not_detailed = projections.values().all(|projection| {
        !projection
            .detailed_entities
            .contains(&EntityKey::Person(ids.remote_person))
    });

    let parallel = compare_sequential_and_parallel(&[
        RegionWorkload {
            region_id: ids.west_region,
            seed: 0x7701,
            steps: 25_000,
        },
        RegionWorkload {
            region_id: ids.east_region,
            seed: 0x7702,
            steps: 28_000,
        },
    ]);

    let hotspot_commands = [ids.client_a, ids.client_b, ids.client_c, ids.client_d]
        .into_iter()
        .enumerate()
        .flat_map(|(client_index, client_id)| {
            (0_u64..64).map(move |sequence| HotspotCommand {
                arrival: sequence * 4 + client_index as u64,
                client_id,
                operation_code: 1,
            })
        })
        .collect::<Vec<_>>();
    let hotspot = world.process_hotspot(port_token, hotspot_commands)?;

    let final_fingerprint = world.semantic_fingerprint();
    Ok(ScenarioEvidence {
        client_count: projections.len(),
        static_region_count: world.static_region_count(),
        mobile_region_count: world.mobile_region_count(),
        projections,
        owners_before_rebalance,
        owners_after_rebalance,
        stale_writer_rejected,
        transfer: TransferEvidence {
            static_transfer_id,
            boarding_transfer_id,
            cargo_transfer_id,
            disembark_transfer_id,
            duplicate_operations,
            fault_recoveries,
            unique_entity_locations,
            persistent_facts_preserved,
        },
        mobile: MobileRegionEvidence {
            region_id: ids.ship_region,
            vehicle_id: ids.ship_vehicle,
            start_cell: port_cell,
            end_cell: east_cell,
            transfer_count_before_move,
            transfer_count_after_move,
            passenger_ids_before,
            passenger_ids_after,
            cargo_stayed_mobile,
            internal_authority_preserved,
        },
        remote_summary_count: world.remote_summary_count(),
        remote_person_is_not_detailed,
        parallel,
        hotspot,
        final_fingerprint,
    })
}

fn static_region(region_id: RegionId, worker_id: WorkerId, cell: HexCoord) -> RegionShard {
    RegionShard {
        id: region_id,
        kind: RegionKind::Static {
            cells: BTreeSet::from([cell]),
        },
        writer: worker_id,
        epoch: 1,
        revision: 0,
        entities: BTreeMap::new(),
    }
}

fn person(
    person_id: PersonId,
    owner_region: RegionId,
    current_cell: HexCoord,
    tier: SimTier,
    stable_seed: u64,
) -> RegionEntity {
    RegionEntity::Person(PersonRecord {
        facts: PersonFacts {
            id: person_id,
            home_cell: current_cell,
            current_cell,
            household_id: stable_seed as u32,
            profession: (stable_seed % 17) as u16,
            wealth: (stable_seed * 101) as u32,
            commitment_id: stable_seed.wrapping_mul(0x9e37_79b9),
        },
        tier,
        owner_region,
        runtime_generation: 1,
    })
}

fn transfer_entity(
    world: &mut RegionAuthorityWorld,
    source_token: AuthorityToken,
    destination_token: AuthorityToken,
    transfer_id: TransferId,
    key: EntityKey,
) -> Result<(), P7HarnessError> {
    world.prepare_transfer(
        source_token,
        transfer_id,
        key,
        destination_token.region_id,
    )?;
    world.accept_transfer(destination_token, transfer_id)?;
    world.commit_transfer(source_token, destination_token, transfer_id)?;
    world.acknowledge_transfer(transfer_id)?;
    Ok(())
}

fn owner_evidence(
    world: &RegionAuthorityWorld,
    ids: &ScenarioIds,
) -> Result<Vec<OwnerEvidence>, P7HarnessError> {
    let mut owners = [
        ids.west_region,
        ids.port_region,
        ids.east_region,
        ids.ship_region,
    ]
    .into_iter()
    .map(|region_id| {
        let token = world.token(region_id)?;
        Ok(OwnerEvidence {
            region_id,
            worker_id: token.worker_id,
            epoch: token.epoch,
        })
    })
    .collect::<Result<Vec<_>, RegionAuthorityError>>()?;
    owners.sort_by_key(|owner| owner.region_id);
    Ok(owners)
}

fn people_in_region(
    world: &RegionAuthorityWorld,
    region_id: RegionId,
) -> Result<BTreeSet<PersonId>, P7HarnessError> {
    Ok(world
        .region(region_id)?
        .entities
        .keys()
        .filter_map(|key| match key {
            EntityKey::Person(person_id) => Some(*person_id),
            EntityKey::Cargo(_) => None,
        })
        .collect())
}

fn persistent_signature(entity: &RegionEntity) -> Option<(PersonId, HexCoord, u32, u16, u32, u64)> {
    let RegionEntity::Person(record) = entity else {
        return None;
    };
    Some((
        record.facts.id,
        record.facts.home_cell,
        record.facts.household_id,
        record.facts.profession,
        record.facts.wealth,
        record.facts.commitment_id,
    ))
}

fn unique_regions_and_tokens(owners: &[OwnerEvidence]) -> bool {
    owners.iter().map(|owner| owner.region_id).collect::<BTreeSet<_>>().len() == owners.len()
        && owners.iter().all(|owner| owner.epoch > 0)
}

fn record(
    checks: &mut Vec<ValidationStep>,
    observe: &mut impl FnMut(&ValidationStep),
    title: &str,
    detail: String,
    passed: bool,
) {
    let step = ValidationStep {
        index: checks.len() + 1,
        title: title.to_owned(),
        detail,
        passed,
    };
    observe(&step);
    checks.push(step);
}

#[derive(Debug, Error)]
pub enum P7HarnessError {
    #[error("region authority failed: {0}")]
    Region(#[from] RegionAuthorityError),
    #[error("required entity is missing: {0:?}")]
    MissingEntity(EntityKey),
    #[error("JSON failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("I/O failed: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_acceptance_scenario_passes_all_checks() {
        let report = run_validation(|_| {}).expect("P7 scenario");
        assert!(report.all_passed);
        assert!(report.fingerprints_match);
        assert_eq!(report.checks.len(), 15);
        assert_eq!(report.first_run.hotspot.unique_writers, 1);
        assert!(report.first_run.parallel.identical);
    }
}
