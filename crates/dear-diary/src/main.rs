//! Dear Diary MCP server entry point.
//!
//! This binary runs the MCP server with stdio transport, connecting
//! to a Qdrant vector database for semantic memory storage and retrieval.

use std::process::ExitCode;

use dear_diary_config::{QdrantSettings, Settings};
use dear_diary_embeddings::FastEmbedProvider;
use dear_diary_mcp::DiaryServer;
use dear_diary_qdrant::QdrantConnectorImpl;
use rmcp::ServiceExt;

/// Determines the Qdrant connection mode from settings.
enum ConnectionMode<'a> {
    Remote {
        url: &'a str,
        api_key: Option<&'a str>,
    },
    Local {
        path: &'a str,
    },
}

/// Returns the connection mode based on Qdrant settings.
fn connection_mode(settings: &QdrantSettings) -> Result<ConnectionMode<'_>, String> {
    match (&settings.qdrant_url, &settings.qdrant_local_path) {
        (Some(url), _) => Ok(ConnectionMode::Remote {
            url,
            api_key: settings.qdrant_api_key.as_deref(),
        }),
        (None, Some(path)) => Ok(ConnectionMode::Local { path }),
        (None, None) => Err("Either QDRANT_URL or QDRANT_LOCAL_PATH must be set".to_owned()),
    }
}

/// Application entry point.
///
/// Initializes the MCP server with configuration from environment variables
/// and runs with stdio transport.
#[expect(
    clippy::print_stderr,
    reason = "CLI error output is the intended behaviour"
)]
fn main() -> ExitCode {
    let result = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("Failed to create tokio runtime: {e}"))
        .and_then(|rt| rt.block_on(run()));

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Error: {e}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), String> {
    let settings = Settings::from_env().map_err(|e| format!("Configuration error: {e}"))?;

    let embedding_provider = FastEmbedProvider::new(&settings.embedding_model)
        .map_err(|e| format!("Failed to initialize embedding provider: {e}"))?;

    let connector = match connection_mode(&settings.qdrant)? {
        ConnectionMode::Remote { url, api_key } => QdrantConnectorImpl::new(
            url,
            api_key,
            settings.qdrant.collection_name.clone(),
            embedding_provider,
        )
        .map_err(|e| format!("Failed to connect to Qdrant: {e}"))?,
        ConnectionMode::Local { path } => QdrantConnectorImpl::new_local(
            path,
            settings.qdrant.collection_name.clone(),
            embedding_provider,
        )
        .map_err(|e| format!("Failed to initialize local Qdrant: {e}"))?,
    };

    let server = DiaryServer::new(connector, settings);

    server
        .serve(rmcp::transport::stdio())
        .await
        .map_err(|e| format!("Server error: {e}"))?
        .waiting()
        .await
        .map_err(|e| format!("Server error: {e}"))?;

    Ok(())
}
