//! Dear Diary MCP server entry point.
//!
//! This binary runs the MCP server with stdio transport, connecting
//! to a Qdrant vector database for semantic memory storage and retrieval.

use std::process::ExitCode;

use dear_diary_config::Settings;
use dear_diary_embeddings::FastEmbedProvider;
use dear_diary_mcp::DiaryServer;
use dear_diary_qdrant::QdrantConnectorImpl;
use rmcp::ServiceExt;

/// Application entry point.
///
/// Initializes the MCP server with configuration from environment variables
/// and runs with stdio transport.
#[expect(clippy::print_stderr, reason = "CLI error output is the intended behaviour")]
fn main() -> ExitCode {
    // Run the async runtime
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
    // Load configuration from environment
    let settings = Settings::from_env().map_err(|e| format!("Configuration error: {e}"))?;

    // Initialize embedding provider
    let embedding_provider = FastEmbedProvider::new(&settings.embedding_model)
        .map_err(|e| format!("Failed to initialize embedding provider: {e}"))?;

    // Initialize Qdrant connector based on configuration
    let connector = if let Some(ref url) = settings.qdrant.qdrant_url {
        QdrantConnectorImpl::new(
            url,
            settings.qdrant.qdrant_api_key.as_deref(),
            settings.qdrant.collection_name.clone(),
            embedding_provider,
        )
        .map_err(|e| format!("Failed to connect to Qdrant: {e}"))?
    } else if let Some(ref path) = settings.qdrant.qdrant_local_path {
        QdrantConnectorImpl::new_local(
            path,
            settings.qdrant.collection_name.clone(),
            embedding_provider,
        )
        .map_err(|e| format!("Failed to initialize local Qdrant: {e}"))?
    } else {
        return Err("Either QDRANT_URL or QDRANT_LOCAL_PATH must be set".to_owned());
    };

    // Create and run the MCP server
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
