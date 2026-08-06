#![forbid(unsafe_code)]

use glam::{DMat3, DQuat, DVec3, Vec3};
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

    pub fn look_at(
        translation: DVec3,
        target: DVec3,
        up: DVec3,
    ) -> Result<Self, TransformError> {
        if !translation.is_finite() || !target.is_finite() || !up.is_finite() {
            return Err(TransformError::NonFinite);
        }

        let forward = target - translation;
        if forward.length_squared() <= 1.0e-18 || up.length_squared() <= 1.0e-18 {
            return Err(TransformError::DegenerateLookAt);
        }
        let forward = forward.normalize();
        let right = forward.cross(up);
        if right.length_squared() <= 1.0e-18 {
            return Err(TransformError::DegenerateLookAt);
        }
        let right = right.normalize();
        let corrected_up = right.cross(forward).normalize();
        let basis = DMat3::from_cols(right, corrected_up, -forward);
        Self::new(translation, DQuat::from_mat3(&basis).normalize())
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

    pub fn node(&self, id: FrameId) -> Option<ReferenceFrameNode> {
        self.nodes.get(&id).copied()
    }

    pub fn set_pose_in_parent(
        &mut self,
        id: FrameId,
        pose_in_parent: RigidTransform,
    ) -> Result<(), FrameGraphError> {
        let node = self
            .nodes
            .get_mut(&id)
            .ok_or(FrameGraphError::UnknownFrame(id))?;
        node.pose_in_parent = pose_in_parent;
        Ok(())
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

    pub fn transform_between(
        &self,
        source: FrameId,
        target: FrameId,
    ) -> Result<RigidTransform, FrameGraphError> {
        let source_world = self.world_transform(source)?;
        let target_world = self.world_transform(target)?;
        Ok(source_world.relative_to(target_world))
    }
}

#[derive(Clone, Copy, Debug, Error, PartialEq)]
pub enum TransformError {
    #[error("transform contains a non-finite value")]
    NonFinite,
    #[error("rotation quaternion must be normalized, length was {0}")]
    NonUnitQuaternion(f64),
    #[error("look-at transform requires distinct position/target and a non-parallel up vector")]
    DegenerateLookAt,
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
        let transform =
            RigidTransform::new(DVec3::new(10.0, 2.0, -4.0), DQuat::from_rotation_y(0.7))
                .expect("valid transform");
        let point = DVec3::new(3.0, 5.0, 7.0);
        assert_near(
            transform
                .inverse()
                .transform_point(transform.transform_point(point)),
            point,
            1.0e-10,
        );
    }

    #[test]
    fn look_at_uses_negative_local_z_as_forward() {
        let camera = RigidTransform::look_at(DVec3::ZERO, -DVec3::Z, DVec3::Y)
            .expect("valid camera");
        assert_near(camera.rotation * -DVec3::Z, -DVec3::Z, 1.0e-12);
        assert_near(camera.rotation * DVec3::Y, DVec3::Y, 1.0e-12);
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
    fn moving_frame_pose_can_be_updated_without_rebuilding_graph() {
        let world = FrameId::from_u128(1);
        let ship = FrameId::from_u128(2);
        let mut graph = ReferenceFrameGraph::default();
        graph.insert_root(world).expect("world frame");
        graph
            .insert_child(ship, world, RigidTransform::IDENTITY)
            .expect("ship frame");

        let pose = RigidTransform::new(
            DVec3::new(3_000_000.0, 40.0, -8_000_000.0),
            DQuat::from_rotation_y(1.2),
        )
        .expect("ship pose");
        graph
            .set_pose_in_parent(ship, pose)
            .expect("update ship pose");
        assert_eq!(graph.world_transform(ship).expect("ship world"), pose);
        assert_eq!(graph.node(ship).expect("ship node").parent, Some(world));
    }

    #[test]
    fn transform_between_frames_maps_source_local_to_target_local() {
        let world = FrameId::from_u128(1);
        let ship = FrameId::from_u128(2);
        let mut graph = ReferenceFrameGraph::default();
        graph.insert_root(world).expect("world frame");
        graph
            .insert_child(
                ship,
                world,
                RigidTransform::new(
                    DVec3::new(100.0, 0.0, 50.0),
                    DQuat::from_rotation_y(0.5),
                )
                .expect("ship pose"),
            )
            .expect("ship frame");

        let ship_to_world = graph.transform_between(ship, world).expect("mapping");
        assert_eq!(ship_to_world, graph.world_transform(ship).expect("ship world"));
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
