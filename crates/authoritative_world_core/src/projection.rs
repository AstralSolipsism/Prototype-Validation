use crate::engine::AuthorityError;
use crate::model::*;
use protocol::EntityVersion;
use replay_core::StateFingerprint;
use std::collections::BTreeMap;
use world_ids::{BuildingInstanceId, ContainerId, DoorId, ItemId, PersonId, VehicleId};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClientProjection {
    pub world_id: world_ids::WorldId,
    pub revision: WorldRevision,
    pub interest_center: world_generation_core::HexCoord,
    pub players: BTreeMap<PersonId, PlayerState>,
    pub doors: BTreeMap<DoorId, DoorState>,
    pub items: BTreeMap<ItemId, ItemState>,
    pub containers: BTreeMap<ContainerId, ContainerState>,
    pub buildings: BTreeMap<BuildingInstanceId, BuildingState>,
    pub vehicles: BTreeMap<VehicleId, VehicleState>,
    pub state_fingerprint: StateFingerprint,
    pub visual_cache_key: Option<StateFingerprint>,
}

impl ClientProjection {
    pub fn from_snapshot(snapshot: ClientSnapshot) -> Self {
        Self {
            world_id: snapshot.world_id,
            revision: snapshot.revision,
            interest_center: snapshot.interest_center,
            players: snapshot.players,
            doors: snapshot.doors,
            items: snapshot.items,
            containers: snapshot.containers,
            buildings: snapshot.buildings,
            vehicles: snapshot.vehicles,
            state_fingerprint: snapshot.state_fingerprint,
            visual_cache_key: None,
        }
    }

    pub fn apply_sync(&mut self, sync: SyncPayload) -> Result<(), AuthorityError> {
        match sync {
            SyncPayload::Snapshot(snapshot) => {
                *self = Self::from_snapshot(snapshot);
            }
            SyncPayload::Deltas(deltas) => {
                for delta in deltas {
                    self.apply_delta(&delta)?;
                }
            }
        }
        Ok(())
    }

    pub fn apply_receipt(&mut self, receipt: &CommandReceipt) -> Result<(), AuthorityError> {
        if let Some(sync) = &receipt.sync {
            self.apply_sync(sync.clone())?;
        }
        if let Some(delta) = &receipt.delta
            && delta.to_revision > self.revision
        {
            self.apply_delta(delta)?;
        }
        if self.revision == receipt.world_revision {
            self.state_fingerprint = receipt.state_fingerprint;
        }
        Ok(())
    }

    pub fn clear_visual_cache(&mut self) {
        self.visual_cache_key = None;
    }

    pub fn rebuild_visual_cache(&mut self) {
        self.visual_cache_key = Some(self.state_fingerprint);
    }

    pub fn visual_cache_is_current(&self) -> bool {
        self.visual_cache_key == Some(self.state_fingerprint)
    }

    pub fn matches_snapshot(&self, snapshot: &ClientSnapshot) -> bool {
        self.world_id == snapshot.world_id
            && self.revision == snapshot.revision
            && self.players == snapshot.players
            && self.doors == snapshot.doors
            && self.items == snapshot.items
            && self.containers == snapshot.containers
            && self.buildings == snapshot.buildings
            && self.vehicles == snapshot.vehicles
            && self.state_fingerprint == snapshot.state_fingerprint
    }

    fn apply_delta(&mut self, delta: &WorldDelta) -> Result<(), AuthorityError> {
        if delta.from_revision != self.revision {
            return Err(AuthorityError::JournalRevision {
                expected: self.revision,
                actual: delta.from_revision,
            });
        }
        for event in &delta.events {
            self.apply_event(event)?;
        }
        self.revision = delta.to_revision;
        self.visual_cache_key = None;
        Ok(())
    }

