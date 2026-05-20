# Dear Diary user guide

This guide provides comprehensive instructions for installing, configuring, and
using the Dear Diary Model Context Protocol (MCP) server.

## Overview

Dear Diary is an MCP server that provides semantic memory storage and retrieval
capabilities using a Qdrant vector database. It enables AI coding agents and
other MCP clients to persist information across sessions and retrieve it
through semantic search.

The server exposes three tools:

- **qdrant_store** — Store information with optional metadata
- **qdrant_find** — Search for information using semantic similarity
- **qdrant_deprecate** — Mark entries as deprecated for graceful removal

## Prerequisites

Before installing Dear Diary, ensure the following requirements are met:

- Rust 1.85 or later (see `rust-toolchain.toml` for the exact version)
- Access to a Qdrant instance (cloud or self-hosted)
- An MCP-compatible client (such as Claude Desktop or an AI coding agent)

## Installation

### Installing with cargo-binstall

On supported Linux GNU targets, install the prebuilt release archive with
`cargo-binstall`:

```bash
cargo install cargo-binstall
```

Then install Dear Diary:

```bash
cargo binstall dear-diary
```

The release workflow publishes `cargo-binstall` archives for Linux x86_64 and
aarch64 targets. For other targets, build from source.

### Building from source

Clone the repository and build the release binary:

```bash
git clone https://github.com/leynos/dear-diary
cd dear-diary
cargo build --release
```

The compiled binary is located at `target/release/dear-diary`.

### Verifying the build

Run the test suite to confirm the installation:

```bash
cargo test --workspace
```

### CI and coverage

CI measures Rust coverage with the shared `generate-coverage` action. Coverage
runs use the LLVM backend instead of Cranelift because `cargo-llvm-cov` relies
on LLVM coverage instrumentation.

Keep that LLVM instrumentation carve-out inside the shared coverage action. Do
not add a workflow-level or step-level `CARGO_PROFILE_DEV_CODEGEN_BACKEND=llvm`
override: that environment can leak into tool installation before coverage
starts.

## Configuration

Dear Diary is configured through environment variables. These may be set
directly in the shell, via a `.env` file in the working directory, or through
the MCP client configuration.

### Environment variables

_Table 1: Configuration environment variables._

| Variable                 | Description                                          | Required | Default                                  |
| ------------------------ | ---------------------------------------------------- | -------- | ---------------------------------------- |
| `QDRANT_URL`             | URL of the Qdrant server (including port)            | Yes[^1]  | —                                        |
| `QDRANT_API_KEY`         | API key for Qdrant authentication                    | No       | —                                        |
| `COLLECTION_NAME`        | Default collection name (supports interpolation[^2]) | No       | —                                        |
| `QDRANT_LOCAL_PATH`      | Path for local Qdrant storage                        | Yes[^1]  | —                                        |
| `EMBEDDING_MODEL`        | FastEmbed model identifier                           | No       | `sentence-transformers/all-MiniLM-L6-v2` |
| `QDRANT_SEARCH_LIMIT`    | Maximum number of search results                     | No       | `10`                                     |
| `QDRANT_READ_ONLY`       | Disable write operations                             | No       | `false`                                  |
| `TOOL_STORE_DESCRIPTION` | Custom description for store tool                    | No       | —                                        |
| `TOOL_FIND_DESCRIPTION`  | Custom description for find tool                     | No       | —                                        |

### Connection modes

Dear Diary supports two mutually exclusive connection modes:

1. **Remote mode** — Connect to a Qdrant server via `QDRANT_URL`
2. **Local mode** — Use local file storage via `QDRANT_LOCAL_PATH`

Exactly one of these must be configured.

### Example configuration

Create a `.env` file in the project root:

```plaintext
QDRANT_URL=https://abc123.eu-west-1-0.aws.cloud.qdrant.io:6334
QDRANT_API_KEY=your_qdrant_api_key_here
COLLECTION_NAME=my-memories
EMBEDDING_MODEL=sentence-transformers/all-MiniLM-L6-v2
QDRANT_SEARCH_LIMIT=10
```

### Collection name interpolation

The `COLLECTION_NAME` environment variable supports placeholder interpolation.
Placeholders are enclosed in braces and resolved at startup from the current
git repository and working directory.

_Table 2: Supported placeholders for `COLLECTION_NAME`._

| Placeholder | Description                               | Example value |
| ----------- | ----------------------------------------- | ------------- |
| `{repo}`    | Repository name from `origin` remote      | `dear-diary`  |
| `{owner}`   | Repository owner from `origin` remote     | `leynos`      |
| `{cwd}`     | Basename of the current working directory | `my-project`  |
| `{branch}`  | Current git branch name                   | `main`        |

Repository information is inferred from the `origin` remote URL. HTTPS, SSH,
and SCP-style URLs are all supported. For Source Hut repositories, the tilde
prefix is stripped from the owner (e.g. `~sircmpwn` becomes `sircmpwn`).

#### Example

```plaintext
COLLECTION_NAME={owner}-{repo}-notes
```

