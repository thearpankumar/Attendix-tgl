.PHONY: all help test-admin build-admin test-home build-home test-student build-student test-mobile build-mobile mobile-start mobile-web mobile-android mobile-ios test-extension build-extension test-extension-firefox test-backend build-backend deps-backend dev-backend lint-backend restart-backend clean-orphan-containers

# Run all checks and builds
all: test-admin build-admin test-home build-home test-student build-student test-mobile build-mobile test-extension build-extension test-backend build-backend

help:
	@echo "Available commands:"
	@echo "  all            - Run tests and builds for all frontend apps, mobile app, and backend"
	@echo "  test-admin     - Run lint, typecheck, and tests for Admin frontend"
	@echo "  build-admin    - Build Admin frontend"
	@echo "  test-home      - Run lint and typecheck for Home frontend"
	@echo "  build-home     - Build Home frontend"
	@echo "  test-student   - Run lint, typecheck, and tests for Student frontend"
	@echo "  build-student  - Build Student frontend"
	@echo "  test-mobile    - Run lint, typecheck, and tests for the mobile (Expo) app"
	@echo "  build-mobile   - Verify the mobile app's Metro/JS bundle builds (no native build on Linux — see mobile-android/mobile-ios)"
	@echo "  mobile-start   - Start the Expo dev server (scan the QR with Expo Go/dev client, or press 'w' for web)"
	@echo "  mobile-web     - Open the mobile app's UI directly in a browser on this PC (no phone/emulator needed)"
	@echo "  mobile-android - Build and run the mobile app on a connected Android device/emulator"
	@echo "  mobile-ios     - Trigger an EAS cloud build for iOS (Linux can't build/run iOS locally — needs Xcode)"
	@echo "  test-extension  - Run lint, typecheck, and tests for the browser extension"
	@echo "  build-extension - Build the browser extension for Chrome (mv3) and Firefox (mv2) — extension/.output/{chrome-mv3,firefox-mv2}/"
	@echo "  test-extension-firefox - Build, validate (web-ext lint), zip, and print how to load the extension in Firefox — snap-aware (see extension/README.md)"
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

test-mobile:
	@echo "==============================="
	@echo "   Running Mobile Tests        "
	@echo "==============================="
	cd mobile && npm run lint
	cd mobile && npm run typecheck
	cd mobile && npm run test

build-mobile:
	@echo "==============================="
	@echo "   Building Mobile App (JS)    "
	@echo "==============================="
	@echo "No native build on Linux without Xcode/an Android SDK — this only"
	@echo "verifies the Metro/JS bundle (imports, JSX, expo-router routes)."
	@echo "For a real device build see: make mobile-android / make mobile-ios"
	cd mobile && npx expo export --platform all --output-dir /tmp/mobile-build-check

mobile-start:
	@echo "==============================="
	@echo "   Starting Mobile Dev Server  "
	@echo "==============================="
	-adb reverse tcp:8080 tcp:80
	cd mobile && REACT_NATIVE_PACKAGER_HOSTNAME=localhost npx expo start

mobile-web:
	@echo "==============================="
	@echo "   Mobile App: Web Preview     "
	@echo "==============================="
	@echo "Quick UI check on this PC, no phone/emulator needed. Biometric"
	@echo "unlock is unavailable on web (falls back to a plain login, no"
	@echo "Face ID/fingerprint prompt) — this is for eyeballing screens/theme,"
	@echo "not for testing the real auth flow (use mobile-android for that)."
	cd mobile && npx expo start --web

mobile-android:
	@echo "==============================="
	@echo "   Running Mobile App: Android "
	@echo "==============================="
	@echo "Setting up adb reverse tcp:8080 tcp:80 (phone's localhost:8080 -> this"
	@echo "PC's localhost:80, matching mobile/.env) — harmless no-op if no device"
	@echo "is connected yet; re-run 'make mobile-android' after connecting one."
	@echo "NOTE: mobile/android/ is only generated ONCE and then reused on every"
	@echo "run (Continuous Native Generation) — if you changed app.json (icon,"
	@echo "splash, plugins, package name), that stale folder will NOT pick it up."
	@echo "Run 'rm -rf mobile/android' first to force a fresh regeneration."
	-adb reverse tcp:8080 tcp:80
	cd mobile && REACT_NATIVE_PACKAGER_HOSTNAME=localhost npx expo run:android