    fn apply_event(&mut self, event: &WorldEvent) -> Result<(), AuthorityError> {
        match event {
            WorldEvent::PlayerMoved {
                player_id, to_cell, ..
            } => {
                let player = self
                    .players
                    .get_mut(player_id)
                    .ok_or_else(|| AuthorityError::Invariant("projection player missing".into()))?;
                player.current_cell = *to_cell;
                player.location = ActorLocation::InCell { cell: *to_cell };
                player.version = EntityVersion(player.version.0 + 1);
            }
            WorldEvent::DoorToggled {
                door_id, is_open, ..
            } => {
                let door = self
                    .doors
                    .get_mut(door_id)
                    .ok_or_else(|| AuthorityError::Invariant("projection door missing".into()))?;
                door.is_open = *is_open;
                door.version = EntityVersion(door.version.0 + 1);
            }
            WorldEvent::ItemTransferred {
                item_id,
                quantity,
                from,
                to,
                ..
            } => {
                self.remove_item(*item_id, *quantity, from)?;
                self.add_item(*item_id, *quantity, to)?;
                let item = self
                    .items
                    .get_mut(item_id)
                    .ok_or_else(|| AuthorityError::Invariant("projection item missing".into()))?;
                item.owner = to.clone();
                item.version = EntityVersion(item.version.0 + 1);
            }
            WorldEvent::PlayerEnteredBuilding {
                player_id,
                building_id,
                cell,
                ..
            } => {
                let player = self
                    .players
                    .get_mut(player_id)
                    .ok_or_else(|| AuthorityError::Invariant("projection player missing".into()))?;
                player.current_cell = *cell;
                player.location = ActorLocation::InBuilding {
                    cell: *cell,
                    building_id: *building_id,
                };
                player.version = EntityVersion(player.version.0 + 1);
            }
            WorldEvent::PlayerLeftBuilding {
                player_id, cell, ..
            } => {
                let player = self
                    .players
                    .get_mut(player_id)
                    .ok_or_else(|| AuthorityError::Invariant("projection player missing".into()))?;
                player.current_cell = *cell;
                player.location = ActorLocation::InCell { cell: *cell };
                player.version = EntityVersion(player.version.0 + 1);
            }
            WorldEvent::PlayerBoardedVehicle {
                player_id,
                vehicle_id,
                cell,
                ..
            } => {
                let player = self
                    .players
                    .get_mut(player_id)
                    .ok_or_else(|| AuthorityError::Invariant("projection player missing".into()))?;
                player.current_cell = *cell;
                player.location = ActorLocation::InVehicle {
                    cell: *cell,
                    vehicle_id: *vehicle_id,
                };
                player.version = EntityVersion(player.version.0 + 1);
                let vehicle = self.vehicles.get_mut(vehicle_id).ok_or_else(|| {
                    AuthorityError::Invariant("projection vehicle missing".into())
                })?;
                vehicle.passengers.insert(*player_id);
                vehicle.version = EntityVersion(vehicle.version.0 + 1);
            }
            WorldEvent::PlayerDisembarkedVehicle {
                player_id,
                vehicle_id,
                cell,
                ..
            } => {
                let player = self
                    .players
                    .get_mut(player_id)
                    .ok_or_else(|| AuthorityError::Invariant("projection player missing".into()))?;
                player.current_cell = *cell;
                player.location = ActorLocation::InCell { cell: *cell };
                player.version = EntityVersion(player.version.0 + 1);
                let vehicle = self.vehicles.get_mut(vehicle_id).ok_or_else(|| {
                    AuthorityError::Invariant("projection vehicle missing".into())
                })?;
                vehicle.passengers.remove(player_id);
                vehicle.version = EntityVersion(vehicle.version.0 + 1);
            }
            WorldEvent::OpeningAdded {
                building_id,
                wall_id,
                opening_id,
                ..
            } => {
                let building = self.buildings.get_mut(building_id).ok_or_else(|| {
                    AuthorityError::Invariant("projection building missing".into())
                })?;
                building.openings.insert(*opening_id, *wall_id);
                building.version = EntityVersion(building.version.0 + 1);
            }
        }
        Ok(())
    }

    fn remove_item(
        &mut self,
        item_id: ItemId,
        quantity: u32,
        owner: &ItemOwner,
    ) -> Result<(), AuthorityError> {
        match owner {
            ItemOwner::Container(container_id) => {
                let container = self.containers.get_mut(container_id).ok_or_else(|| {
                    AuthorityError::Invariant("projection source container missing".into())
                })?;
                container.item_ids.remove(&item_id);
                container.version = EntityVersion(container.version.0 + 1);
            }
            ItemOwner::Person(person_id) => {
                let player = self.players.get_mut(person_id).ok_or_else(|| {
                    AuthorityError::Invariant("projection source player missing".into())
                })?;
                if player.inventory.remove(&item_id) != Some(quantity) {
                    return Err(AuthorityError::Invariant(
                        "projection source inventory mismatch".into(),
                    ));
                }
                player.version = EntityVersion(player.version.0 + 1);
            }
            ItemOwner::Vehicle(_) => {}
        }
        Ok(())
    }

    fn add_item(
        &mut self,
        item_id: ItemId,
        quantity: u32,
        owner: &ItemOwner,
    ) -> Result<(), AuthorityError> {
        match owner {
            ItemOwner::Container(container_id) => {
                let container = self.containers.get_mut(container_id).ok_or_else(|| {
                    AuthorityError::Invariant("projection target container missing".into())
                })?;
                container.item_ids.insert(item_id);
                container.version = EntityVersion(container.version.0 + 1);
            }
            ItemOwner::Person(person_id) => {
                let player = self.players.get_mut(person_id).ok_or_else(|| {
                    AuthorityError::Invariant("projection target player missing".into())
                })?;
                player.inventory.insert(item_id, quantity);
                player.version = EntityVersion(player.version.0 + 1);
            }
            ItemOwner::Vehicle(_) => {}
        }
        Ok(())
    }
}
