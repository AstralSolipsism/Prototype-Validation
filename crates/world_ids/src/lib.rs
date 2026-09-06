#![forbid(unsafe_code)]

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::fmt;

macro_rules! stable_id {
    ($name:ident) => {
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u128);

        impl $name {
            pub const fn from_u128(value: u128) -> Self {
                Self(value)
            }

            pub const fn from_parts(high: u64, low: u64) -> Self {
                Self(((high as u128) << 64) | low as u128)
            }

            pub const fn as_u128(self) -> u128 {
                self.0
            }

            pub const fn high(self) -> u64 {
                (self.0 >> 64) as u64
            }

            pub const fn low(self) -> u64 {
                self.0 as u64
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(formatter, "{:032x}", self.0)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(formatter, "{}({:032x})", stringify!($name), self.0)
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str(&self.to_string())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct StableIdVisitor;

                impl<'de> de::Visitor<'de> for StableIdVisitor {
                    type Value = u128;

                    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                        formatter.write_str("a 32-character lowercase hexadecimal stable ID")
                    }

                    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
                    where
                        E: de::Error,
                    {
                        if value.len() != 32
                            || !value
                                .bytes()
                                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                        {
                            return Err(E::custom(
                                "stable ID must be exactly 32 lowercase hexadecimal characters",
                            ));
                        }
                        u128::from_str_radix(value, 16).map_err(E::custom)
                    }
                }

                deserializer.deserialize_str(StableIdVisitor).map(Self)
            }
        }
    };
}

stable_id!(WorldId);
stable_id!(EntityId);
stable_id!(PersonId);
stable_id!(BuildingId);
stable_id!(BuildingInstanceId);
stable_id!(VehicleId);
stable_id!(CellId);
stable_id!(RegionId);
stable_id!(FrameId);
stable_id!(CommandId);
stable_id!(EventId);
stable_id!(BuildingLevelId);
stable_id!(RoomId);
stable_id!(WallId);
stable_id!(OpeningId);
stable_id!(StairId);
stable_id!(RoofRegionId);
stable_id!(ArchitectureStyleId);
stable_id!(RiverId);
stable_id!(RoadId);
stable_id!(SettlementId);
stable_id!(LandmarkId);
stable_id!(RouteId);
stable_id!(PortalId);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_high_and_low_parts() {
        let id = PersonId::from_parts(0x0123_4567_89ab_cdef, 0xfedc_ba98_7654_3210);
        assert_eq!(id.high(), 0x0123_4567_89ab_cdef);
        assert_eq!(id.low(), 0xfedc_ba98_7654_3210);
        assert_eq!(id.to_string(), "0123456789abcdeffedcba9876543210");
    }

    #[test]
    fn serializes_as_fixed_width_hex_string() {
        let id = BuildingId::from_parts(u64::MAX, u64::MAX);
        let encoded = serde_json::to_string(&id).expect("serialize id");
        assert_eq!(encoded, "\"ffffffffffffffffffffffffffffffff\"");
        let decoded: BuildingId = serde_json::from_str(&encoded).expect("deserialize id");
        assert_eq!(decoded, id);
    }

    #[test]
    fn building_element_ids_share_the_same_wire_contract() {
        let room = RoomId::from_u128(42);
        let wall = WallId::from_u128(43);
        assert_eq!(room.to_string().len(), 32);
        assert_eq!(wall.to_string().len(), 32);
        assert_eq!(
            serde_json::to_string(&room).expect("room"),
            format!("\"{room}\"")
        );
    }

    #[test]
    fn world_feature_ids_share_the_same_wire_contract() {
        let settlement = SettlementId::from_u128(100);
        let route = RouteId::from_u128(101);
        let portal = PortalId::from_u128(102);
        assert_eq!(settlement.to_string().len(), 32);
        assert_eq!(route.to_string().len(), 32);
        assert_eq!(portal.to_string().len(), 32);
    }

    #[test]
    fn rejects_ambiguous_or_malformed_text() {
        for invalid in [
            "\"2a\"",
            "\"0000000000000000000000000000002A\"",
            "\"0000000000000000000000000000002g\"",
        ] {
            assert!(serde_json::from_str::<EntityId>(invalid).is_err());
        }
    }
}
