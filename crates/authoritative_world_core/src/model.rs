use protocol::{CommandRejection, EntityVersion, SchemaVersion};
use replay_core::{FingerprintBuilder, StateFingerprint};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use world_generation_core::HexCoord;
use world_ids::{
    BuildingInstanceId, ClientId, CommandId, ContainerId, DoorId, EventId, ItemId, OpeningId,
    PersonId, RouteId, SessionId, SnapshotId, VehicleId, WallId, WorldId,
};
use world_time::WorldInstant;

pub const P5_SCHEMA_VERSION: SchemaVersion = SchemaVersion(1);

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct WorldRevision(pub u64);

impl WorldRevision {
    pub const ZERO: Self = Self(0);

    pub const fn next(self) -> Self {
        Self(self.0 + 1)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditContext {
    pub client_id: ClientId,
    pub session_id: SessionId,
    pub request_sequence: u64,
    pub trace_label: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorityCommandEnvelope {
    pub command_id: CommandId,
    pub actor_id: PersonId,
    pub target_world: WorldId,
    pub issued_at: WorldInstant,
    pub expected_world_revision: Option<WorldRevision>,
    pub expected_entity_version: Option<EntityVersion>,
    pub schema_version: SchemaVersion,
    pub audit: AuditContext,
    pub payload: AuthorityCommand,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthorityCommand {
    Connect {
        last_acknowledged_revision: WorldRevision,
        interest_center: HexCoord,
    },
    Disconnect,
    MoveAlongRoute {
        route_id: RouteId,
        destination: HexCoord,
    },
    ToggleDoor {
        door_id: DoorId,
    },
    TakeItem {
        item_id: ItemId,
        quantity: u32,
    },
    PutItem {
        item_id: ItemId,
        quantity: u32,
        container_id: ContainerId,
    },
    EnterBuilding {
        building_id: BuildingInstanceId,
    },
    LeaveBuilding,
    BoardVehicle {
        vehicle_id: VehicleId,
    },
    DisembarkVehicle,
    AddOpening {
        building_id: BuildingInstanceId,
        wall_id: WallId,
        opening_id: OpeningId,
    },
    RequestSync {
        last_seen_revision: WorldRevision,
        interest_center: HexCoord,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActorLocation {
    InCell {
        cell: HexCoord,
    },
    InBuilding {
        cell: HexCoord,
        building_id: BuildingInstanceId,
    },
    InVehicle {
        cell: HexCoord,
        vehicle_id: VehicleId,
    },
}

impl ActorLocation {
    pub const fn cell(&self) -> HexCoord {
        match self {
            Self::InCell { cell }
            | Self::InBuilding { cell, .. }
            | Self::InVehicle { cell, .. } => *cell,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerState {
    pub id: PersonId,
    pub current_cell: HexCoord,
    pub location: ActorLocation,
    pub inventory: BTreeMap<ItemId, u32>,
    pub version: EntityVersion,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoorState {
    pub id: DoorId,
    pub building_id: BuildingInstanceId,
    pub is_open: bool,
    pub version: EntityVersion,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemOwner {
    Container(ContainerId),
    Person(PersonId),
    Vehicle(VehicleId),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemState {
    pub id: ItemId,
    pub archetype: String,
    pub quantity: u32,
    pub owner: ItemOwner,
    pub version: EntityVersion,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerState {
    pub id: ContainerId,
    pub cell: HexCoord,
    pub item_ids: BTreeSet<ItemId>,
    pub version: EntityVersion,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildingState {
    pub id: BuildingInstanceId,
    pub cell: HexCoord,
    pub wall_ids: BTreeSet<WallId>,
    pub openings: BTreeMap<OpeningId, WallId>,
    pub editors: BTreeSet<PersonId>,
    pub version: EntityVersion,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VehicleState {
    pub id: VehicleId,
    pub cell: HexCoord,
    pub passengers: BTreeSet<PersonId>,
    pub version: EntityVersion,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteAuthority {
    pub id: RouteId,
    pub ordered_cells: Vec<HexCoord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldState {
    pub world_id: WorldId,
    pub revision: WorldRevision,
    pub clock: WorldInstant,
    pub players: BTreeMap<PersonId, PlayerState>,
    pub doors: BTreeMap<DoorId, DoorState>,
    pub items: BTreeMap<ItemId, ItemState>,
    pub containers: BTreeMap<ContainerId, ContainerState>,
    pub buildings: BTreeMap<BuildingInstanceId, BuildingState>,
    pub vehicles: BTreeMap<VehicleId, VehicleState>,
    pub routes: BTreeMap<RouteId, RouteAuthority>,
}

impl WorldState {
    pub fn semantic_fingerprint(&self) -> Result<StateFingerprint, serde_json::Error> {
        fingerprint_serializable(self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionState {
    pub client_id: ClientId,
    pub session_id: SessionId,
    pub actor_id: PersonId,
    pub last_acknowledged_revision: WorldRevision,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorldEvent {
    PlayerMoved {
        event_id: EventId,
        player_id: PersonId,
        route_id: RouteId,
        from_cell: HexCoord,
        to_cell: HexCoord,
    },
    DoorToggled {
        event_id: EventId,
        door_id: DoorId,
        is_open: bool,
    },
    ItemTransferred {
        event_id: EventId,
        item_id: ItemId,
        quantity: u32,
        from: ItemOwner,
        to: ItemOwner,
    },
    PlayerEnteredBuilding {
        event_id: EventId,
        player_id: PersonId,
        building_id: BuildingInstanceId,
        cell: HexCoord,
    },
    PlayerLeftBuilding {
        event_id: EventId,
        player_id: PersonId,
        building_id: BuildingInstanceId,
        cell: HexCoord,
    },
    PlayerBoardedVehicle {
        event_id: EventId,
        player_id: PersonId,
        vehicle_id: VehicleId,
        cell: HexCoord,
    },
    PlayerDisembarkedVehicle {
        event_id: EventId,
        player_id: PersonId,
        vehicle_id: VehicleId,
        cell: HexCoord,
    },
    OpeningAdded {
        event_id: EventId,
        building_id: BuildingInstanceId,
        wall_id: WallId,
        opening_id: OpeningId,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldDelta {
    pub from_revision: WorldRevision,
    pub to_revision: WorldRevision,
    pub events: Vec<WorldEvent>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientSnapshot {
    pub world_id: WorldId,
    pub revision: WorldRevision,
    pub interest_center: HexCoord,
    pub players: BTreeMap<PersonId, PlayerState>,
    pub doors: BTreeMap<DoorId, DoorState>,
    pub items: BTreeMap<ItemId, ItemState>,
    pub containers: BTreeMap<ContainerId, ContainerState>,
    pub buildings: BTreeMap<BuildingInstanceId, BuildingState>,
    pub vehicles: BTreeMap<VehicleId, VehicleState>,
    pub state_fingerprint: StateFingerprint,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncPayload {
    Snapshot(ClientSnapshot),
    Deltas(Vec<WorldDelta>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReceiptDisposition {
    Applied,
    Noop,
    Rejected,
    Duplicate,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredReceipt {
    pub command_id: CommandId,
    pub world_revision: WorldRevision,
    pub resulting_entity_version: Option<EntityVersion>,
    pub rejection: Option<CommandRejection>,
    pub delta: Option<WorldDelta>,
    pub sync: Option<SyncPayload>,
    pub state_fingerprint: StateFingerprint,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandReceipt {
    pub command_id: CommandId,
    pub disposition: ReceiptDisposition,
    pub world_revision: WorldRevision,
    pub resulting_entity_version: Option<EntityVersion>,
    pub rejection: Option<CommandRejection>,
    pub delta: Option<WorldDelta>,
    pub sync: Option<SyncPayload>,
    pub state_fingerprint: StateFingerprint,
}

impl StoredReceipt {
    pub fn as_receipt(&self, disposition: ReceiptDisposition) -> CommandReceipt {
        CommandReceipt {
            command_id: self.command_id,
            disposition,
            world_revision: self.world_revision,
            resulting_entity_version: self.resulting_entity_version,
            rejection: self.rejection.clone(),
            delta: self.delta.clone(),
            sync: self.sync.clone(),
            state_fingerprint: self.state_fingerprint,
        }
    }

    pub fn first_disposition(&self) -> ReceiptDisposition {
        if self.rejection.is_some() {
            ReceiptDisposition::Rejected
        } else if self.delta.is_some() {
            ReceiptDisposition::Applied
        } else {
            ReceiptDisposition::Noop
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalRecord {
    pub sequence: u64,
    pub envelope: AuthorityCommandEnvelope,
    pub stored_receipt: StoredReceipt,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthoritySnapshot {
    pub snapshot_id: SnapshotId,
    pub schema_version: SchemaVersion,
    pub next_journal_sequence: u64,
    pub world: WorldState,
    pub idempotency: BTreeMap<CommandId, StoredReceipt>,
    pub state_fingerprint: StateFingerprint,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandProcessResult {
    pub receipt: CommandReceipt,
    pub journal_record: Option<JournalRecord>,
}

pub fn fingerprint_serializable<T: Serialize>(
    value: &T,
) -> Result<StateFingerprint, serde_json::Error> {
    let bytes = serde_json::to_vec(value)?;
    let mut builder = FingerprintBuilder::default();
    builder.write_bytes(&bytes);
    Ok(builder.finish())
}
