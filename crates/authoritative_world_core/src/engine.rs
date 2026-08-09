use crate::model::*;
use protocol::{CommandRejection, EntityVersion, RejectionCode};
use std::collections::BTreeMap;
use thiserror::Error;
use world_ids::{CommandId, EventId, SnapshotId};

#[derive(Debug, Error)]
pub enum AuthorityError {
    #[error("serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("journal sequence mismatch: expected {expected}, actual {actual}")]
    JournalSequence { expected: u64, actual: u64 },
    #[error("journal revision mismatch: expected {expected:?}, actual {actual:?}")]
    JournalRevision {
        expected: WorldRevision,
        actual: WorldRevision,
    },
    #[error("journal state fingerprint mismatch")]
    JournalFingerprint,
    #[error("entity version conflict: expected {expected}, actual {actual}")]
    VersionConflict { expected: u64, actual: u64 },
    #[error("domain invariant failed: {0}")]
    Invariant(String),
}

#[derive(Clone, Debug)]
pub struct AuthorityServer {
    world: WorldState,
    idempotency: BTreeMap<CommandId, StoredReceipt>,
    sessions: BTreeMap<world_ids::ClientId, SessionState>,
    journal: Vec<JournalRecord>,
    next_journal_sequence: u64,
    delta_floor_revision: WorldRevision,
}

impl AuthorityServer {
    pub fn new(world: WorldState) -> Result<Self, AuthorityError> {
        world.semantic_fingerprint()?;
        let delta_floor_revision = world.revision;
        Ok(Self {
            world,
            idempotency: BTreeMap::new(),
            sessions: BTreeMap::new(),
            journal: Vec::new(),
            next_journal_sequence: 0,
            delta_floor_revision,
        })
    }

    pub fn from_snapshot(snapshot: AuthoritySnapshot) -> Result<Self, AuthorityError> {
        let actual = snapshot.world.semantic_fingerprint()?;
        if actual != snapshot.state_fingerprint {
            return Err(AuthorityError::JournalFingerprint);
        }
        let delta_floor_revision = snapshot.world.revision;
        Ok(Self {
            world: snapshot.world,
            idempotency: snapshot.idempotency,
            sessions: BTreeMap::new(),
            journal: Vec::new(),
            next_journal_sequence: snapshot.next_journal_sequence,
            delta_floor_revision,
        })
    }

    pub fn world(&self) -> &WorldState {
        &self.world
    }

    pub fn sessions(&self) -> &BTreeMap<world_ids::ClientId, SessionState> {
        &self.sessions
    }

    pub fn journal(&self) -> &[JournalRecord] {
        &self.journal
    }

    pub fn next_journal_sequence(&self) -> u64 {
        self.next_journal_sequence
    }

    pub fn process(
        &mut self,
        envelope: AuthorityCommandEnvelope,
    ) -> Result<CommandProcessResult, AuthorityError> {
        if let Some(stored) = self.idempotency.get(&envelope.command_id) {
            return Ok(CommandProcessResult {
                receipt: stored.as_receipt(ReceiptDisposition::Duplicate),
                journal_record: None,
            });
        }

        let stored_receipt = self.execute_first(&envelope)?;
        let receipt = stored_receipt.as_receipt(stored_receipt.first_disposition());
        let record = JournalRecord {
            sequence: self.next_journal_sequence,
            envelope,
            stored_receipt: stored_receipt.clone(),
        };
        self.next_journal_sequence += 1;
        self.idempotency
            .insert(stored_receipt.command_id, stored_receipt);
        self.journal.push(record.clone());

        Ok(CommandProcessResult {
            receipt,
            journal_record: Some(record),
        })
    }

    pub fn snapshot(&self, snapshot_id: SnapshotId) -> Result<AuthoritySnapshot, AuthorityError> {
        Ok(AuthoritySnapshot {
            snapshot_id,
            schema_version: P5_SCHEMA_VERSION,
            next_journal_sequence: self.next_journal_sequence,
            world: self.world.clone(),
            idempotency: self.idempotency.clone(),
            state_fingerprint: self.world.semantic_fingerprint()?,
        })
    }

