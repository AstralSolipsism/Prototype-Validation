#![forbid(unsafe_code)]

mod engine;
mod model;
mod projection;

pub use engine::{AuthorityError, AuthorityServer, apply_event};
pub use model::*;
pub use projection::ClientProjection;
