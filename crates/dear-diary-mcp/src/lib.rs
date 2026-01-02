//! MCP server implementation for Dear Diary.
//!
//! This crate implements the Model Context Protocol server using
//! `rmcp`, providing `qdrant_store` and `qdrant_find` tools for
//! storing and retrieving information from a Qdrant vector database.
//!
//! # Example
//!
//! ```ignore
//! use dear_diary_mcp::DiaryServer;
//! use dear_diary_config::Settings;
//! use dear_diary_qdrant::QdrantConnectorImpl;
//! use rmcp::ServiceExt;
//!
//! let settings = Settings::from_env()?;
//! let connector = QdrantConnectorImpl::new(...)?;
//! let server = DiaryServer::new(connector, settings);
//! server.serve(rmcp::transport::stdio()).await?;
//! ```

mod deprecation;
mod error;
mod server;
mod tools;

pub use error::McpServerError;
pub use server::DiaryServer;
pub use tools::{FindRequest, StoreRequest};