    pub fn apply_journal_record(&mut self, record: JournalRecord) -> Result<(), AuthorityError> {
        if record.sequence != self.next_journal_sequence {
            return Err(AuthorityError::JournalSequence {
                expected: self.next_journal_sequence,
                actual: record.sequence,
            });
        }
        if self.idempotency.contains_key(&record.envelope.command_id) {
            return Err(AuthorityError::Invariant(format!(
                "journal repeats command {}",
                record.envelope.command_id
            )));
        }
        if let Some(delta) = &record.stored_receipt.delta {
            if delta.from_revision != self.world.revision {
                return Err(AuthorityError::JournalRevision {
                    expected: self.world.revision,
                    actual: delta.from_revision,
                });
            }
            for event in &delta.events {
                apply_event(&mut self.world, event)?;
            }
            self.world.revision = delta.to_revision;
            self.world.clock = record.envelope.issued_at;
        }
        let actual = self.world.semantic_fingerprint()?;
        if actual != record.stored_receipt.state_fingerprint {
            return Err(AuthorityError::JournalFingerprint);
        }
        self.idempotency
            .insert(record.envelope.command_id, record.stored_receipt.clone());
        self.next_journal_sequence += 1;
        self.journal.push(record);
        Ok(())
    }

    pub fn client_snapshot(
        &self,
        interest_center: world_generation_core::HexCoord,
    ) -> Result<ClientSnapshot, AuthorityError> {
        Ok(ClientSnapshot {
            world_id: self.world.world_id,
            revision: self.world.revision,
            interest_center,
            players: self.world.players.clone(),
            doors: self.world.doors.clone(),
            items: self.world.items.clone(),
            containers: self.world.containers.clone(),
            buildings: self.world.buildings.clone(),
            vehicles: self.world.vehicles.clone(),
            state_fingerprint: self.world.semantic_fingerprint()?,
        })
    }

    pub fn sync_payload(
        &self,
        last_seen_revision: WorldRevision,
        interest_center: world_generation_core::HexCoord,
    ) -> Result<SyncPayload, AuthorityError> {
        if last_seen_revision == self.world.revision {
            return Ok(SyncPayload::Deltas(Vec::new()));
        }
        if last_seen_revision < self.delta_floor_revision {
            return Ok(SyncPayload::Snapshot(
                self.client_snapshot(interest_center)?,
            ));
        }

        let mut cursor = last_seen_revision;
        let mut deltas = Vec::new();
        for delta in self
            .journal
            .iter()
            .filter_map(|record| record.stored_receipt.delta.as_ref())
        {
            if delta.to_revision <= last_seen_revision {
                continue;
            }
            if delta.from_revision != cursor {
                return Ok(SyncPayload::Snapshot(
                    self.client_snapshot(interest_center)?,
                ));
            }
            cursor = delta.to_revision;
            deltas.push(delta.clone());
        }
        if cursor == self.world.revision {
            Ok(SyncPayload::Deltas(deltas))
        } else {
            Ok(SyncPayload::Snapshot(
                self.client_snapshot(interest_center)?,
            ))
        }
    }

