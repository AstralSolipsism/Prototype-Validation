#![forbid(unsafe_code)]

use authoritative_world_core::{
    ActorLocation, AuditContext, AuthorityCommand, AuthorityCommandEnvelope, BuildingState,
    ContainerState, DoorState, ItemOwner, ItemState, P5_SCHEMA_VERSION, PlayerState,
    RouteAuthority, VehicleState, WorldRevision, WorldState,
};
use p4_region_scale_scenario::generate_region_scale_world;
use protocol::EntityVersion;
use region_scale_core::{RegionScaleError, activate_cell};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;
use world_generation_core::HexCoord;
use world_ids::{
    BuildingInstanceId, ClientId, CommandId, ContainerId, DoorId, ItemId, OpeningId, PersonId,
    RouteId, SessionId, VehicleId, WallId, WorldId,
};
use world_time::WorldInstant;

#[derive(Clone, Debug)]
pub struct P5Scenario {
    pub baseline: WorldState,
    pub ids: ScenarioIds,
}

#[derive(Clone, Debug)]
pub struct ScenarioIds {
    pub world_id: WorldId,
    pub player_a: PersonId,
    pub player_b: PersonId,
    pub client_a: ClientId,
    pub client_b: ClientId,
    pub session_a: SessionId,
    pub session_b: SessionId,
    pub reconnect_session_a: SessionId,
    pub start_cell: HexCoord,
    pub destination_cell: HexCoord,
    pub route_id: RouteId,
    pub start_building: BuildingInstanceId,
    pub destination_building: BuildingInstanceId,
    pub door_id: DoorId,
    pub item_id: ItemId,
    pub container_id: ContainerId,
    pub vehicle_id: VehicleId,
    pub wall_id: WallId,
    pub opening_id: OpeningId,
}

pub fn generate_p5_scenario() -> Result<P5Scenario, ScenarioError> {
    let region = generate_region_scale_world()?;
    let primary_route = region
        .routes
        .first()
        .ok_or(ScenarioError::MissingRoute)?;
    let mut ordered_cells = primary_route.crossed_cells.clone();
    ordered_cells.dedup();
    let start_cell = *ordered_cells.first().ok_or(ScenarioError::MissingRoute)?;
    let destination_cell = *ordered_cells.last().ok_or(ScenarioError::MissingRoute)?;
    let start_materialization = activate_cell(&region, start_cell)?;
    let p4_start_building = start_materialization
        .focused
        .buildings
        .first()
        .map(|building| building.instance_id)
        .unwrap_or_else(|| BuildingInstanceId::from_u128(0x5000_0001));

    let ids = ScenarioIds {
        world_id: region.atlas.world_id,
        player_a: PersonId::from_u128(0x5000_00a1),
        player_b: PersonId::from_u128(0x5000_00b2),
        client_a: ClientId::from_u128(0x5000_0ca1),
        client_b: ClientId::from_u128(0x5000_0cb2),
        session_a: SessionId::from_u128(0x5000_05a1),
        session_b: SessionId::from_u128(0x5000_05b2),
        reconnect_session_a: SessionId::from_u128(0x5000_15a1),
        start_cell,
        destination_cell,
        route_id: primary_route.id,
        start_building: p4_start_building,
        destination_building: BuildingInstanceId::from_u128(0x5000_0d57),
        door_id: DoorId::from_u128(0x5000_d001),
        item_id: ItemId::from_u128(0x5000_1701),
        container_id: ContainerId::from_u128(0x5000_c001),
        vehicle_id: VehicleId::from_u128(0x5000_0e01),
        wall_id: WallId::from_u128(0x5000_a111),
        opening_id: OpeningId::from_u128(0x5000_0f11),
    };

    let mut players = BTreeMap::new();
    for player_id in [ids.player_a, ids.player_b] {
        players.insert(
            player_id,
            PlayerState {
                id: player_id,
                current_cell: start_cell,
                location: ActorLocation::InCell { cell: start_cell },
                inventory: BTreeMap::new(),
                version: EntityVersion(0),
            },
        );
    }

    let mut buildings = BTreeMap::new();
    buildings.insert(
        ids.start_building,
        BuildingState {
            id: ids.start_building,
            cell: start_cell,
            wall_ids: BTreeSet::from([WallId::from_u128(0x5000_a001)]),
            openings: BTreeMap::new(),
            editors: BTreeSet::from([ids.player_a]),
            version: EntityVersion(0),
        },
    );
    buildings.insert(
        ids.destination_building,
        BuildingState {
            id: ids.destination_building,
            cell: destination_cell,
            wall_ids: BTreeSet::from([ids.wall_id]),
            openings: BTreeMap::new(),
            editors: BTreeSet::from([ids.player_a]),
            version: EntityVersion(0),
        },
    );

    let mut doors = BTreeMap::new();
    doors.insert(
        ids.door_id,
        DoorState {
            id: ids.door_id,
            building_id: ids.start_building,
            is_open: false,
            version: EntityVersion(0),
        },
    );

    let mut containers = BTreeMap::new();
    containers.insert(
        ids.container_id,
        ContainerState {
            id: ids.container_id,
            cell: start_cell,
            item_ids: BTreeSet::from([ids.item_id]),
            version: EntityVersion(0),
        },
    );

    let mut items = BTreeMap::new();
    items.insert(
        ids.item_id,
        ItemState {
            id: ids.item_id,
            archetype: "sealed-port-ledger".into(),
            quantity: 1,
            owner: ItemOwner::Container(ids.container_id),
            version: EntityVersion(0),
        },
    );

    let mut vehicles = BTreeMap::new();
    vehicles.insert(
        ids.vehicle_id,
        VehicleState {
            id: ids.vehicle_id,
            cell: destination_cell,
            passengers: BTreeSet::new(),
            version: EntityVersion(0),
        },
    );

    let routes = region
        .routes
        .iter()
        .map(|route| {
            let mut cells = route.crossed_cells.clone();
            cells.dedup();
            (
                route.id,
                RouteAuthority {
                    id: route.id,
                    ordered_cells: cells,
                },
            )
        })
        .collect();

    Ok(P5Scenario {
        baseline: WorldState {
            world_id: ids.world_id,
            revision: WorldRevision::ZERO,
            clock: WorldInstant::ZERO,
            players,
            doors,
            items,
            containers,
            buildings,
            vehicles,
            routes,
        },
        ids,
    })
}

