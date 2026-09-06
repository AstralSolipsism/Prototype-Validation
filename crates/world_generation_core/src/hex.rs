use glam::DVec2;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct HexCoord {
    pub q: i16,
    pub r: i16,
}

impl HexCoord {
    pub const ZERO: Self = Self { q: 0, r: 0 };

    pub const fn new(q: i16, r: i16) -> Self {
        Self { q, r }
    }

    pub const fn s(self) -> i16 {
        -self.q - self.r
    }

    pub fn distance(self, other: Self) -> i16 {
        let dq = (self.q - other.q).abs();
        let dr = (self.r - other.r).abs();
        let ds = (self.s() - other.s()).abs();
        dq.max(dr).max(ds)
    }

    pub const fn neighbor(self, direction: HexDirection) -> Self {
        let (dq, dr) = direction.delta();
        Self {
            q: self.q + dq,
            r: self.r + dr,
        }
    }

    pub fn center_xz(self, radius_m: f64) -> DVec2 {
        let q = f64::from(self.q);
        let r = f64::from(self.r);
        DVec2::new(
            3.0_f64.sqrt() * radius_m * (q + r * 0.5),
            1.5 * radius_m * r,
        )
    }

    pub fn disk(radius: i16) -> Vec<Self> {
        let mut coords = Vec::new();
        for q in -radius..=radius {
            let r_min = (-radius).max(-q - radius);
            let r_max = radius.min(-q + radius);
            for r in r_min..=r_max {
                coords.push(Self::new(q, r));
            }
        }
        coords.sort();
        coords
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum HexDirection {
    East,
    NorthEast,
    NorthWest,
    West,
    SouthWest,
    SouthEast,
}

impl HexDirection {
    pub const ALL: [Self; 6] = [
        Self::East,
        Self::NorthEast,
        Self::NorthWest,
        Self::West,
        Self::SouthWest,
        Self::SouthEast,
    ];

    pub const fn index(self) -> usize {
        match self {
            Self::East => 0,
            Self::NorthEast => 1,
            Self::NorthWest => 2,
            Self::West => 3,
            Self::SouthWest => 4,
            Self::SouthEast => 5,
        }
    }

    pub const fn delta(self) -> (i16, i16) {
        match self {
            Self::East => (1, 0),
            Self::NorthEast => (1, -1),
            Self::NorthWest => (0, -1),
            Self::West => (-1, 0),
            Self::SouthWest => (-1, 1),
            Self::SouthEast => (0, 1),
        }
    }

    pub const fn opposite(self) -> Self {
        match self {
            Self::East => Self::West,
            Self::NorthEast => Self::SouthWest,
            Self::NorthWest => Self::SouthEast,
            Self::West => Self::East,
            Self::SouthWest => Self::NorthEast,
            Self::SouthEast => Self::NorthWest,
        }
    }

    pub fn between(from: HexCoord, to: HexCoord) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|direction| from.neighbor(*direction) == to)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct BoundaryKey {
    pub low: HexCoord,
    pub high: HexCoord,
}

impl BoundaryKey {
    pub fn new(left: HexCoord, right: HexCoord) -> Option<Self> {
        HexDirection::between(left, right)?;
        let (low, high) = if left <= right {
            (left, right)
        } else {
            (right, left)
        };
        Some(Self { low, high })
    }

    pub fn contains(self, coord: HexCoord) -> bool {
        self.low == coord || self.high == coord
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn radius_two_disk_contains_nineteen_cells() {
        let coords = HexCoord::disk(2);
        assert_eq!(coords.len(), 19);
        assert_eq!(coords.first().copied(), Some(HexCoord::new(-2, 0)));
        assert!(coords.contains(&HexCoord::ZERO));
    }

    #[test]
    fn neighbors_and_boundaries_are_symmetric() {
        let origin = HexCoord::ZERO;
        for direction in HexDirection::ALL {
            let neighbor = origin.neighbor(direction);
            assert_eq!(neighbor.neighbor(direction.opposite()), origin);
            assert_eq!(
                BoundaryKey::new(origin, neighbor),
                BoundaryKey::new(neighbor, origin)
            );
        }
    }
}