    fn execute_first(
        &mut self,
        envelope: &AuthorityCommandEnvelope,
    ) -> Result<StoredReceipt, AuthorityError> {
        if envelope.schema_version != P5_SCHEMA_VERSION {
            return self.rejected(
                envelope.command_id,
                RejectionCode::UnsupportedSchema,
                "unsupported command schema",
            );
        }
        if envelope.target_world != self.world.world_id {
            return self.rejected(
                envelope.command_id,
                RejectionCode::InvalidPayload,
                "command targets another world",
            );
        }

        match &envelope.payload {
            AuthorityCommand::Connect {
                last_acknowledged_revision,
                interest_center,
            } => {
                if !self.world.players.contains_key(&envelope.actor_id) {
                    return self.rejected(
                        envelope.command_id,
                        RejectionCode::PermissionDenied,
                        "actor is not a player in this world",
                    );
                }
                if *last_acknowledged_revision > self.world.revision {
                    return self.rejected(
                        envelope.command_id,
                        RejectionCode::InvalidPayload,
                        "client revision is ahead of the server",
                    );
                }
                self.sessions.insert(
                    envelope.audit.client_id,
                    SessionState {
                        client_id: envelope.audit.client_id,
                        session_id: envelope.audit.session_id,
                        actor_id: envelope.actor_id,
                        last_acknowledged_revision: *last_acknowledged_revision,
                    },
                );
                self.accepted_noop(
                    envelope.command_id,
                    None,
                    Some(SyncPayload::Snapshot(
                        self.client_snapshot(*interest_center)?,
                    )),
                )
            }
            _ => {
                if let Err(rejection) = self.require_session(envelope) {
                    return self.rejected_existing(envelope.command_id, rejection);
                }
                match &envelope.payload {
                    AuthorityCommand::Disconnect => {
                        self.sessions.remove(&envelope.audit.client_id);
                        self.accepted_noop(envelope.command_id, None, None)
                    }
                    AuthorityCommand::RequestSync {
                        last_seen_revision,
                        interest_center,
                    } => {
                        let payload = self.sync_payload(*last_seen_revision, *interest_center)?;
                        if let Some(session) = self.sessions.get_mut(&envelope.audit.client_id) {
                            session.last_acknowledged_revision = self.world.revision;
                        }
                        self.accepted_noop(envelope.command_id, None, Some(payload))
                    }
                    command => {
                        if let Some(expected) = envelope.expected_world_revision
                            && expected != self.world.revision
                        {
                            return self.rejected(
                                envelope.command_id,
                                RejectionCode::VersionConflict,
                                format!(
                                    "world revision mismatch: expected {}, actual {}",
                                    expected.0, self.world.revision.0
                                ),
                            );
                        }
                        match self.execute_mutation(envelope, command) {
                            Err(AuthorityError::VersionConflict { expected, actual }) => {
                                self.rejected(
                                    envelope.command_id,
                                    RejectionCode::VersionConflict,
                                    format!(
                                        "entity version mismatch: expected {expected}, actual {actual}"
                                    ),
                                )
                            }
                            result => result,
                        }
                    }
                    AuthorityCommand::Connect { .. } => unreachable!("connect handled above"),
                }
            }
        }
    }

