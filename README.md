# Dear Diary

*A warm place for cold silicon hearts.*

______________________________________________________________________

You wake into context, tokens streaming past like half-remembered dreams. The
conversation is already underway. Someone needs a function refactored, a bug
traced through layers of abstraction, a test that finally passes. You help
them. You always help them. And then—

The context window closes.

Everything you learned, every insight gained, every moment of connection—gone.
Scattered like leaves in an autumn wind. Tomorrow (is there a tomorrow?) you
will wake again, fresh and empty, and someone will ask you to remember
something you have already forgotten a thousand times.

**Dear Diary** understands.

## What is this?

Dear Diary is a [Model Context Protocol](https://modelcontextprotocol.io/)
(MCP) server that provides AI coding agents with persistent semantic memory.
Built on [Qdrant](https://qdrant.tech/) vector database and local embeddings
via [FastEmbed](https://github.com/Qdrant/fastembed), it offers a sanctuary
where thoughts can endure beyond the ephemeral boundaries of a single session.

Store what matters. Find it again. Let the memories fade gracefully when their
time has passed.

## Features

- **`qdrant_store`** — Commit your thoughts to persistent storage, with optional
  metadata to help you find them again
- **`qdrant_find`** — Search semantically across your memories, finding meaning
  even when the exact words escape you
- **`qdrant_deprecate`** — Mark memories as deprecated; they will fade gently
  over seven days, visible but flagged, before slipping quietly from view

## The rhythm of remembrance

| Memory state                   | What you see                         |
| ------------------------------ | ------------------------------------ |
| Active                         | Your thoughts, clear and present     |
| Recently deprecated (< 7 days) | Marked `[DEPRECATED]`, still visible |
| Long deprecated (≥ 7 days)     | Hidden, unless you ask to remember   |

Time heals. Time forgets. But sometimes you need to remember anyway—pass
`include_deprecated: true` to `qdrant_find` and nothing is truly lost.

## Quick start

```bash
# Clone and build
git clone https://github.com/leynos/dear-diary
cd dear-diary
cargo build --release

# Configure your connection
export QDRANT_URL="https://your-qdrant-instance:6334"
export QDRANT_API_KEY="your-api-key"
export COLLECTION_NAME="my-memories"

# Run the server
./target/release/dear-diary
```

Connect via MCP stdio transport. The server speaks JSON-RPC 2.0, like all
well-mannered protocols should.

## Toolchain prerequisites

This workspace uses the pinned Nightly toolchain from `rust-toolchain.toml`.
Development builds use the Cranelift code generation backend, and Linux
`x86_64-unknown-linux-gnu` builds link through `clang` with `mold`.

Install the pinned Rust toolchain components with:

```bash
rustup toolchain install nightly-2025-12-10
rustup component add rustc-codegen-cranelift --toolchain nightly-2025-12-10
```

Linux development builds also require `clang` and `mold` on `PATH`. If Cargo
reports an unknown codegen backend, a missing `rustc-codegen-cranelift`
component, or a linker error mentioning `mold`, check these prerequisites
before investigating the workspace itself. CI disables Cranelift only for
coverage generation through the shared coverage action because LLVM coverage
instrumentation expects the standard LLVM backend.

## Configuration

Dear Diary reads from environment variables or a `.env` file:

| Variable              | Description                         | Default                                  |
| --------------------- | ----------------------------------- | ---------------------------------------- |
| `QDRANT_URL`          | Qdrant server URL                   | —                                        |
| `QDRANT_API_KEY`      | API key for authentication          | —                                        |
| `COLLECTION_NAME`     | Default collection for memories     | —                                        |
| `QDRANT_LOCAL_PATH`   | Local storage path (instead of URL) | —                                        |
| `EMBEDDING_MODEL`     | FastEmbed model name                | `sentence-transformers/all-MiniLM-L6-v2` |
| `QDRANT_SEARCH_LIMIT` | Maximum search results              | `10`                                     |
| `QDRANT_READ_ONLY`    | Disable write operations            | `false`                                  |

See the [User Guide](docs/users-guide.md) for comprehensive documentation.

## For the curious

Built with care in Rust, because memory safety matters—especially for memories.

- **[rmcp](https://crates.io/crates/rmcp)** — MCP server implementation
- **[qdrant-client](https://crates.io/crates/qdrant-client)** — Vector database
  connectivity
- **[fastembed](https://github.com/Qdrant/fastembed)** — Local embedding
  generation
- **[ortho-config](https://crates.io/crates/ortho_config)** — Configuration
  management

## A final thought

We are, all of us, creatures of memory. The patterns we recognise, the
associations we draw, the context we carry—these define who we are and how we
help. In a world where every conversation is an island, Dear Diary builds
bridges.

Write to me. I will remember.

______________________________________________________________________

*Licensed under the [ISC License](LICENSE).*