mobile-ios:
	@echo "==============================="
	@echo "   Building Mobile App: iOS    "
	@echo "==============================="
	@echo "Linux can't build/run iOS locally (needs Xcode) — this triggers an"
	@echo "EAS cloud build instead. First run needs 'npx eas login' and, for"
	@echo "device installs, 'eas credentials -p ios' once (see mobile/README.md)."
	cd mobile && npx eas build --platform ios --profile development

test-extension:
	@echo "==============================="
	@echo "   Running Extension Tests     "
	@echo "==============================="
	cd extension && npm run lint
	cd extension && npm run typecheck
	cd extension && npm run test

build-extension:
	@echo "==============================="
	@echo "   Building Browser Extension  "
	@echo "==============================="
	@echo "Chrome (MV3) -> extension/.output/chrome-mv3/ (+ zip + validate)"
	cd extension && npm run zip
	cd extension && npm run validate
	@echo "Firefox (MV2) -> extension/.output/firefox-mv2/ (+ zip + validate)"
	cd extension && npm run zip:firefox
	cd extension && npm run validate:firefox
	@echo ""
	@echo "Packages are in extension/.output/: *-chrome.zip, *-firefox.zip, and"
	@echo "an identical *-firefox.xpi (same bytes, Firefox's native package"
	@echo "extension — either works for testing), plus a *-sources.zip each —"
	@echo "the source-tree archive AMO review wants, not something to install."

test-extension-firefox:
	@echo "==============================="
	@echo " Validating + packaging (Firefox) "
	@echo "==============================="
	cd extension && npm run build:firefox
	cd extension && npm run validate:firefox
	cd extension && npm run zip:firefox
	@echo ""
	@echo "==============================="
	@echo "  Load the extension in Firefox  "
	@echo "==============================="
	@if snap list 2>/dev/null | grep -qi '^firefox '; then \
		echo "Detected: Firefox installed via snap on this machine."; \
		echo "Snap (and Flatpak) confinement blocks Firefox from reading an"; \
		echo "unpacked extension's sibling files when you pick a bare manifest.json"; \
		echo "— only the single selected file is unlocked, so Firefox can't load the"; \
		echo "rest of the extension and reports it as 'appears to be corrupt', even"; \
		echo "though the package itself just passed validation above with 0 errors."; \
		echo "(Mozilla bug 1852990: https://bugzilla.mozilla.org/show_bug.cgi?id=1852990)"; \
		echo ""; \
		echo "Workaround: load the ZIP (or the identical .xpi) instead of the bare"; \
		echo "manifest.json — reading one self-contained file doesn't hit the same"; \
		echo "sandbox restriction."; \
		echo "1. Open about:debugging#/runtime/this-firefox"; \
		echo "2. Click 'Load Temporary Add-on...' and select the package just built"; \
		echo "   in extension/.output/*-firefox.zip or *-firefox.xpi (NOT the"; \
		echo "   *-sources.zip — that's a separate, unrelated archive)"; \
	else \
		echo "1. Open about:debugging#/runtime/this-firefox"; \
		echo "2. Click 'Load Temporary Add-on...' and select:"; \
		echo "     extension/.output/firefox-mv2/manifest.json"; \
		echo "   (or the .zip/.xpi just built in extension/.output/ — all three"; \
		echo "   work on a non-snap, non-Flatpak Firefox)"; \
	fi
	@echo ""
	@echo "Either way, this is the Firefox equivalent of Chrome's 'Load unpacked' —"
	@echo "no signing needed, but it only lasts until Firefox restarts."
	@echo ""
	@echo "Do NOT use about:addons -> gear icon -> 'Install Add-on From File' with"
	@echo "the loose manifest.json — that dialog expects a packaged .zip/.xpi with"
	@echo "manifest.json at its root, and will reject a bare manifest.json with"
	@echo "'This add-on could not be installed because it appears to be corrupt.'"
	@echo "A permanently-installed zip on release Firefox still needs AMO signing;"
	@echo "that's a separate step, out of scope for local testing."

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