    fn execute_mutation(
        &mut self,
        envelope: &AuthorityCommandEnvelope,
        command: &AuthorityCommand,
    ) -> Result<StoredReceipt, AuthorityError> {
        let actor = self
            .world
            .players
            .get(&envelope.actor_id)
            .cloned()
            .ok_or_else(|| AuthorityError::Invariant("session actor disappeared".into()))?;
        let event_id = event_id(envelope.command_id, 0);

        let (event, target_version) = match command {
            AuthorityCommand::MoveAlongRoute {
                route_id,
                destination,
            } => {
                self.check_entity_version(envelope, actor.version)?;
                let route = match self.world.routes.get(route_id) {
                    Some(route) => route,
                    None => {
                        return self.rejected(
                            envelope.command_id,
                            RejectionCode::InvalidPayload,
                            "route does not exist",
                        );
                    }
                };
                let from_index = route
                    .ordered_cells
                    .iter()
                    .position(|cell| *cell == actor.current_cell);
                let to_index = route
                    .ordered_cells
                    .iter()
                    .position(|cell| *cell == *destination);
                if from_index.is_none() || to_index.is_none() || actor.current_cell == *destination
                {
                    return self.rejected(
                        envelope.command_id,
                        RejectionCode::InvalidState,
                        "destination is not a distinct cell on the selected route",
                    );
                }
                (
                    WorldEvent::PlayerMoved {
                        event_id,
                        player_id: actor.id,
                        route_id: *route_id,
                        from_cell: actor.current_cell,
                        to_cell: *destination,
                    },
                    EntityVersion(actor.version.0 + 1),
                )
            }
            AuthorityCommand::ToggleDoor { door_id } => {
                let door = match self.world.doors.get(door_id) {
                    Some(door) => door,
                    None => {
                        return self.rejected(
                            envelope.command_id,
                            RejectionCode::InvalidPayload,
                            "door does not exist",
                        );
                    }
                };
                self.check_entity_version(envelope, door.version)?;
                let building = &self.world.buildings[&door.building_id];
                if building.cell != actor.current_cell {
                    return self.rejected(
                        envelope.command_id,
                        RejectionCode::PermissionDenied,
                        "actor is not in the door's region",
                    );
                }
                (
                    WorldEvent::DoorToggled {
                        event_id,
                        door_id: *door_id,
                        is_open: !door.is_open,
                    },
                    EntityVersion(door.version.0 + 1),
                )
            }
            AuthorityCommand::TakeItem { item_id, quantity } => {
                let item = match self.world.items.get(item_id) {
                    Some(item) => item,
                    None => {
                        return self.rejected(
                            envelope.command_id,
                            RejectionCode::InvalidPayload,
                            "item does not exist",
                        );
                    }
                };
                self.check_entity_version(envelope, item.version)?;
                if *quantity != item.quantity || *quantity == 0 {
                    return self.rejected(
                        envelope.command_id,
                        RejectionCode::InvalidPayload,
                        "prototype item stacks must be transferred whole",
                    );
                }
                let ItemOwner::Container(container_id) = item.owner else {
                    return self.rejected(
                        envelope.command_id,
                        RejectionCode::InvalidState,
                        "item is not in a world container",
                    );
                };
                let container = &self.world.containers[&container_id];
                if container.cell != actor.current_cell {
                    return self.rejected(
                        envelope.command_id,
                        RejectionCode::PermissionDenied,
                        "item container is outside the actor's region",
                    );
                }
                (
                    WorldEvent::ItemTransferred {
                        event_id,
                        item_id: *item_id,
                        quantity: *quantity,
                        from: ItemOwner::Container(container_id),
                        to: ItemOwner::Person(actor.id),
                    },
                    EntityVersion(item.version.0 + 1),
                )
            }
            AuthorityCommand::PutItem {
                item_id,
                quantity,
                container_id,
            } => {
                let item = match self.world.items.get(item_id) {
                    Some(item) => item,
                    None => {
                        return self.rejected(
                            envelope.command_id,
                            RejectionCode::InvalidPayload,
                            "item does not exist",
                        );
                    }
                };
                self.check_entity_version(envelope, item.version)?;
                if item.owner != ItemOwner::Person(actor.id) || *quantity != item.quantity {
                    return self.rejected(
                        envelope.command_id,
                        RejectionCode::InvalidState,
                        "actor does not own the requested complete item stack",
                    );
                }
                let Some(container) = self.world.containers.get(container_id) else {
                    return self.rejected(
                        envelope.command_id,
                        RejectionCode::InvalidPayload,
                        "target container does not exist",
                    );
                };
                if container.cell != actor.current_cell {
                    return self.rejected(
                        envelope.command_id,
                        RejectionCode::PermissionDenied,
                        "target container is outside the actor's region",
                    );
                }
                (
                    WorldEvent::ItemTransferred {
                        event_id,
                        item_id: *item_id,
                        quantity: *quantity,
                        from: ItemOwner::Person(actor.id),
                        to: ItemOwner::Container(*container_id),
                    },
                    EntityVersion(item.version.0 + 1),
                )
            }
            AuthorityCommand::EnterBuilding { building_id } => {
                self.check_entity_version(envelope, actor.version)?;
                let Some(building) = self.world.buildings.get(building_id) else {
                    return self.rejected(
                        envelope.command_id,
                        RejectionCode::InvalidPayload,
                        "building does not exist",
                    );
                };
                if building.cell != actor.current_cell
                    || !matches!(actor.location, ActorLocation::InCell { .. })
                {
                    return self.rejected(
                        envelope.command_id,
                        RejectionCode::InvalidState,
                        "actor cannot enter this building from the current location",
                    );
                }
                (
                    WorldEvent::PlayerEnteredBuilding {
                        event_id,
                        player_id: actor.id,
                        building_id: *building_id,
                        cell: building.cell,
                    },
                    EntityVersion(actor.version.0 + 1),
                )
            }
            AuthorityCommand::LeaveBuilding => {
                self.check_entity_version(envelope, actor.version)?;
                let ActorLocation::InBuilding { cell, building_id } = actor.location else {
                    return self.rejected(
                        envelope.command_id,
                        RejectionCode::InvalidState,
                        "actor is not inside a building",
                    );
                };
                (
                    WorldEvent::PlayerLeftBuilding {
                        event_id,
                        player_id: actor.id,
                        building_id,
                        cell,
                    },
                    EntityVersion(actor.version.0 + 1),
                )
            }
            AuthorityCommand::BoardVehicle { vehicle_id } => {
                let Some(vehicle) = self.world.vehicles.get(vehicle_id) else {
                    return self.rejected(
                        envelope.command_id,
                        RejectionCode::InvalidPayload,
                        "vehicle does not exist",
                    );
                };
                self.check_entity_version(envelope, vehicle.version)?;
                if vehicle.cell != actor.current_cell
                    || !matches!(actor.location, ActorLocation::InCell { .. })
                {
                    return self.rejected(
                        envelope.command_id,
                        RejectionCode::InvalidState,
                        "actor cannot board this vehicle from the current location",
                    );
                }
                (
                    WorldEvent::PlayerBoardedVehicle {
                        event_id,
                        player_id: actor.id,
                        vehicle_id: *vehicle_id,
                        cell: vehicle.cell,
                    },
                    EntityVersion(vehicle.version.0 + 1),
                )
            }
            AuthorityCommand::DisembarkVehicle => {
                let ActorLocation::InVehicle { cell, vehicle_id } = actor.location else {
                    return self.rejected(
                        envelope.command_id,
                        RejectionCode::InvalidState,
                        "actor is not aboard a vehicle",
                    );
                };
                let vehicle = &self.world.vehicles[&vehicle_id];
                self.check_entity_version(envelope, vehicle.version)?;
                (
                    WorldEvent::PlayerDisembarkedVehicle {
                        event_id,
                        player_id: actor.id,
                        vehicle_id,
                        cell,
                    },
                    EntityVersion(vehicle.version.0 + 1),
                )
            }
            AuthorityCommand::AddOpening {
                building_id,
                wall_id,
                opening_id,
            } => {
                let Some(building) = self.world.buildings.get(building_id) else {
                    return self.rejected(
                        envelope.command_id,
                        RejectionCode::InvalidPayload,
                        "building does not exist",
                    );
                };
                self.check_entity_version(envelope, building.version)?;
                if actor.location
                    != (ActorLocation::InBuilding {
                        cell: building.cell,
                        building_id: *building_id,
                    })
                    || !building.editors.contains(&actor.id)
                {
                    return self.rejected(
                        envelope.command_id,
                        RejectionCode::PermissionDenied,
                        "actor cannot edit this building",
                    );
                }
                if !building.wall_ids.contains(wall_id)
                    || building.openings.contains_key(opening_id)
                {
                    return self.rejected(
                        envelope.command_id,
                        RejectionCode::InvalidState,
                        "wall is invalid or opening already exists",
                    );
                }
                (
                    WorldEvent::OpeningAdded {
                        event_id,
                        building_id: *building_id,
                        wall_id: *wall_id,
                        opening_id: *opening_id,
                    },
                    EntityVersion(building.version.0 + 1),
                )
            }
            AuthorityCommand::Connect { .. }
            | AuthorityCommand::Disconnect
            | AuthorityCommand::RequestSync { .. } => {
                unreachable!("non-mutating command handled before mutation dispatch")
            }
        };

        let from_revision = self.world.revision;
        apply_event(&mut self.world, &event)?;
        self.world.revision = self.world.revision.next();
        self.world.clock = envelope.issued_at;
        let delta = WorldDelta {
            from_revision,
            to_revision: self.world.revision,
            events: vec![event],
        };
        if let Some(session) = self.sessions.get_mut(&envelope.audit.client_id) {
            session.last_acknowledged_revision = self.world.revision;
        }
        Ok(StoredReceipt {
            command_id: envelope.command_id,
            world_revision: self.world.revision,
            resulting_entity_version: Some(target_version),
            rejection: None,
            delta: Some(delta),
            sync: None,
            state_fingerprint: self.world.semantic_fingerprint()?,
        })
    }

