#![forbid(unsafe_code)]

use glam::DVec3;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use world_ids::FrameId;
use world_math::{FrameGraphError, ReferenceFrameGraph, RigidTransform};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct FrameBoundPose {
    pub frame: FrameId,
    pub local_pose: RigidTransform,
}

impl FrameBoundPose {
    pub const fn new(frame: FrameId, local_pose: RigidTransform) -> Self {
        Self { frame, local_pose }
    }

    pub fn world_pose(
        self,
        frames: &ReferenceFrameGraph,
    ) -> Result<RigidTransform, MobileRegionError> {
        Ok(frames.world_transform(self.frame)?.compose(self.local_pose))
    }

    pub fn expressed_in(
        self,
        target_frame: FrameId,
        frames: &ReferenceFrameGraph,
    ) -> Result<Self, MobileRegionError> {
        let world_pose = self.world_pose(frames)?;
        let target_world = frames.world_transform(target_frame)?;
        Ok(Self {
            frame: target_frame,
            local_pose: world_pose.relative_to(target_world),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct FrameTransferResult {
    pub before: FrameBoundPose,
    pub after: FrameBoundPose,
    pub translation_error_meters: f64,
    pub rotation_error_radians: f64,
}

pub fn transfer_pose(
    pose: FrameBoundPose,
    target_frame: FrameId,
    frames: &ReferenceFrameGraph,
) -> Result<FrameTransferResult, MobileRegionError> {
    let world_before = pose.world_pose(frames)?;
    let after = pose.expressed_in(target_frame, frames)?;
    let world_after = after.world_pose(frames)?;
    let rotation_dot = world_before
        .rotation
        .dot(world_after.rotation)
        .abs()
        .clamp(0.0, 1.0);

    Ok(FrameTransferResult {
        before: pose,
        after,
        translation_error_meters: world_before.translation.distance(world_after.translation),
        rotation_error_radians: 2.0 * rotation_dot.acos(),
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct VehiclePoseState {
    pub authoritative_in_parent: RigidTransform,
    pub presentation_offset: RigidTransform,
}

impl VehiclePoseState {
    pub const fn new(authoritative_in_parent: RigidTransform) -> Self {
        Self {
            authoritative_in_parent,
            presentation_offset: RigidTransform::IDENTITY,
        }
    }

    pub const fn with_presentation_offset(mut self, offset: RigidTransform) -> Self {
        self.presentation_offset = offset;
        self
    }

    pub fn presentation_in_parent(self) -> RigidTransform {
        self.authoritative_in_parent
            .compose(self.presentation_offset)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct DockingLink {
    pub static_anchor: FrameBoundPose,
    pub mobile_anchor: FrameBoundPose,
}

impl DockingLink {
    pub fn separation_meters(
        self,
        frames: &ReferenceFrameGraph,
    ) -> Result<f64, MobileRegionError> {
        let static_world = self.static_anchor.world_pose(frames)?;
        let mobile_world = self.mobile_anchor.world_pose(frames)?;
        Ok(static_world.translation.distance(mobile_world.translation))
    }
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum MobileRegionError {
    #[error(transparent)]
    FrameGraph(#[from] FrameGraphError),
}

pub fn relative_distance_in_world(
    left: FrameBoundPose,
    right: FrameBoundPose,
    frames: &ReferenceFrameGraph,
) -> Result<f64, MobileRegionError> {
    let left_world = left.world_pose(frames)?;
    let right_world = right.world_pose(frames)?;
    Ok(left_world.translation.distance(right_world.translation))
}

pub fn local_point(frame: FrameId, position: DVec3) -> FrameBoundPose {
    FrameBoundPose::new(
        frame,
        RigidTransform {
            translation: position,
            rotation: glam::DQuat::IDENTITY,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{DQuat, DVec3};

    fn frames() -> (ReferenceFrameGraph, FrameId, FrameId) {
        let world = FrameId::from_u128(1);
        let vehicle = FrameId::from_u128(2);
        let mut frames = ReferenceFrameGraph::default();
        frames.insert_root(world).expect("world frame");
        frames
            .insert_child(
                vehicle,
                world,
                RigidTransform::new(
                    DVec3::new(5_000_000.0, 12.0, -4_000_000.0),
                    DQuat::from_rotation_y(0.7),
                )
                .expect("vehicle pose"),
            )
            .expect("vehicle frame");
        (frames, world, vehicle)
    }

    #[test]
    fn frame_transfer_preserves_world_pose() {
        let (frames, world, vehicle) = frames();
        let aboard = FrameBoundPose::new(
            vehicle,
            RigidTransform::new(DVec3::new(3.0, 2.0, -1.0), DQuat::from_rotation_y(0.2))
                .expect("local pose"),
        );

        let disembarked = transfer_pose(aboard, world, &frames).expect("transfer");
        assert!(disembarked.translation_error_meters < 1.0e-9);
        assert!(disembarked.rotation_error_radians < 1.0e-9);

        let boarded =
            transfer_pose(disembarked.after, vehicle, &frames).expect("reverse transfer");
        assert!(boarded.translation_error_meters < 1.0e-9);
        assert!(boarded.rotation_error_radians < 1.0e-9);
        assert!(
            boarded
                .after
                .local_pose
                .translation
                .distance(aboard.local_pose.translation)
                < 1.0e-9
        );
    }

    #[test]
    fn moving_vehicle_preserves_interior_separation() {
        let (mut frames, _world, vehicle) = frames();
        let left = local_point(vehicle, DVec3::new(-4.0, 1.0, 2.0));
        let right = local_point(vehicle, DVec3::new(7.0, 3.0, -5.0));
        let expected = left
            .local_pose
            .translation
            .distance(right.local_pose.translation);

        for step in 0..120 {
            let t = step as f64 * 0.1;
            frames
                .set_pose_in_parent(
                    vehicle,
                    RigidTransform::new(
                        DVec3::new(
                            5_000_000.0 + t.cos() * 100.0,
                            12.0,
                            -4_000_000.0 + t.sin() * 100.0,
                        ),
                        DQuat::from_rotation_y(t),
                    )
                    .expect("moving pose"),
                )
                .expect("update vehicle");
            let actual = relative_distance_in_world(left, right, &frames).expect("distance");
            assert!((actual - expected).abs() < 1.0e-8);
        }
    }

    #[test]
    fn presentation_sway_does_not_mutate_authoritative_pose() {
        let authoritative = RigidTransform::new(
            DVec3::new(100.0, 4.0, 200.0),
            DQuat::from_rotation_y(1.0),
        )
        .expect("authority");
        let offset = RigidTransform::new(
            DVec3::new(0.0, 0.2, 0.0),
            DQuat::from_rotation_z(0.03),
        )
        .expect("sway");
        let state = VehiclePoseState::new(authoritative).with_presentation_offset(offset);

        assert_eq!(state.authoritative_in_parent, authoritative);
        assert_ne!(state.presentation_in_parent(), authoritative);
    }

    #[test]
    fn docking_link_uses_true_world_positions() {
        let (frames, world, vehicle) = frames();
        let vehicle_anchor = local_point(vehicle, DVec3::new(8.0, 0.0, 0.0));
        let static_anchor = vehicle_anchor
            .expressed_in(world, &frames)
            .expect("world anchor");
        let link = DockingLink {
            static_anchor,
            mobile_anchor: vehicle_anchor,
        };
        assert!(link.separation_meters(&frames).expect("separation") < 1.0e-9);
    }
}
