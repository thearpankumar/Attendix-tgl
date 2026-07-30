.PHONY: all help test-admin build-admin test-home build-home test-student build-student test-backend build-backend deps-backend dev-backend lint-backend restart-backend

# Run all checks and builds
all: test-admin build-admin test-home build-home test-student build-student test-backend build-backend

help:
	@echo "Available commands:"
	@echo "  all            - Run tests and builds for all frontend apps and backend"
	@echo "  test-admin     - Run lint, typecheck, and tests for Admin frontend"
	@echo "  build-admin    - Build Admin frontend"
	@echo "  test-home      - Run lint and typecheck for Home frontend"
	@echo "  build-home     - Build Home frontend"
	@echo "  test-student   - Run lint, typecheck, and tests for Student frontend"
	@echo "  build-student  - Build Student frontend"
	@echo "  deps-backend   - Fetch/build Rust backend dependencies"
	@echo "  test-backend   - Run tests for the Rust backend (--all-features, matches CI)"
	@echo "  lint-backend   - Run clippy and fmt --check for the Rust backend (matches CI)"
	@echo "  dev-backend    - Run the Rust backend in dev mode (cargo run)"
	@echo "  build-backend  - Build the Rust backend"
	@echo "  restart-backend - Rebuild and restart the backend container"

test-admin:
	@echo "==============================="
	@echo "   Running Admin Tests         "
	@echo "==============================="
	cd frontend/admin && npm run lint
	cd frontend/admin && npm run typecheck
	cd frontend/admin && npm run test

build-admin:
	@echo "==============================="
	@echo "   Building Admin App          "
	@echo "==============================="
	cd frontend/admin && npm run build

test-home:
	@echo "==============================="
	@echo "   Running Home Tests          "
	@echo "==============================="
	cd frontend/home && bun run lint
	cd frontend/home && bun run typecheck

build-home:
	@echo "==============================="
	@echo "   Building Home App           "
	@echo "==============================="
	cd frontend/home && bun run build

test-student:
	@echo "==============================="
	@echo "   Running Student Tests       "
	@echo "==============================="
	cd frontend/student && npm run lint
	cd frontend/student && npm run typecheck
	cd frontend/student && npm run test

build-student:
	@echo "==============================="
	@echo "   Building Student App        "
	@echo "==============================="
	cd frontend/student && npm run build

deps-backend:
	@echo "==============================="
	@echo "   Fetching Backend Deps       "
	@echo "==============================="
	cd backend-rust && cargo build

test-backend:
	@echo "==============================="
	@echo "   Running Backend Tests       "
	@echo "==============================="
	cd backend-rust && cargo test --all-features

lint-backend:
	@echo "==============================="
	@echo "   Running Backend Linter      "
	@echo "==============================="
	cd backend-rust && cargo clippy -- -D warnings
	cd backend-rust && cargo fmt -- --check

dev-backend:
	@echo "==============================="
	@echo "   Starting Backend (dev)      "
	@echo "==============================="
	cd backend-rust && cargo run

build-backend:
	@echo "==============================="
	@echo "   Building Rust Backend (GNU)"
	@echo "==============================="
	cd backend-rust && cargo zigbuild --release --target x86_64-unknown-linux-gnu
	docker compose build backend

restart-backend: build-backend
	@echo "==============================="
	@echo "   Restarting Backend          "
	@echo "==============================="
	docker compose up -d backend