    fn require_session(
        &self,
        envelope: &AuthorityCommandEnvelope,
    ) -> Result<&SessionState, CommandRejection> {
        let Some(session) = self.sessions.get(&envelope.audit.client_id) else {
            return Err(rejection(
                RejectionCode::PermissionDenied,
                "client is not connected",
            ));
        };
        if session.session_id != envelope.audit.session_id || session.actor_id != envelope.actor_id
        {
            return Err(rejection(
                RejectionCode::PermissionDenied,
                "session identity does not match the command actor",
            ));
        }
        Ok(session)
    }

    fn check_entity_version(
        &self,
        envelope: &AuthorityCommandEnvelope,
        actual: EntityVersion,
    ) -> Result<(), AuthorityError> {
        if let Some(expected) = envelope.expected_entity_version
            && expected != actual
        {
            return Err(AuthorityError::VersionConflict {
                expected: expected.0,
                actual: actual.0,
            });
        }
        Ok(())
    }

    fn accepted_noop(
        &self,
        command_id: CommandId,
        resulting_entity_version: Option<EntityVersion>,
        sync: Option<SyncPayload>,
    ) -> Result<StoredReceipt, AuthorityError> {
        Ok(StoredReceipt {
            command_id,
            world_revision: self.world.revision,
            resulting_entity_version,
            rejection: None,
            delta: None,
            sync,
            state_fingerprint: self.world.semantic_fingerprint()?,
        })
    }

