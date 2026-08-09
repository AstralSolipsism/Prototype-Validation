#![forbid(unsafe_code)]

use authoritative_world_core::{
    AuthorityCommand, ClientProjection, CommandReceipt, ItemOwner, ReceiptDisposition, SyncPayload,
    WorldRevision,
};
use authority_persistence::FilePersistenceStore;
use authority_transport::{
    RunningAuthorityServer, TcpAuthorityClient, TransportError, WireRequest, WireResponse,
};
use p5_authority_scenario::{
    P5Scenario, ScenarioCommandFactory, ScenarioError, generate_p5_scenario,
};
use protocol::{EntityVersion, RejectionCode};
use replay_core::StateFingerprint;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};
use thiserror::Error;
use world_ids::{OpeningId, SessionId, SnapshotId};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationStep {
    pub index: usize,
    pub title: String,
    pub detail: String,
    pub passed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct P5ValidationReport {
    pub schema_version: u32,
    pub checks: Vec<ValidationStep>,
    pub final_revision: WorldRevision,
    pub pre_crash_fingerprint: StateFingerprint,
    pub recovered_fingerprint: StateFingerprint,
    pub journal_records: usize,
    pub duplicate_receipts: usize,
    pub rejected_receipts: usize,
    pub buffered_out_of_order_commands: usize,
    pub snapshot_file: String,
    pub journal_file: String,
    pub all_passed: bool,
}

pub fn run_validation(
    root: impl AsRef<Path>,
    mut observe: impl FnMut(&ValidationStep),
) -> Result<P5ValidationReport, HarnessError> {
    let scenario = generate_p5_scenario()?;
    let root = root.as_ref().to_path_buf();
    let store = FilePersistenceStore::new(&root);
    store.reset()?;
    let mut checks = Vec::new();
    let mut duplicate_receipts = 0usize;
    let mut rejected_receipts = 0usize;
    let mut buffered_count = 0usize;

    let server = RunningAuthorityServer::start(
        "127.0.0.1:0",
        scenario.baseline.clone(),
        store.clone(),
        false,
    )?;
    let address = server.address();
    let mut client_a = TcpAuthorityClient::connect(address)?;
    let mut client_b = TcpAuthorityClient::connect(address)?;
    let mut factory_a = ScenarioCommandFactory::new(
        scenario.ids.player_a,
        scenario.ids.client_a,
        scenario.ids.session_a,
        scenario.ids.world_id,
        0x5100_0000_0000_0000,
    );
    let mut factory_b = ScenarioCommandFactory::new(
        scenario.ids.player_b,
        scenario.ids.client_b,
        scenario.ids.session_b,
        scenario.ids.world_id,
        0x5200_0000_0000_0000,
    );

    let connect_a = factory_a.envelope(
        AuthorityCommand::Connect {
            last_acknowledged_revision: WorldRevision::ZERO,
            interest_center: scenario.ids.start_cell,
        },
        None,
        None,
        "A connect",
    );
    let connect_b = factory_b.envelope(
        AuthorityCommand::Connect {
            last_acknowledged_revision: WorldRevision::ZERO,
            interest_center: scenario.ids.start_cell,
        },
        None,
        None,
        "B connect",
    );
    let mut projection_a = projection_from_receipt(single_receipt(client_a.command(connect_a)?)?)?;
    let mut projection_b = projection_from_receipt(single_receipt(client_b.command(connect_b)?)?)?;
    record(
        &mut checks,
        &mut observe,
        "two clients connect",
        format!(
            "A and B received revision {} snapshots from one TCP authority server",
            projection_a.revision.0
        ),
        projection_a.revision == WorldRevision::ZERO
            && projection_b.revision == WorldRevision::ZERO,
    );

    let toggle = factory_a.envelope(
        AuthorityCommand::ToggleDoor {
            door_id: scenario.ids.door_id,
        },
        Some(WorldRevision(0)),
        Some(EntityVersion(0)),
        "A opens warehouse door",
    );
    let toggle_receipt = single_receipt(client_a.command(toggle.clone())?)?;
    projection_a.apply_receipt(&toggle_receipt)?;
    sync_projection(
        &mut client_b,
        &mut factory_b,
        &mut projection_b,
        scenario.ids.start_cell,
        "B sync after door",
    )?;
    record(
        &mut checks,
        &mut observe,
        "authoritative door replication",
        "A opened the door; B synchronized the same versioned door state".into(),
        projection_a.doors[&scenario.ids.door_id].is_open
            && projection_b.doors[&scenario.ids.door_id].is_open,
    );

    let take = factory_a.envelope(
        AuthorityCommand::TakeItem {
            item_id: scenario.ids.item_id,
            quantity: 1,
        },
        Some(WorldRevision(1)),
        Some(EntityVersion(0)),
        "A takes sealed ledger",
    );
    let take_receipt = single_receipt(client_a.command(take.clone())?)?;
    projection_a.apply_receipt(&take_receipt)?;
    sync_projection(
        &mut client_b,
        &mut factory_b,
        &mut projection_b,
        scenario.ids.start_cell,
        "B sync after item",
    )?;
    record(
        &mut checks,
        &mut observe,
        "authoritative item ownership",
        "The item moved from the world container to A in both client projections".into(),
        projection_a.items[&scenario.ids.item_id].owner == ItemOwner::Person(scenario.ids.player_a)
            && projection_b.items[&scenario.ids.item_id].owner
                == ItemOwner::Person(scenario.ids.player_a),
    );

    let duplicate = single_receipt(client_a.command(take.clone())?)?;
    if duplicate.disposition == ReceiptDisposition::Duplicate {
        duplicate_receipts += 1;
    }
    record(
        &mut checks,
        &mut observe,
        "idempotent duplicate command",
        "The same CommandId returned the original result without transferring the item twice"
            .into(),
        duplicate.disposition == ReceiptDisposition::Duplicate
            && projection_a.players[&scenario.ids.player_a]
                .inventory
                .get(&scenario.ids.item_id)
                == Some(&1),
    );

    let move_command = factory_a.envelope(
        AuthorityCommand::MoveAlongRoute {
            route_id: scenario.ids.route_id,
            destination: scenario.ids.destination_cell,
        },
        Some(WorldRevision(2)),
        Some(EntityVersion(1)),
        "A travels across AtlasCells",
    );
    let enter_command = factory_a.envelope(
        AuthorityCommand::EnterBuilding {
            building_id: scenario.ids.destination_building,
        },
        Some(WorldRevision(3)),
        Some(EntityVersion(2)),
        "A enters destination building",
    );
    let buffered = client_a.request(&WireRequest::Command(enter_command.clone()))?;
    if matches!(&buffered, WireResponse::Buffered { .. }) {
        buffered_count += 1;
    }
    let flushed = client_a.request(&WireRequest::Command(move_command))?;
    let receipts = expect_receipts(flushed)?;
    for receipt in &receipts {
        projection_a.apply_receipt(receipt)?;
    }
    sync_projection(
        &mut client_b,
        &mut factory_b,
        &mut projection_b,
        scenario.ids.destination_cell,
        "B sync after cross-cell travel",
    )?;
    record(
        &mut checks,
        &mut observe,
        "out-of-order buffering and cross-cell travel",
        "The future enter-building request waited for the missing move request, then both committed in sequence"
            .into(),
        matches!(&buffered, WireResponse::Buffered { .. })
            && receipts.len() == 2
            && projection_a.players[&scenario.ids.player_a].current_cell
                == scenario.ids.destination_cell
            && projection_a.players[&scenario.ids.player_a].location
                == authoritative_world_core::ActorLocation::InBuilding {
                    cell: scenario.ids.destination_cell,
                    building_id: scenario.ids.destination_building,
                },
    );

    let add_opening = factory_a.envelope(
        AuthorityCommand::AddOpening {
            building_id: scenario.ids.destination_building,
            wall_id: scenario.ids.wall_id,
            opening_id: scenario.ids.opening_id,
        },
        Some(WorldRevision(4)),
        Some(EntityVersion(0)),
        "A adds a validated opening",
    );
    let opening_receipt = single_receipt(client_a.command(add_opening.clone())?)?;
    projection_a.apply_receipt(&opening_receipt)?;
    let duplicate_opening = single_receipt(client_a.command(add_opening.clone())?)?;
    if duplicate_opening.disposition == ReceiptDisposition::Duplicate {
        duplicate_receipts += 1;
    }
    sync_projection(
        &mut client_b,
        &mut factory_b,
        &mut projection_b,
        scenario.ids.destination_cell,
        "B sync after building edit",
    )?;
    record(
        &mut checks,
        &mut observe,
        "building edit and duplicate suppression",
        "One opening exists in both clients after the edit and exact retry".into(),
        projection_a.buildings[&scenario.ids.destination_building]
            .openings
            .len()
            == 1
            && projection_b.buildings[&scenario.ids.destination_building]
                .openings
                .len()
                == 1
            && duplicate_opening.disposition == ReceiptDisposition::Duplicate,
    );

    let stale_opening = factory_a.envelope(
        AuthorityCommand::AddOpening {
            building_id: scenario.ids.destination_building,
            wall_id: scenario.ids.wall_id,
            opening_id: OpeningId::from_u128(0x5000_0f12),
        },
        Some(WorldRevision(5)),
        Some(EntityVersion(0)),
        "stale building version",
    );
    let stale_receipt = single_receipt(client_a.command(stale_opening)?)?;
    if stale_receipt.disposition == ReceiptDisposition::Rejected {
        rejected_receipts += 1;
    }
    record(
        &mut checks,
        &mut observe,
        "stale version rejection",
        "A stale entity version was rejected without changing the world revision".into(),
        stale_receipt.disposition == ReceiptDisposition::Rejected
            && stale_receipt
                .rejection
                .as_ref()
                .is_some_and(|rejection| rejection.code == RejectionCode::VersionConflict)
            && stale_receipt.world_revision == WorldRevision(5),
    );

    let unauthorized = factory_b.envelope(
        AuthorityCommand::AddOpening {
            building_id: scenario.ids.destination_building,
            wall_id: scenario.ids.wall_id,
            opening_id: OpeningId::from_u128(0x5000_0f13),
        },
        Some(WorldRevision(5)),
        None,
        "B unauthorized building edit",
    );
    let unauthorized_receipt = single_receipt(client_b.command(unauthorized)?)?;
    if unauthorized_receipt.disposition == ReceiptDisposition::Rejected {
        rejected_receipts += 1;
    }
    record(
        &mut checks,
        &mut observe,
        "permission rejection",
        "B could not modify a building without location and editor authority".into(),
        unauthorized_receipt.disposition == ReceiptDisposition::Rejected
            && unauthorized_receipt
                .rejection
                .as_ref()
                .is_some_and(|rejection| rejection.code == RejectionCode::PermissionDenied),
    );

    let snapshot_response = client_b.request(&WireRequest::SnapshotNow {
        snapshot_id: SnapshotId::from_u128(0x5000_5001),
    })?;
    record(
        &mut checks,
        &mut observe,
        "region snapshot checkpoint",
        format!("Checkpoint written at world revision 5: {snapshot_response:?}"),
        matches!(
            snapshot_response,
            WireResponse::SnapshotWritten {
                revision: WorldRevision(5),
                ..
            }
        ),
    );

    let disconnect = factory_a.envelope(AuthorityCommand::Disconnect, None, None, "A disconnects");
    let _ = client_a.command(disconnect)?;
    drop(client_a);

    let post_snapshot_toggle = factory_b.envelope(
        AuthorityCommand::ToggleDoor {
            door_id: scenario.ids.door_id,
        },
        Some(WorldRevision(5)),
        Some(EntityVersion(1)),
        "B closes door after snapshot",
    );
    let post_snapshot_receipt = single_receipt(client_b.command(post_snapshot_toggle)?)?;
    projection_b.apply_receipt(&post_snapshot_receipt)?;

    let mut reconnect_a = TcpAuthorityClient::connect(address)?;
    let mut reconnect_factory_a = ScenarioCommandFactory::new(
        scenario.ids.player_a,
        scenario.ids.client_a,
        scenario.ids.reconnect_session_a,
        scenario.ids.world_id,
        0x5300_0000_0000_0000,
    );
    let reconnect_receipt = single_receipt(reconnect_a.command(reconnect_factory_a.envelope(
        AuthorityCommand::Connect {
            last_acknowledged_revision: projection_a.revision,
            interest_center: scenario.ids.destination_cell,
        },
        None,
        None,
        "A reconnects before crash",
    ))?)?;
    projection_a = projection_from_receipt(reconnect_receipt)?;
    record(
        &mut checks,
        &mut observe,
        "disconnect and reconnect",
        "A reconnected from its last acknowledged revision and received the post-snapshot door change"
            .into(),
        projection_a.revision == WorldRevision(6)
            && !projection_a.doors[&scenario.ids.door_id].is_open,
    );

    let pre_crash = server.summary()?;
    let _ = server.stop()?;
    drop(client_b);
    drop(reconnect_a);

    let recovered_server = RunningAuthorityServer::start(
        "127.0.0.1:0",
        scenario.baseline.clone(),
        store.clone(),
        true,
    )?;
    let recovered_summary = recovered_server.summary()?;
    record(
        &mut checks,
        &mut observe,
        "snapshot plus journal recovery",
        "A fresh server process recovered the checkpoint and replayed post-snapshot journal records"
            .into(),
        recovered_summary.revision == pre_crash.revision
            && recovered_summary.state_fingerprint == pre_crash.state_fingerprint,
    );

    let recovered_address = recovered_server.address();
    let mut recovered_a = TcpAuthorityClient::connect(recovered_address)?;
    let mut recovered_b = TcpAuthorityClient::connect(recovered_address)?;
    let mut recovered_factory_a = ScenarioCommandFactory::new(
        scenario.ids.player_a,
        scenario.ids.client_a,
        SessionId::from_u128(0x5000_25a1),
        scenario.ids.world_id,
        0x5400_0000_0000_0000,
    );
    let mut recovered_factory_b = ScenarioCommandFactory::new(
        scenario.ids.player_b,
        scenario.ids.client_b,
        SessionId::from_u128(0x5000_25b2),
        scenario.ids.world_id,
        0x5500_0000_0000_0000,
    );
    let recovered_receipt_a =
        single_receipt(recovered_a.command(recovered_factory_a.envelope(
            AuthorityCommand::Connect {
                last_acknowledged_revision: WorldRevision::ZERO,
                interest_center: scenario.ids.destination_cell,
            },
            None,
            None,
            "A connects after recovery",
        ))?)?;
    let recovered_receipt_b =
        single_receipt(recovered_b.command(recovered_factory_b.envelope(
            AuthorityCommand::Connect {
                last_acknowledged_revision: WorldRevision::ZERO,
                interest_center: scenario.ids.start_cell,
            },
            None,
            None,
            "B connects after recovery",
        ))?)?;
    let mut recovered_projection_a = projection_from_receipt(recovered_receipt_a)?;
    let mut recovered_projection_b = projection_from_receipt(recovered_receipt_b)?;
    record(
        &mut checks,
        &mut observe,
        "two-client recovery convergence",
        "Both newly connected clients received the same recovered semantic state".into(),
        recovered_projection_a.state_fingerprint == pre_crash.state_fingerprint
            && recovered_projection_b.state_fingerprint == pre_crash.state_fingerprint
            && recovered_projection_a.revision == recovered_projection_b.revision,
    );

    recovered_projection_a.clear_visual_cache();
    recovered_projection_b.clear_visual_cache();
    recovered_projection_a.rebuild_visual_cache();
    recovered_projection_b.rebuild_visual_cache();
    record(
        &mut checks,
        &mut observe,
        "client visual cache reconstruction",
        "Deleting client-only cache did not change authoritative facts and both caches rebuilt from snapshots"
            .into(),
        recovered_projection_a.visual_cache_is_current()
            && recovered_projection_b.visual_cache_is_current(),
    );

    let final_state_valid = recovered_projection_a.items[&scenario.ids.item_id].owner
        == ItemOwner::Person(scenario.ids.player_a)
        && recovered_projection_a.buildings[&scenario.ids.destination_building]
            .openings
            .contains_key(&scenario.ids.opening_id)
        && recovered_projection_a.players[&scenario.ids.player_a].current_cell
            == scenario.ids.destination_cell
        && !recovered_projection_a.doors[&scenario.ids.door_id].is_open;
    record(
        &mut checks,
        &mut observe,
        "final authoritative facts",
        "Item ownership, player region, building edit and post-snapshot door state all survived recovery"
            .into(),
        final_state_valid,
    );

    let final_summary = recovered_server.summary()?;
    let _ = recovered_server.stop()?;
    let journal_records = store.load_journal()?.len();
    let all_passed = checks.iter().all(|check| check.passed);
    let report = P5ValidationReport {
        schema_version: 1,
        checks,
        final_revision: final_summary.revision,
        pre_crash_fingerprint: pre_crash.state_fingerprint,
        recovered_fingerprint: recovered_summary.state_fingerprint,
        journal_records,
        duplicate_receipts,
        rejected_receipts,
        buffered_out_of_order_commands: buffered_count,
        snapshot_file: store.snapshot_path().display().to_string(),
        journal_file: store.journal_path().display().to_string(),
        all_passed,
    };
    fs::write(
        root.join("p5-validation-report.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    Ok(report)
}

fn projection_from_receipt(receipt: CommandReceipt) -> Result<ClientProjection, HarnessError> {
    let Some(SyncPayload::Snapshot(snapshot)) = receipt.sync else {
        return Err(HarnessError::ExpectedSnapshot);
    };
    Ok(ClientProjection::from_snapshot(snapshot))
}

fn single_receipt(mut receipts: Vec<CommandReceipt>) -> Result<CommandReceipt, HarnessError> {
    if receipts.len() != 1 {
        return Err(HarnessError::ReceiptCount(receipts.len()));
    }
    Ok(receipts.remove(0))
}

fn expect_receipts(response: WireResponse) -> Result<Vec<CommandReceipt>, HarnessError> {
    match response {
        WireResponse::Receipts(receipts) => Ok(receipts),
        other => Err(HarnessError::UnexpectedResponse(format!("{other:?}"))),
    }
}

fn sync_projection(
    client: &mut TcpAuthorityClient,
    factory: &mut ScenarioCommandFactory,
    projection: &mut ClientProjection,
    interest_center: world_generation_core::HexCoord,
    label: &str,
) -> Result<(), HarnessError> {
    let envelope = factory.envelope(
        AuthorityCommand::RequestSync {
            last_seen_revision: projection.revision,
            interest_center,
        },
        None,
        None,
        label,
    );
    let receipt = single_receipt(client.command(envelope)?)?;
    projection.apply_receipt(&receipt)?;
    Ok(())
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
        title: title.into(),
        detail,
        passed,
    };
    observe(&step);
    checks.push(step);
}

#[derive(Debug, Error)]
pub enum HarnessError {
    #[error("P5 scenario failed: {0}")]
    Scenario(#[from] ScenarioError),
    #[error("transport failed: {0}")]
    Transport(#[from] TransportError),
    #[error("authority projection failed: {0}")]
    Authority(#[from] authoritative_world_core::AuthorityError),
    #[error("persistence failed: {0}")]
    Persistence(#[from] authority_persistence::PersistenceError),
    #[error("JSON report failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("report file failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("connect receipt did not contain a full snapshot")]
    ExpectedSnapshot,
    #[error("expected one receipt, received {0}")]
    ReceiptCount(usize),
    #[error("unexpected wire response: {0}")]
    UnexpectedResponse(String),
}
