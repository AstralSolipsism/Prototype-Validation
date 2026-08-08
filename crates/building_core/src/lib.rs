#![forbid(unsafe_code)]

mod compile;
mod delta;
mod model;
mod validate;

pub use compile::{compile_blueprint, semantic_fingerprint};
pub use delta::{apply_delta, impact_for_delta};
pub use model::*;
pub use validate::{validate_blueprint, BuildingError, ValidationReport};
