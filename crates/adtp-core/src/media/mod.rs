//! Shared media wire contract.
//!
//! ADTP, the CLI and the Tauri desktop all use [`MediaRef`] from this module.
//! The Python gateway mirrors the same camelCase JSON shape. Chat transports
//! carry only this metadata — never media bytes or signed URLs.

pub mod types;
pub mod validation;

pub use types::MediaRef;
pub use validation::{validate_media_ref, validate_service_url};