    fn rejected(
        &self,
        command_id: CommandId,
        code: RejectionCode,
        message: impl Into<String>,
    ) -> Result<StoredReceipt, AuthorityError> {
        self.rejected_existing(command_id, rejection(code, message))
    }

    fn rejected_existing(
        &self,
        command_id: CommandId,
        rejection: CommandRejection,
    ) -> Result<StoredReceipt, AuthorityError> {
        Ok(StoredReceipt {
            command_id,
            world_revision: self.world.revision,
            resulting_entity_version: None,
            rejection: Some(rejection),
            delta: None,
            sync: None,
            state_fingerprint: self.world.semantic_fingerprint()?,
        })
    }
}

fn rejection(code: RejectionCode, message: impl Into<String>) -> CommandRejection {
    CommandRejection {
        code,
        message: message.into(),
    }
}

fn event_id(command_id: CommandId, index: u32) -> EventId {
    let mixed = command_id
        .as_u128()
        .rotate_left(29)
        .wrapping_add(0x9e37_79b9_7f4a_7c15_u128)
        ^ u128::from(index + 1);
    EventId::from_u128(mixed)
}

pub fn apply_event(world: &mut WorldState, event: &WorldEvent) -> Result<(), AuthorityError> {
    match event {
        WorldEvent::PlayerMoved {
            player_id, to_cell, ..
        } => {
            let player = world
                .players
                .get_mut(player_id)
                .ok_or_else(|| AuthorityError::Invariant("move player missing".into()))?;
            player.current_cell = *to_cell;
            player.location = ActorLocation::InCell { cell: *to_cell };
            player.version = EntityVersion(player.version.0 + 1);
        }
        WorldEvent::DoorToggled {
            door_id, is_open, ..
        } => {
            let door = world
                .doors
                .get_mut(door_id)
                .ok_or_else(|| AuthorityError::Invariant("door missing".into()))?;
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
            let item = world
                .items
                .get_mut(item_id)
                .ok_or_else(|| AuthorityError::Invariant("item missing".into()))?;
            if item.owner != *from || item.quantity != *quantity {
                return Err(AuthorityError::Invariant(
                    "item transfer source no longer matches".into(),
                ));
            }
            remove_item_reference(world, *item_id, *quantity, from)?;
            add_item_reference(world, *item_id, *quantity, to)?;
            let item = world
                .items
                .get_mut(item_id)
                .ok_or_else(|| AuthorityError::Invariant("item disappeared".into()))?;
            item.owner = to.clone();
            item.version = EntityVersion(item.version.0 + 1);
        }
        WorldEvent::PlayerEnteredBuilding {
            player_id,
            building_id,
            cell,
            ..
        } => {
            let player = world
                .players
                .get_mut(player_id)
                .ok_or_else(|| AuthorityError::Invariant("enter player missing".into()))?;
            player.location = ActorLocation::InBuilding {
                cell: *cell,
                building_id: *building_id,
            };
            player.current_cell = *cell;
            player.version = EntityVersion(player.version.0 + 1);
        }
        WorldEvent::PlayerLeftBuilding {
            player_id, cell, ..
        } => {
            let player = world
                .players
                .get_mut(player_id)
                .ok_or_else(|| AuthorityError::Invariant("leave player missing".into()))?;
            player.location = ActorLocation::InCell { cell: *cell };
            player.current_cell = *cell;
            player.version = EntityVersion(player.version.0 + 1);
        }
        WorldEvent::PlayerBoardedVehicle {
            player_id,
            vehicle_id,
            cell,
            ..
        } => {
            let player = world
                .players
                .get_mut(player_id)
                .ok_or_else(|| AuthorityError::Invariant("board player missing".into()))?;
            player.location = ActorLocation::InVehicle {
                cell: *cell,
                vehicle_id: *vehicle_id,
            };
            player.current_cell = *cell;
            player.version = EntityVersion(player.version.0 + 1);
            let vehicle = world
                .vehicles
                .get_mut(vehicle_id)
                .ok_or_else(|| AuthorityError::Invariant("vehicle missing".into()))?;
            vehicle.passengers.insert(*player_id);
            vehicle.version = EntityVersion(vehicle.version.0 + 1);
        }
        WorldEvent::PlayerDisembarkedVehicle {
            player_id,
            vehicle_id,
            cell,
            ..
        } => {
            let player = world
                .players
                .get_mut(player_id)
                .ok_or_else(|| AuthorityError::Invariant("disembark player missing".into()))?;
            player.location = ActorLocation::InCell { cell: *cell };
            player.current_cell = *cell;
            player.version = EntityVersion(player.version.0 + 1);
            let vehicle = world
                .vehicles
                .get_mut(vehicle_id)
                .ok_or_else(|| AuthorityError::Invariant("vehicle missing".into()))?;
            vehicle.passengers.remove(player_id);
            vehicle.version = EntityVersion(vehicle.version.0 + 1);
        }
        WorldEvent::OpeningAdded {
            building_id,
            wall_id,
            opening_id,
            ..
        } => {
            let building = world
                .buildings
                .get_mut(building_id)
                .ok_or_else(|| AuthorityError::Invariant("building missing".into()))?;
            building.openings.insert(*opening_id, *wall_id);
            building.version = EntityVersion(building.version.0 + 1);
        }
    }
    Ok(())
}

