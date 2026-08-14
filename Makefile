.PHONY: all help test-admin build-admin test-home build-home test-student build-student test-backend build-backend deps-backend dev-backend lint-backend restart-backend clean-orphan-containers

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
	@echo "  clean-orphan-containers - Remove leaked testcontainers containers (safety net; backend-rust's own tests now use named/reused containers, see tests/common/test_db.rs)"

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
	@echo "   Building Rust Backend       "
	@echo "==============================="
	docker compose build backend

restart-backend: build-backend
	@echo "==============================="
	@echo "   Restarting Backend          "
	@echo "==============================="
	docker compose up -d backend

# Safety net, not the primary fix: backend-rust's tests (tests/common/test_db.rs)
# now use named, `with_reuse(Always)` Postgres/Redis containers shared across
# every test binary and run, instead of a fresh throwaway pair per binary --
# that's what stops the leak at the source. This just mops up anything still
# labeled testcontainers-managed (e.g. from before that fix, or from a run that
# got killed hard enough to skip even the reuse path) without touching
# unrelated containers on the machine.
#
# testcontainers stamps org.testcontainers.managed-by=testcontainers on EVERY
# container it starts, including the two intentionally-reused ones below --
# they're excluded by name here so this can't undo the fix above by deleting
# the very containers meant to persist.
REUSED_TEST_CONTAINERS := backend-rust-test-postgres backend-rust-test-redis

clean-orphan-containers:
	@echo "==============================="
	@echo "   Cleaning Orphan Containers  "
	@echo "==============================="
	@exclude="$$(echo '$(REUSED_TEST_CONTAINERS)' | tr ' ' '|')"; \
	ids="$$(docker ps -a --filter 'label=org.testcontainers.managed-by=testcontainers' --format '{{.ID}} {{.Names}}' | grep -Ev " ($$exclude)\$$" | awk '{print $$1}')"; \
	if [ -n "$$ids" ]; then \
		echo "Removing $$(echo "$$ids" | wc -l) leaked testcontainers container(s)..."; \
		docker rm -f $$ids >/dev/null; \
	else \
		echo "No leaked testcontainers containers found."; \
	fi
