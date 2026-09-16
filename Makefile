.PHONY: help all clean test test-scripts test-workflow build release lint fmt \
	check-fmt markdownlint spelling nixie


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
# `make fmt` and `make check-fmt` call mdtablefix directly. `--git` selects the
# Markdown files Git tracks and `--include-untracked` adds the untracked files
# Git does not ignore, so a new document is formatted before it is staged.
# Both modes need mdtablefix 0.6.0 or later; CI pins the version at the
# install-mdtablefix step.
MDTABLEFIX ?= mdtablefix
MDTABLEFIX_SELECT = --git --include-untracked
MDTABLEFIX_RULES = --wrap --renumber --breaks --ellipsis --fences
NIXIE ?= nixie
PYTEST ?= uv run --with pytest --with cyclopts --with syrupy python -m pytest
WHITAKER ?= $(or $(shell command -v whitaker 2>/dev/null),$(wildcard $(USER_WHITAKER)),whitaker)
TYPOS_CONFIG_BUILDER_VERSION ?= v0.1.1
TYPOS_CONFIG_BUILDER = uv tool run --python 3.14 --from \
	"git+https://github.com/leynos/typos-config-builder.git@$(TYPOS_CONFIG_BUILDER_VERSION)" \
	typos-config-builder

build: target/debug/$(TARGET) ## Build debug binary
release: target/release/$(TARGET) ## Build release binary

all: check-fmt lint test test-scripts ## Perform a comprehensive check of code

clean: ## Remove build artifacts
	$(CARGO) clean
	rm -rf .coverage .pytest_cache scripts/__pycache__ scripts/tests/__pycache__
	rm -f .typos-oxendict-base.json .typos-oxendict-base.toml

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
	+$(MAKE) spelling

fmt: ## Format Rust and Markdown sources
	$(CARGO) fmt --all
	$(MDTABLEFIX) --in-place $(MDTABLEFIX_SELECT) $(MDTABLEFIX_RULES)
	$(MDLINT) --fix "**/*.md"

check-fmt: ## Verify formatting
	$(CARGO) fmt --all -- --check
	$(MDTABLEFIX) --check $(MDTABLEFIX_SELECT) $(MDTABLEFIX_RULES)

markdownlint: ## Lint Markdown files
	$(MDLINT) '**/*.md'
	+$(MAKE) spelling

spelling: ## Enforce en-GB-oxendict spelling in Markdown prose
	$(TYPOS_CONFIG_BUILDER) gate --repository .

nixie: ## Validate Mermaid diagrams
	$(NIXIE) --no-sandbox

help: ## Show available targets
	@grep -E '^[a-zA-Z_-]+:.*?##' $(MAKEFILE_LIST) | \
	awk -F '##' '{sub(/:.*$$/, "", $$1); printf "  %-20s%s\n", $$1, $$2}'