With a remote of `git@github.com:leynos/dear-diary.git`, this resolves to
`leynos-dear-diary-notes`.

#### Error behaviour

If a placeholder is used, but the corresponding value cannot be determined, the
server fails to start with a descriptive error message. Only placeholders that
are present in the value are evaluated, so `{cwd}` can be used outside a git
repository without requiring `{repo}` or `{branch}`.

Common failure scenarios:

- `{repo}` or `{owner}` used, but no `origin` remote is configured
- `{branch}` used, but HEAD is detached, or the directory is not a git
  repository
- `{cwd}` used, but the current directory cannot be determined

### Qdrant Cloud configuration

When connecting to Qdrant Cloud, use port 6334 for gRPC connections:

```plaintext
QDRANT_URL=https://abc123.eu-west-1-0.aws.cloud.qdrant.io:6334
QDRANT_API_KEY=your_cloud_api_key_from_qdrant_console
```

## Running the server

### Standalone execution

Run the server directly:

```bash
./target/release/dear-diary
```

The server communicates via stdio using the MCP JSON-RPC protocol.

### Integration with Claude Desktop

Add Dear Diary to the Claude Desktop configuration file:

On macOS, edit
`~/Library/Application Support/Claude/claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "dear-diary": {
      "command": "/path/to/dear-diary",
      "env": {
        "QDRANT_URL": "https://xyz789.eu-west-1-0.aws.cloud.qdrant.io:6334",
        "QDRANT_API_KEY": "your_production_api_key_here",
        "COLLECTION_NAME": "claude-memories"
      }
    }
  }
}
```

On Linux, the configuration file is typically located at
`~/.config/Claude/claude_desktop_config.json`.

## Tool reference

### qdrant_store

Store information in the Qdrant database.

#### Parameters

_Table 3: Parameters for qdrant_store._

| Parameter         | Type   | Required | Description                                 |
| ----------------- | ------ | -------- | ------------------------------------------- |
| `information`     | string | Yes      | The text content to store                   |
| `collection_name` | string | No       | Target collection (uses default if omitted) |
| `metadata`        | object | No       | Key-value pairs for filtering               |

#### Example request

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "qdrant_store",
    "arguments": {
      "information": "The authentication module uses JWT tokens with a 24-hour expiry.",
      "metadata": {
        "project": "backend-api",
        "category": "security"
      }
    }
  }
}
```

#### Example response

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "Successfully stored information in collection 'my-memories'"
      }
    ]
  }
}
```

### qdrant_find

Search for relevant information using semantic similarity.

#### Parameters

_Table 4: Parameters for qdrant_find._

| Parameter            | Type    | Required | Description                                 |
| -------------------- | ------- | -------- | ------------------------------------------- |
| `query`              | string  | Yes      | The search query                            |
| `collection_name`    | string  | No       | Target collection (uses default if omitted) |
| `filter`             | object  | No       | Qdrant filter object[^3]                    |
| `include_deprecated` | boolean | No       | Include deprecated entries (default: false) |

#### Deprecation filtering

By default, entries deprecated more than seven days ago are hidden from search
results. The `include_deprecated` parameter overrides this behaviour:

- `false` (default) — Hide entries deprecated ≥ 7 days ago
- `true` — Include all deprecated entries in results

Entries deprecated within the past seven days are always visible but prefixed
with `[DEPRECATED]`.

#### Example request

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/call",
  "params": {
    "name": "qdrant_find",
    "arguments": {
      "query": "How does authentication work?",
      "include_deprecated": false
    }
  }
}
```

#### Example response

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "The authentication module uses JWT tokens with a 24-hour expiry."
      }
    ]
  }
}
```

### qdrant_deprecate

Mark an entry as deprecated. Deprecated entries remain visible for seven days
(with a `[DEPRECATED]` prefix) before being hidden from default searches.

#### Parameters

_Table 5: Parameters for qdrant_deprecate._

| Parameter         | Type   | Required | Description                                 |
| ----------------- | ------ | -------- | ------------------------------------------- |
| `query`           | string | Yes      | Search query to find the entry to deprecate |
| `collection_name` | string | No       | Target collection (uses default if omitted) |

The tool finds the top matching entry for the query and marks it as deprecated.
If the entry is already deprecated, no changes are made.

#### Example request

```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "tools/call",
  "params": {
    "name": "qdrant_deprecate",
    "arguments": {
      "query": "JWT authentication expiry"
    }
  }
}
```

#### Example response

```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "Successfully deprecated entry: The authentication module uses JWT tokens with a 24-hour expiry."
      }
    ]
  }
}
```

## Deprecation lifecycle

The deprecation system provides graceful memory management:

_Table 6: Deprecation states and visibility._

| State               | Age        | Default visibility | With `include_deprecated` |
| ------------------- | ---------- | ------------------ | ------------------------- |
| Active              | —          | Visible            | Visible                   |
| Recently deprecated | < 7 days   | Visible (flagged)  | Visible (flagged)         |
| Long deprecated     | ≥ 7 days   | Hidden             | Visible (flagged)         |

