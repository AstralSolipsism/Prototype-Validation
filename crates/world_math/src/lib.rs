#![forbid(unsafe_code)]

use glam::{DQuat, DVec3, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, btree_map::Entry};
use thiserror::Error;
use world_ids::FrameId;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct RigidTransform {
    pub translation: DVec3,
    pub rotation: DQuat,
}

impl RigidTransform {
    pub const IDENTITY: Self = Self {
        translation: DVec3::ZERO,
        rotation: DQuat::IDENTITY,
    };

    pub fn new(translation: DVec3, rotation: DQuat) -> Result<Self, TransformError> {
        if !translation.is_finite() || !rotation.is_finite() {
            return Err(TransformError::NonFinite);
        }
        let length = rotation.length();
        if (length - 1.0).abs() > 1.0e-9 {
            return Err(TransformError::NonUnitQuaternion(length));
        }
        Ok(Self {
            translation,
            rotation,
        })
    }

    pub fn transform_point(self, point: DVec3) -> DVec3 {
        self.rotation * point + self.translation
    }

    pub fn compose(self, local: Self) -> Self {
        Self {
            translation: self.transform_point(local.translation),
            rotation: (self.rotation * local.rotation).normalize(),
        }
    }

    pub fn inverse(self) -> Self {
        let inverse_rotation = self.rotation.conjugate();
        Self {
            translation: inverse_rotation * -self.translation,
            rotation: inverse_rotation,
        }
    }

    pub fn relative_to(self, reference: Self) -> Self {
        reference.inverse().compose(self)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraRelativePose {
    pub translation: Vec3,
    pub rotation: DQuat,
}

impl CameraRelativePose {
    pub fn from_world(object_world: RigidTransform, camera_world: RigidTransform) -> Self {
        let relative = object_world.relative_to(camera_world);
        Self {
            translation: relative.translation.as_vec3(),
            rotation: relative.rotation,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReferenceFrameNode {
    pub parent: Option<FrameId>,
    pub pose_in_parent: RigidTransform,
}

#[derive(Clone, Debug, Default)]
pub struct ReferenceFrameGraph {
    nodes: BTreeMap<FrameId, ReferenceFrameNode>,
}

impl ReferenceFrameGraph {
    pub fn insert_root(&mut self, id: FrameId) -> Result<(), FrameGraphError> {
        self.insert(
            id,
            ReferenceFrameNode {
                parent: None,
                pose_in_parent: RigidTransform::IDENTITY,
            },
        )
    }

    pub fn insert_child(
        &mut self,
        id: FrameId,
        parent: FrameId,
        pose_in_parent: RigidTransform,
    ) -> Result<(), FrameGraphError> {
        if !self.nodes.contains_key(&parent) {
            return Err(FrameGraphError::MissingParent(parent));
        }
        self.insert(
            id,
            ReferenceFrameNode {
                parent: Some(parent),
                pose_in_parent,
            },
        )
    }

    fn insert(&mut self, id: FrameId, node: ReferenceFrameNode) -> Result<(), FrameGraphError> {
        match self.nodes.entry(id) {
            Entry::Vacant(entry) => {
                entry.insert(node);
                Ok(())
            }
            Entry::Occupied(_) => Err(FrameGraphError::DuplicateFrame(id)),
        }
    }

    pub fn world_transform(&self, id: FrameId) -> Result<RigidTransform, FrameGraphError> {
        let mut chain = Vec::new();
        let mut visited = BTreeSet::new();
        let mut current = id;

        loop {
            if !visited.insert(current) {
                return Err(FrameGraphError::Cycle(current));
            }
            let node = self
                .nodes
                .get(&current)
                .ok_or(FrameGraphError::UnknownFrame(current))?;
            chain.push(node.pose_in_parent);
            match node.parent {
                Some(parent) => current = parent,
                None => break,
            }
        }

        Ok(chain
            .into_iter()
            .rev()
            .fold(RigidTransform::IDENTITY, RigidTransform::compose))
    }
}

#[derive(Clone, Copy, Debug, Error, PartialEq)]
pub enum TransformError {
    #[error("transform contains a non-finite value")]
    NonFinite,
    #[error("rotation quaternion must be normalized, length was {0}")]
    NonUnitQuaternion(f64),
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum FrameGraphError {
    #[error("frame already exists: {0}")]
    DuplicateFrame(FrameId),
    #[error("parent frame does not exist: {0}")]
    MissingParent(FrameId),
    #[error("frame does not exist: {0}")]
    UnknownFrame(FrameId),
    #[error("reference-frame cycle detected at {0}")]
    Cycle(FrameId),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_near(left: DVec3, right: DVec3, tolerance: f64) {
        let distance = left.distance(right);
        assert!(
            distance <= tolerance,
            "left={left:?}, right={right:?}, distance={distance}"
        );
    }

    #[test]
    fn rigid_transform_round_trip_is_stable() {
        let transform = RigidTransform::new(
            DVec3::new(10.0, 2.0, -4.0),
            DQuat::from_rotation_y(0.7),
        )
        .expect("valid transform");
        let point = DVec3::new(3.0, 5.0, 7.0);
        assert_near(
            transform.inverse().transform_point(transform.transform_point(point)),
            point,
            1.0e-10,
        );
    }

    #[test]
    fn nested_vehicle_frame_resolves_to_world() {
        let world = FrameId::from_u128(1);
        let ship = FrameId::from_u128(2);
        let cabin = FrameId::from_u128(3);
        let mut graph = ReferenceFrameGraph::default();
        graph.insert_root(world).expect("world frame");
        graph
            .insert_child(
                ship,
                world,
                RigidTransform::new(
                    DVec3::new(1_000_000.0, 20.0, -2_000_000.0),
                    DQuat::from_rotation_y(0.5),
                )
                .expect("ship transform"),
            )
            .expect("ship frame");
        graph
            .insert_child(
                cabin,
                ship,
                RigidTransform::new(DVec3::new(5.0, 2.0, 1.0), DQuat::IDENTITY)
                    .expect("cabin transform"),
            )
            .expect("cabin frame");

        let cabin_world = graph.world_transform(cabin).expect("resolve cabin");
        let ship_world = graph.world_transform(ship).expect("resolve ship");
        assert_near(
            cabin_world.translation,
            ship_world.transform_point(DVec3::new(5.0, 2.0, 1.0)),
            1.0e-9,
        );
    }

    #[test]
    fn camera_relative_conversion_preserves_small_offsets_at_large_coordinates() {
        let camera = RigidTransform::new(
            DVec3::new(9_000_000.0, 500.0, -7_000_000.0),
            DQuat::IDENTITY,
        )
        .expect("camera");
        let object = RigidTransform::new(
            camera.translation + DVec3::new(0.0125, 1.5, -3.25),
            DQuat::IDENTITY,
        )
        .expect("object");

        let relative = CameraRelativePose::from_world(object, camera);
        assert!(relative.translation.distance(Vec3::new(0.0125, 1.5, -3.25)) < 1.0e-5);
    }

    #[test]
    fn duplicate_insert_does_not_replace_existing_frame() {
        let world = FrameId::from_u128(1);
        let mut graph = ReferenceFrameGraph::default();
        graph.insert_root(world).expect("initial root");
        assert_eq!(
            graph.insert_root(world),
            Err(FrameGraphError::DuplicateFrame(world))
        );
        assert_eq!(
            graph.world_transform(world).expect("original root"),
            RigidTransform::IDENTITY
        );
    }
}
