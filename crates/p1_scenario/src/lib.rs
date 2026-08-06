#![forbid(unsafe_code)]

use glam::DVec3;
use scroll_camera_core::{PolylineRoute, RouteError, ScrollGrammar};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TravelDirection {
    Forward,
    Reverse,
}

impl TravelDirection {
    pub const fn sign(self) -> f64 {
        match self {
            Self::Forward => 1.0,
            Self::Reverse => -1.0,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Forward => "forward",
            Self::Reverse => "reverse",
        }
    }
}

pub fn route() -> Result<PolylineRoute, RouteError> {
    PolylineRoute::new(route_points())
}

pub fn route_points() -> Vec<DVec3> {
    vec![
        DVec3::new(-35.0, 0.0, 0.0),
        DVec3::new(-12.0, 0.0, 0.0),
        DVec3::new(0.0, 0.0, 0.0),
        DVec3::new(0.0, 0.0, -12.0),
        DVec3::new(0.0, 0.0, -26.0),
        DVec3::new(8.0, 0.5, -36.0),
        DVec3::new(20.0, 1.5, -43.0),
        DVec3::new(38.0, 3.0, -43.0),
    ]
}

pub fn grammar_at(
    distance: f64,
    total_length: f64,
    direction: TravelDirection,
) -> ScrollGrammar {
    let spatial_progress = (distance / total_length).clamp(0.0, 1.0);
    let directed_progress = match direction {
        TravelDirection::Forward => spatial_progress,
        TravelDirection::Reverse => 1.0 - spatial_progress,
    };

    match directed_progress {
        value if value < 0.23 => ScrollGrammar::StandardSideView,
        value if value < 0.32 => ScrollGrammar::JunctionApproach,
        value if value < 0.39 => ScrollGrammar::JunctionDecision,
        value if value < 0.48 => ScrollGrammar::TurnCommit,
        value if value < 0.62 => ScrollGrammar::CameraReorientation,
        value if value < 0.78 => ScrollGrammar::LightDepth,
        value if value < 0.9 => ScrollGrammar::VistaReveal,
        _ => ScrollGrammar::StandardSideView,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_directions_begin_in_standard_view() {
        let route = route().expect("route");
        assert_eq!(
            grammar_at(0.0, route.total_length(), TravelDirection::Forward),
            ScrollGrammar::StandardSideView
        );
        assert_eq!(
            grammar_at(
                route.total_length(),
                route.total_length(),
                TravelDirection::Reverse
            ),
            ScrollGrammar::StandardSideView
        );
    }
}