Flagged entries are prefixed with `[DEPRECATED]` in search results.

## Collections

Collections in Qdrant are analogous to database tables. Dear Diary creates
collections automatically when the first entry is stored.

### Collection naming

Specify collections explicitly via the `collection_name` parameter, or
configure a default via the `COLLECTION_NAME` environment variable. If neither
is provided, store and find operations will fail with a missing collection
error.

### Multiple collections

Organize memories into separate collections for different purposes:

```json
{
  "arguments": {
    "information": "Database migration script location: /scripts/migrate.sh",
    "collection_name": "infrastructure-notes"
  }
}
```

## Embedding model

Dear Diary uses FastEmbed for local embedding generation. The default model is
`sentence-transformers/all-MiniLM-L6-v2`, which provides a good balance of
quality and performance.

### Model download

On first use, the embedding model is downloaded automatically to a local cache.
This may take a few moments depending on network speed.

### Alternative models

Configure a different model via the `EMBEDDING_MODEL` environment variable.
Consult the [FastEmbed documentation](https://github.com/Qdrant/fastembed) for
available models.

## Read-only mode

Set `QDRANT_READ_ONLY=true` to disable write operations. In this mode:

- `qdrant_store` returns an error
- `qdrant_deprecate` returns an error
- `qdrant_find` operates normally

This is useful for shared read-only access to a memory store.

## Error handling

Dear Diary returns structured MCP errors for common failure cases:

_Table 7: Common error conditions._

| Condition                    | Error code        | Resolution                                         |
| ---------------------------- | ----------------- | -------------------------------------------------- |
| Missing collection name      | `INVALID_PARAMS`  | Provide `collection_name` or set `COLLECTION_NAME` |
| Collection does not exist    | —                 | Use `qdrant_store` to create the collection        |
| Server in read-only mode     | `INVALID_REQUEST` | Disable read-only mode for write operations        |
| Connection failure           | `INTERNAL_ERROR`  | Verify Qdrant URL and credentials                  |
| Arbitrary filter not enabled | `INVALID_PARAMS`  | Filters require additional configuration           |

## Troubleshooting

### Connection refused

Verify that the Qdrant URL includes the correct port:

- gRPC connections (recommended): port 6334
- REST connections: port 6333

### Authentication failures

Ensure the API key is correctly configured. For Qdrant Cloud, the API key is
available in the cluster dashboard.

### Empty search results

- Confirm the collection exists by storing at least one entry
- Check that the collection name matches between store and find operations
- Verify the query is semantically related to stored content

### Model download failures

If the embedding model fails to download, check network connectivity and ensure
the cache directory is writable. The default cache location is
`.fastembed_cache/` in the working directory.

## Programmatic API

Dear Diary can be used as a library in addition to being run as a standalone
MCP server. The following types are exported from the `dear_diary_mcp` crate:

### `DiaryServer`

The main MCP server type. Create an instance with a Qdrant connector and
settings:

```rust,ignore
use dear_diary_mcp::DiaryServer;
use dear_diary_config::Settings;
use dear_diary_qdrant::QdrantConnectorImpl;
use rmcp::ServiceExt;

let settings = Settings::from_env()?;
let connector = QdrantConnectorImpl::new(...)?;
let server = DiaryServer::new(connector, settings);
server.serve(rmcp::transport::stdio()).await?;
```

### `McpServerError`

Error type for server operations, including:

- `MissingCollectionName` — No collection specified, and no default configured
- `InvalidFilter` — Filter parsing failed or filters not enabled
- `ConnectionError` — Failed to connect to Qdrant
- `StoreError` — Failed to store entry
- `SearchError` — Failed to search entries

### Request types

- `StoreRequest` — Parameters for the `qdrant_store` tool
- `FindRequest` — Parameters for the `qdrant_find` tool

## Known limitations

### Arbitrary filter parsing

The `filter` parameter on `qdrant_find` has limited support. The
`allow_arbitrary_filter` configuration option exists but is **not yet
functional**. Arbitrary JSON filter parsing is not implemented because Qdrant's
protobuf `Filter` type does not implement serde's `Deserialize` trait,
requiring manual construction from JSON structures.

**Current behaviour**: Providing a `filter` value in a `qdrant_find` request
returns an error:

- If `allow_arbitrary_filter` is `false` (default): "Arbitrary filters are not
  enabled"
- If `allow_arbitrary_filter` is `true`: "Arbitrary JSON filter parsing is not
  yet implemented"

**Workaround**: Use the `filterable_fields` configuration to enable metadata
filtering on specific fields. When entries are stored with metadata, the
specified fields become searchable through Qdrant's native filtering
capabilities.

**Tracking**: This limitation is tracked in
[GitHub issue `#2`](https://github.com/leynos/dear-diary/issues/2).

______________________________________________________________________

[^1]: Either `QDRANT_URL` or `QDRANT_LOCAL_PATH` must be set, but not both.

[^2]: See "Collection name interpolation" for supported placeholders.

[^3]: Arbitrary filter support requires additional configuration and is not
    enabled by default. See "Known limitations" for details.
