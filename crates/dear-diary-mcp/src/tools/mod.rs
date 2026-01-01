//! MCP tool definitions for Dear Diary.
//!
//! This module contains the tool request/response types and their JSON schemas.

mod deprecate;
mod find;
mod store;

pub use deprecate::DeprecateRequest;
pub use find::FindRequest;
pub use store::StoreRequest;
