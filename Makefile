.PHONY: build test lint fmt deploy bindings load-test clean help

## Build the contract for WASM target
build:
	cargo build --release --target wasm32-unknown-unknown

## Run the full test suite
test:
	cargo test

## Run Clippy linter (warnings as errors) and check formatting
lint:
	cargo clippy -- -D warnings && cargo fmt --check

## Auto-format all source files with rustfmt
fmt:
	cargo fmt

## Deploy the contract using the deploy script
deploy:
	./scripts/deploy.sh

## Generate TypeScript bindings from the compiled WASM
bindings: build
	mkdir -p sdk/generated
	stellar contract bindings typescript \
		--wasm target/wasm32-unknown-unknown/release/mergemint_contracts.wasm \
		--output-dir sdk/generated

## Run k6 load scenarios against the backend (SCENARIO=mixed|list|filter|stream|tx)
load-test:
	./scripts/k6/run.sh $(or $(SCENARIO),mixed)

## Remove build artifacts and generated bindings
clean:
	cargo clean
	rm -rf sdk/generated

help:
	@echo "Available targets:"
	@echo "  make build     - Build WASM contract"
	@echo "  make test      - Run test suite"
	@echo "  make lint      - Run Clippy and check formatting"
	@echo "  make fmt       - Auto-format source files"
	@echo "  make deploy    - Deploy contract via scripts/deploy.sh"
	@echo "  make bindings  - Generate TypeScript bindings"
	@echo "  make load-test - Run k6 load scenarios (SCENARIO=mixed|list|filter|stream|tx)"
	@echo "  make clean     - Clean build artifacts"
	@echo "  make help      - Show this help"
