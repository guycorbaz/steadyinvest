//! Response DTO (Data Transfer Object) layer.
//!
//! View structs define the JSON shape returned to API consumers,
//! decoupling internal model representation from the public API contract.
//!
//! - [`auth`] — Authentication response payloads (login tokens, user info)

pub mod auth;
