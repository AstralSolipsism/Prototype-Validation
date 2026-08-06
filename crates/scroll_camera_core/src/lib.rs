#![forbid(unsafe_code)]

use glam::{DQuat, DVec3};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScrollGrammar {
    StandardSideView,
    LightDepth,
    JunctionApproach,
    JunctionDecision,
    TurnCommit,
    CameraReorientation,
    VistaReveal,
    VerticalTransition,
    Cutaway,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViewSide {
    Left,
    Right,
}

impl ViewSide {
    fn sign(self) -> f64 {
        match self {
            Self::Left => 1.0,
            Self::Right => -1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CameraRigConfig {
    pub side_distance: f64,
    pub height: f64,
    pub look_ahead: f64,
    pub look_height: f64,
    pub trailing_offset: f64,
    pub heading_half_life_seconds: f64,
    pub composition_half_life_seconds: f64,
}

impl Default for CameraRigConfig {
    fn default() -> Self {
        Self {
            side_distance: 16.0,
            height: 8.0,
            look_ahead: 4.0,
            look_height: 1.5,
            trailing_offset: 1.0,
            heading_half_life_seconds: 0.7,
            composition_half_life_seconds: 0.55,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraPose {
    pub position: DVec3,
    pub target: DVec3,
    pub up: DVec3,
}

impl CameraPose {
    pub fn forward(self) -> DVec3 {
        (self.target - self.position).normalize_or_zero()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct CompositionScale {
    distance: f64,
    height: f64,
    look: f64,
}

impl CompositionScale {
    const STANDARD: Self = Self {
        distance: 1.0,
        height: 1.0,
        look: 1.0,
    };

    fn lerp(self, target: Self, amount: f64) -> Self {
        Self {
            distance: self.distance + (target.distance - self.distance) * amount,
            height: self.height + (target.height - self.height) * amount,
            look: self.look + (target.look - self.look) * amount,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraRigState {
    config: CameraRigConfig,
    smoothed_heading: DVec3,
    transported_up: DVec3,
    view_side: ViewSide,
    composition: CompositionScale,
}

impl CameraRigState {
    pub fn new(
        config: CameraRigConfig,
        initial_heading: DVec3,
        view_side: ViewSide,
    ) -> Result<Self, CameraError> {
        validate_config(config)?;
        let heading = normalize_horizontal(initial_heading, DVec3::X)?;
        Ok(Self {
            config,
            smoothed_heading: heading,
            transported_up: DVec3::Y,
            view_side,
            composition: CompositionScale::STANDARD,
        })
    }

    pub const fn view_side(&self) -> ViewSide {
        self.view_side
    }

    pub fn set_view_side(&mut self, view_side: ViewSide) {
        self.view_side = view_side;
    }

    pub fn update(
        &mut self,
        subject_position: DVec3,
        route_tangent: DVec3,
        grammar: ScrollGrammar,
        delta_seconds: f64,
    ) -> Result<CameraPose, CameraError> {
        if !subject_position.is_finite() || !route_tangent.is_finite() {
            return Err(CameraError::NonFiniteInput);
        }
        if !delta_seconds.is_finite() || delta_seconds < 0.0 {
            return Err(CameraError::InvalidDelta(delta_seconds));
        }

        let desired_heading = normalize_horizontal(route_tangent, self.smoothed_heading)?;
        let heading_blend = smoothing_factor(self.config.heading_half_life_seconds, delta_seconds);
        self.smoothed_heading =
            rotate_horizontal_toward(self.smoothed_heading, desired_heading, heading_blend)?;

        let composition_blend =
            smoothing_factor(self.config.composition_half_life_seconds, delta_seconds);
        self.composition = self
            .composition
            .lerp(grammar_scale(grammar), composition_blend);

        let side = self
            .transported_up
            .cross(self.smoothed_heading)
            .normalize_or_zero()
            * self.view_side.sign();
        if side.length_squared() < 1.0e-12 {
            return Err(CameraError::DegenerateFrame);
        }

        let position = subject_position
            + side * self.config.side_distance * self.composition.distance
            + self.transported_up * self.config.height * self.composition.height
            - self.smoothed_heading * self.config.trailing_offset;
        let target = subject_position
            + self.smoothed_heading * self.config.look_ahead * self.composition.look
            + self.transported_up * self.config.look_height;

        Ok(CameraPose {
            position,
            target,
            up: self.transported_up,
        })
    }
}

fn validate_config(config: CameraRigConfig) -> Result<(), CameraError> {
    let values = [
        config.side_distance,
        config.height,
        config.look_ahead,
        config.look_height,
        config.trailing_offset,
        config.heading_half_life_seconds,
        config.composition_half_life_seconds,
    ];
    if values.iter().any(|value| !value.is_finite()) {
        return Err(CameraError::NonFiniteInput);
    }
    if config.side_distance <= 0.0
        || config.height < 0.0
        || config.heading_half_life_seconds < 0.0
        || config.composition_half_life_seconds < 0.0
    {
        return Err(CameraError::InvalidConfig);
    }
    Ok(())
}

fn grammar_scale(grammar: ScrollGrammar) -> CompositionScale {
    let (distance, height, look) = match grammar {
        ScrollGrammar::StandardSideView => (1.0, 1.0, 1.0),
        ScrollGrammar::LightDepth => (1.05, 1.0, 1.1),
        ScrollGrammar::JunctionApproach => (1.2, 1.15, 1.2),
        ScrollGrammar::JunctionDecision => (1.45, 1.35, 0.75),
        ScrollGrammar::TurnCommit => (1.3, 1.2, 0.9),
        ScrollGrammar::CameraReorientation => (1.55, 1.4, 0.8),
        ScrollGrammar::VistaReveal => (1.35, 1.25, 1.5),
        ScrollGrammar::VerticalTransition => (1.2, 1.3, 0.9),
        ScrollGrammar::Cutaway => (0.75, 0.85, 0.5),
    };
    CompositionScale {
        distance,
        height,
        look,
    }
}

fn normalize_horizontal(value: DVec3, fallback: DVec3) -> Result<DVec3, CameraError> {
    let horizontal = DVec3::new(value.x, 0.0, value.z);
    if horizontal.length_squared() >= 1.0e-12 {
        return Ok(horizontal.normalize());
    }

    let fallback_horizontal = DVec3::new(fallback.x, 0.0, fallback.z);
    if fallback_horizontal.length_squared() >= 1.0e-12 {
        return Ok(fallback_horizontal.normalize());
    }

    Err(CameraError::DegenerateFrame)
}

fn rotate_horizontal_toward(
    current: DVec3,
    desired: DVec3,
    amount: f64,
) -> Result<DVec3, CameraError> {
    let current = normalize_horizontal(current, DVec3::X)?;
    let desired = normalize_horizontal(desired, current)?;
    let dot = current.dot(desired).clamp(-1.0, 1.0);
    let signed_angle = current.cross(desired).y.atan2(dot);
    let rotated = DQuat::from_rotation_y(signed_angle * amount) * current;
    normalize_horizontal(rotated, desired)
}

fn smoothing_factor(half_life: f64, delta_seconds: f64) -> f64 {
    if half_life <= 0.0 {
        return 1.0;
    }
    1.0 - 2.0_f64.powf(-delta_seconds / half_life)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RouteFrame {
    pub position: DVec3,
    pub tangent: DVec3,
    pub segment_index: usize,
    pub distance_on_segment: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PolylineRoute {
    points: Vec<DVec3>,
    segment_lengths: Vec<f64>,
    cumulative_lengths: Vec<f64>,
    total_length: f64,
}

impl PolylineRoute {
    pub fn new(points: Vec<DVec3>) -> Result<Self, RouteError> {
        if points.len() < 2 {
            return Err(RouteError::TooFewPoints);
        }
        if points.iter().any(|point| !point.is_finite()) {
            return Err(RouteError::NonFinitePoint);
        }

        let mut segment_lengths = Vec::with_capacity(points.len() - 1);
        let mut cumulative_lengths = Vec::with_capacity(points.len());
        cumulative_lengths.push(0.0);
        let mut total_length = 0.0;

        for (index, pair) in points.windows(2).enumerate() {
            let length = pair[0].distance(pair[1]);
            if length <= 1.0e-9 {
                return Err(RouteError::ZeroLengthSegment(index));
            }
            segment_lengths.push(length);
            total_length += length;
            cumulative_lengths.push(total_length);
        }

        Ok(Self {
            points,
            segment_lengths,
            cumulative_lengths,
            total_length,
        })
    }

    pub fn points(&self) -> &[DVec3] {
        &self.points
    }

    pub const fn total_length(&self) -> f64 {
        self.total_length
    }

    pub fn sample(&self, distance: f64) -> Result<RouteFrame, RouteError> {
        if !distance.is_finite() {
            return Err(RouteError::NonFiniteDistance);
        }
        let clamped = distance.clamp(0.0, self.total_length);
        let segment_index = self
            .cumulative_lengths
            .partition_point(|value| *value <= clamped)
            .saturating_sub(1)
            .min(self.segment_lengths.len() - 1);
        let segment_start = self.cumulative_lengths[segment_index];
        let distance_on_segment = clamped - segment_start;
        let fraction = (distance_on_segment / self.segment_lengths[segment_index]).clamp(0.0, 1.0);
        let start = self.points[segment_index];
        let end = self.points[segment_index + 1];
        let delta = end - start;

        Ok(RouteFrame {
            position: start.lerp(end, fraction),
            tangent: delta.normalize(),
            segment_index,
            distance_on_segment,
        })
    }
}

#[derive(Clone, Copy, Debug, Error, PartialEq)]
pub enum CameraError {
    #[error("camera input or configuration contains a non-finite value")]
    NonFiniteInput,
    #[error("camera delta time must be finite and non-negative, got {0}")]
    InvalidDelta(f64),
    #[error("camera configuration is outside its valid range")]
    InvalidConfig,
    #[error("camera frame is degenerate")]
    DegenerateFrame,
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum RouteError {
    #[error("route requires at least two points")]
    TooFewPoints,
    #[error("route contains a non-finite point")]
    NonFinitePoint,
    #[error("route segment {0} has zero length")]
    ZeroLengthSegment(usize),
    #[error("route sample distance is non-finite")]
    NonFiniteDistance,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_both_sides_of_ninety_degree_turn() {
        let route = PolylineRoute::new(vec![
            DVec3::new(-20.0, 0.0, 0.0),
            DVec3::ZERO,
            DVec3::new(0.0, 0.0, -20.0),
        ])
        .expect("route");
        assert_eq!(route.sample(10.0).expect("before").tangent, DVec3::X);
        assert_eq!(route.sample(30.0).expect("after").tangent, -DVec3::Z);
    }

    #[test]
    fn grammar_change_is_smoothed_instead_of_teleporting_camera() {
        let mut rig =
            CameraRigState::new(CameraRigConfig::default(), DVec3::X, ViewSide::Left).expect("rig");
        let before = rig
            .update(
                DVec3::ZERO,
                DVec3::X,
                ScrollGrammar::StandardSideView,
                1.0 / 60.0,
            )
            .expect("before");
        let after = rig
            .update(
                DVec3::ZERO,
                DVec3::X,
                ScrollGrammar::CameraReorientation,
                1.0 / 60.0,
            )
            .expect("after");
        assert!(before.position.distance(after.position) < 0.5);
    }

    #[test]
    fn opposite_heading_rotates_without_degenerate_lerp() {
        let mut rig =
            CameraRigState::new(CameraRigConfig::default(), DVec3::X, ViewSide::Left).expect("rig");
        for _ in 0..180 {
            let pose = rig
                .update(
                    DVec3::ZERO,
                    -DVec3::X,
                    ScrollGrammar::CameraReorientation,
                    1.0 / 60.0,
                )
                .expect("reverse heading");
            assert!(pose.position.is_finite());
        }
    }

    #[test]
    fn explicit_view_side_is_preserved_during_turn() {
        let mut rig =
            CameraRigState::new(CameraRigConfig::default(), DVec3::X, ViewSide::Left).expect("rig");
        for _ in 0..180 {
            rig.update(
                DVec3::ZERO,
                -DVec3::Z,
                ScrollGrammar::CameraReorientation,
                1.0 / 60.0,
            )
            .expect("turn");
        }
        assert_eq!(rig.view_side(), ViewSide::Left);
    }
}
