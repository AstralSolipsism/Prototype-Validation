#![forbid(unsafe_code)]

#[allow(
    clippy::collapsible_if,
    reason = "the route grammar keeps existence and slope checks visually separate"
)]
mod generate;
mod hex;
mod model;
mod validate;

pub use generate::{
    WorldGenerationError, generate_world, generate_world_with_order, semantic_fingerprint,
};
pub use hex::{BoundaryKey, HexCoord, HexDirection};
pub use model::*;
pub use validate::{ValidationReport, WorldValidationError, validate_world};
