.PHONY: help all clean test test-scripts test-workflow build release lint fmt check-fmt markdownlint nixie


TARGET ?= dear-diary

USER_WHITAKER := $(HOME)/.local/bin/whitaker
USER_BIN_PATH := $(HOME)/.cargo/bin:$(HOME)/.local/bin:$(HOME)/.bun/bin
CARGO ?= cargo
BUILD_JOBS ?=
RUST_FLAGS ?= -D warnings
RUSTDOC_FLAGS ?= -D warnings
CARGO_FLAGS ?= --workspace --all-targets --all-features
CLIPPY_FLAGS ?= $(CARGO_FLAGS) -- $(RUST_FLAGS)
TEST_FLAGS ?= $(CARGO_FLAGS)
MDLINT ?= markdownlint-cli2
NIXIE ?= nixie
PYTEST ?= uv run --with pytest --with cyclopts --with syrupy python -m pytest
WHITAKER ?= $(or $(shell command -v whitaker 2>/dev/null),$(wildcard $(USER_WHITAKER)),whitaker)

build: target/debug/$(TARGET) ## Build debug binary
release: target/release/$(TARGET) ## Build release binary

all: check-fmt lint test test-scripts ## Perform a comprehensive check of code

clean: ## Remove build artifacts
	$(CARGO) clean

test: ## Run tests with warnings treated as errors
	RUSTFLAGS="$(RUST_FLAGS)" $(CARGO) test $(TEST_FLAGS) $(BUILD_JOBS)

test-scripts: ## Run Python script tests
	$(PYTEST) scripts/tests

test-workflow: ## Run act-backed workflow tests; requires act and Docker/Podman
	uv run --with pytest --with cmd-mox --with pyyaml python -m pytest tests/

target/%/$(TARGET): ## Build binary in debug or release mode
	$(CARGO) build $(BUILD_JOBS) $(if $(findstring release,$(@)),--release) --bin $(TARGET)

lint: ## Run Clippy with warnings denied
	RUSTDOCFLAGS="$(RUSTDOC_FLAGS)" $(CARGO) doc --workspace --no-deps
	$(CARGO) clippy $(CLIPPY_FLAGS)
	PATH="$(USER_BIN_PATH):$(PATH)" RUSTFLAGS="$(RUST_FLAGS)" $(WHITAKER) --all -- $(CARGO_FLAGS)

fmt: ## Format Rust and Markdown sources
	$(CARGO) fmt --all
	mdformat-all

check-fmt: ## Verify formatting
	$(CARGO) fmt --all -- --check

markdownlint: ## Lint Markdown files
	$(MDLINT) '**/*.md'

nixie: ## Validate Mermaid diagrams
	$(NIXIE) --no-sandbox

help: ## Show available targets
	@grep -E '^[a-zA-Z_-]+:.*?##' $(MAKEFILE_LIST) | \
	awk -F '##' '{sub(/:.*$$/, "", $$1); printf "  %-20s%s\n", $$1, $$2}'
