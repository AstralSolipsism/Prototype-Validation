#![forbid(unsafe_code)]

mod compile;
mod delta;
mod model;
mod validate;

pub use compile::{CompileError, compile_blueprint, semantic_fingerprint};
pub use delta::{apply_delta, impact_for_delta};
pub use model::*;
pub use validate::{BuildingError, ValidationReport, validate_blueprint};
