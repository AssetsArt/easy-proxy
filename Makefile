# Nylon Proxy Server Makefile
# 
# This Makefile provides targets for building, developing, and managing the Nylon proxy server.
# It includes development tools, build processes, and example compilation.

# Configuration
PORTS := 8088 8443 6192
RUST_BACKTRACE := 1
RUST_LOG := "info,warn,debug"
DEV_CMD := cargo watch -w crates -w examples -w proto -w sdk -q -c -s "cargo run -- run --config ./examples/config.yaml"

# Default target
.PHONY: default
default: dev

# Development target - runs the server in development mode
.PHONY: dev
dev:
	@echo "🧹 Cleaning up existing processes on ports: $(PORTS)"
	@for port in $(PORTS); do \
		kill -9 $$(lsof -t -i :$$port) 2>/dev/null || true; \
	done
	@echo "🚀 Starting Nylon development server..."
	RUST_BACKTRACE=$(RUST_BACKTRACE) $(DEV_CMD)

# Development target with debug logging
.PHONY: dev-debug
dev-debug:
	@echo "🐛 Starting Nylon development server with debug logging..."
	RUST_LOG=$(RUST_LOG) $(DEV_CMD)

# Generate FlatBuffers code from protocol definitions
.PHONY: generate
generate:
	@echo "📝 Generating FlatBuffers code..."
	flatc --rust -o sdk/rust/src/fbs proto/plugin.fbs
	flatc --go -o sdk/go/fbs proto/plugin.fbs
	@echo "✅ FlatBuffers code generation completed"

# Build release version
.PHONY: build
build:
	@echo "🏗️  Building Nylon release version..."
	cargo build --release
	@echo "✅ Release build completed"

# Clean build artifacts
.PHONY: clean
clean:
	@echo "🧹 Cleaning build artifacts..."
	cargo clean
	rm -rf target/examples/go/plugin_sdk.so
	@echo "✅ Clean completed"

# Run tests
.PHONY: test
test:
	@echo "🧪 Running tests..."
	cargo test
	@echo "✅ Tests completed"

# Check code formatting
.PHONY: fmt
fmt:
	@echo "🎨 Checking code formatting..."
	cargo fmt --check
	@echo "✅ Code formatting check completed"

# Run clippy linter
.PHONY: clippy
clippy:
	@echo "🔍 Running clippy linter..."
	cargo clippy -- -D warnings
	@echo "✅ Clippy check completed"

# Full code quality check
.PHONY: check
check: fmt clippy test
	@echo "✅ All code quality checks passed"

# Install development dependencies
.PHONY: install-dev
install-dev:
	@echo "📦 Installing development dependencies..."
	cargo install cargo-watch
	@echo "✅ Development dependencies installed"

# Show help
.PHONY: help
help:
	@echo "Nylon Proxy Server - Available targets:"
	@echo ""
	@echo "  dev          - Start development server with hot reload"
	@echo "  dev-debug    - Start development server with debug logging"
	@echo "  build        - Build release version"
	@echo "  generate     - Generate FlatBuffers code"
	@echo "  test         - Run tests"
	@echo "  fmt          - Check code formatting"
	@echo "  clippy       - Run clippy linter"
	@echo "  check        - Run all code quality checks"
	@echo "  clean        - Clean build artifacts"
	@echo "  install-dev  - Install development dependencies"
	@echo "  help         - Show this help message"
