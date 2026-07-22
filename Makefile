.PHONY: all help test-admin build-admin test-home build-home test-student build-student test-backend build-backend

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
	@echo "  test-backend   - Run tests for the Rust backend"
	@echo "  build-backend  - Build the Rust backend"

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

test-backend:
	@echo "==============================="
	@echo "   Running Backend Tests       "
	@echo "==============================="
	cd backend-rust && cargo test

build-backend:
	@echo "==============================="
	@echo "   Building Rust Backend       "
	@echo "==============================="
	cd backend-rust && cargo build --release