fn remove_item_reference(
    world: &mut WorldState,
    item_id: world_ids::ItemId,
    quantity: u32,
    owner: &ItemOwner,
) -> Result<(), AuthorityError> {
    match owner {
        ItemOwner::Container(container_id) => {
            let container = world
                .containers
                .get_mut(container_id)
                .ok_or_else(|| AuthorityError::Invariant("source container missing".into()))?;
            if !container.item_ids.remove(&item_id) {
                return Err(AuthorityError::Invariant(
                    "source container did not contain item".into(),
                ));
            }
            container.version = EntityVersion(container.version.0 + 1);
        }
        ItemOwner::Person(person_id) => {
            let player = world
                .players
                .get_mut(person_id)
                .ok_or_else(|| AuthorityError::Invariant("source player missing".into()))?;
            if player.inventory.remove(&item_id) != Some(quantity) {
                return Err(AuthorityError::Invariant(
                    "source player inventory did not contain item".into(),
                ));
            }
            player.version = EntityVersion(player.version.0 + 1);
        }
        ItemOwner::Vehicle(_) => {}
    }
    Ok(())
}

fn add_item_reference(
    world: &mut WorldState,
    item_id: world_ids::ItemId,
    quantity: u32,
    owner: &ItemOwner,
) -> Result<(), AuthorityError> {
    match owner {
        ItemOwner::Container(container_id) => {
            let container = world
                .containers
                .get_mut(container_id)
                .ok_or_else(|| AuthorityError::Invariant("target container missing".into()))?;
            container.item_ids.insert(item_id);
            container.version = EntityVersion(container.version.0 + 1);
        }
        ItemOwner::Person(person_id) => {
            let player = world
                .players
                .get_mut(person_id)
                .ok_or_else(|| AuthorityError::Invariant("target player missing".into()))?;
            player.inventory.insert(item_id, quantity);
            player.version = EntityVersion(player.version.0 + 1);
        }
        ItemOwner::Vehicle(_) => {}
    }
    Ok(())
}
