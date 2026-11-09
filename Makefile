# Shortcuts for setup and daily use. Every target runs through scripts/run.sh,
# which activates Node from .nvmrc, pnpm (corepack) and the rustup toolchain,
# so no manual `nvm use` is needed. Run `make help` for the list.

RUN := ./scripts/run.sh
CARGO := ./scripts/cargo.sh

.DEFAULT_GOAL := help
.PHONY: help setup setup-e2e install dev build wasm test test-ts test-rust \
	conformance e2e lint typecheck format check run oracle record clean doctor

help: ## List targets
	@awk 'BEGIN {FS = ":.*## "} /^[a-zA-Z_-]+:.*## / {printf "  \033[1m%-12s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST)
	@echo ""
	@echo "  Examples: make run CHANNEL=fidelity/language/hello"
	@echo "            make record PROBE=fidelity/language/numbers"
	@echo "            make oracle ARGS=device-info"

setup: ## Install toolchains (asks first) and all dependencies
	@./scripts/setup.sh

setup-e2e: ## Same as setup, plus the Playwright browser for e2e tests
	@./scripts/setup.sh --e2e

install: ## Install Node dependencies only (lockfile)
	@$(RUN) pnpm install --frozen-lockfile

doctor: ## Show the resolved tool versions
	@$(RUN) sh -c 'echo "node  $$(node --version)"; echo "pnpm  $$(pnpm --version)"; rustc --version; cargo --version; rustup target list --installed | sed "s/^/target /"'

dev: ## Build wasm and start the web shell (http://127.0.0.1:5173)
	@$(RUN) pnpm dev

build: ## Production build (wasm + all packages)
	@$(RUN) pnpm build

wasm: ## Build the WebAssembly engine only
	@$(RUN) pnpm build:wasm

test: test-rust test-ts ## Rust + TypeScript unit tests

test-ts: ## TypeScript unit tests (Vitest)
	@$(RUN) pnpm test

test-rust: ## Rust unit tests
	@$(RUN) pnpm test:rust

conformance: ## Run probes in the engine and diff with device goldens
	@$(RUN) pnpm conformance

e2e: ## Browser fidelity tests (needs `make setup-e2e` once)
	@$(RUN) pnpm e2e

lint: ## ESLint + clippy
	@$(RUN) pnpm lint
	@$(RUN) $(CARGO) clippy --workspace --all-targets -- -D warnings

typecheck: ## TypeScript typecheck
	@$(RUN) pnpm typecheck

format: ## Format TypeScript/Markdown (Prettier) and Rust (rustfmt)
	@$(RUN) pnpm format
	@$(RUN) $(CARGO) fmt --all

check: ## Every CI gate except e2e
	@$(RUN) pnpm format:check
	@$(RUN) $(CARGO) fmt --all --check
	@$(MAKE) --no-print-directory lint typecheck test build
	-@$(MAKE) --no-print-directory conformance

run: ## Run a channel natively: make run CHANNEL=<dir|zip>
	@test -n "$(CHANNEL)" || { echo "usage: make run CHANNEL=<dir|zip>"; exit 2; }
	@$(RUN) $(CARGO) run --release -q -p brs-cli -- run "$(CHANNEL)"

oracle: ## Oracle CLI: make oracle ARGS="device-info"
	@$(RUN) pnpm oracle $(ARGS)

record: ## Record probes on the device: make record PROBE=fidelity/language
	@test -n "$(PROBE)" || { echo "usage: make record PROBE=<probe or area dir>"; exit 2; }
	@$(RUN) pnpm oracle record "$(PROBE)"

clean: ## Remove build outputs (keeps node_modules and .env)
	@$(RUN) $(CARGO) clean
	@rm -rf apps/web/dist apps/web/src/wasm test-results playwright-report apps/web/test-results apps/web/playwright-report