#[derive(Clone, Debug)]
pub struct ScenarioCommandFactory {
    actor_id: PersonId,
    client_id: ClientId,
    session_id: SessionId,
    world_id: WorldId,
    command_namespace: u128,
    next_sequence: u64,
    next_tick: i64,
}

impl ScenarioCommandFactory {
    pub const fn new(
        actor_id: PersonId,
        client_id: ClientId,
        session_id: SessionId,
        world_id: WorldId,
        command_namespace: u128,
    ) -> Self {
        Self {
            actor_id,
            client_id,
            session_id,
            world_id,
            command_namespace,
            next_sequence: 0,
            next_tick: 1,
        }
    }

    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    pub fn envelope(
        &mut self,
        payload: AuthorityCommand,
        expected_world_revision: Option<WorldRevision>,
        expected_entity_version: Option<EntityVersion>,
        trace_label: impl Into<String>,
    ) -> AuthorityCommandEnvelope {
        let sequence = self.next_sequence;
        self.next_sequence += 1;
        let command_id = CommandId::from_u128(
            self.command_namespace
                .wrapping_add(u128::from(sequence).wrapping_add(1)),
        );
        let issued_at = WorldInstant::from_ticks(self.next_tick);
        self.next_tick += 1;
        AuthorityCommandEnvelope {
            command_id,
            actor_id: self.actor_id,
            target_world: self.world_id,
            issued_at,
            expected_world_revision,
            expected_entity_version,
            schema_version: P5_SCHEMA_VERSION,
            audit: AuditContext {
                client_id: self.client_id,
                session_id: self.session_id,
                request_sequence: sequence,
                trace_label: trace_label.into(),
            },
            payload,
        }
    }
}

#[derive(Debug, Error)]
pub enum ScenarioError {
    #[error("P4 region-scale world failed: {0}")]
    Region(#[from] RegionScaleError),
    #[error("P4 scenario did not contain a route")]
    MissingRoute,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn baseline_uses_p4_world_identity_and_route() {
        let scenario = generate_p5_scenario().expect("scenario");
        assert_eq!(scenario.baseline.world_id, scenario.ids.world_id);
        assert!(scenario.baseline.routes.contains_key(&scenario.ids.route_id));
        assert_ne!(scenario.ids.start_cell, scenario.ids.destination_cell);
        assert_eq!(scenario.baseline.players.len(), 2);
    }

    #[test]
    fn command_factory_produces_stable_monotonic_sequences() {
        let scenario = generate_p5_scenario().expect("scenario");
        let mut factory = ScenarioCommandFactory::new(
            scenario.ids.player_a,
            scenario.ids.client_a,
            scenario.ids.session_a,
            scenario.ids.world_id,
            0x5000_0000_0000_0000,
        );
        let first = factory.envelope(
            AuthorityCommand::Connect {
                last_acknowledged_revision: WorldRevision::ZERO,
                interest_center: scenario.ids.start_cell,
            },
            None,
            None,
            "connect",
        );
        let second = factory.envelope(AuthorityCommand::Disconnect, None, None, "disconnect");
        assert_eq!(first.audit.request_sequence, 0);
        assert_eq!(second.audit.request_sequence, 1);
        assert_ne!(first.command_id, second.command_id);
    }
}
