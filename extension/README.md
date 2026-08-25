# Attendix Session Monitor — browser extension

WXT-based (Chrome MV3 + Firefox MV2) extension that pairs a device to a monitored Attendix session and reports presence/activity telemetry for its duration. See `../docs/workflow-monitoring.md` for how it fits into the overall pairing flow.

## Build

From the repo root, `make build-extension` builds **both** targets, validates **both**, and packages **both** in one go: Chrome → `.output/chrome-mv3/` + `.output/*-chrome.zip`, Firefox → `.output/firefox-mv2/` + `.output/*-firefox.zip` + `.output/*-firefox.xpi` (an identical copy — `.xpi` is Firefox's native package extension, byte-for-byte the same zip file, just named the way Firefox's own downloads/installs use). It fails the build if either validation surfaces a real (unexpected) error. Run it after every change before testing in a browser.

For just the unpacked, unzipped, unvalidated output (faster inner loop while developing), use `npm run build` (Chrome) / `npm run build:firefox` (Firefox) directly from here.

Configure the backend origin it talks to via `.env` (`WXT_API_BASE_URL`, see `.env.example`) before building — defaults to `http://localhost`.

## Validating the package before installing

`npm run validate` (Chrome) / `npm run validate:firefox` (Firefox) run Mozilla's own [`addons-linter`](https://github.com/mozilla/addons-linter) (via `web-ext lint`) against the built output — `make build-extension` runs both automatically. It parses the manifest and every referenced file exactly like a real browser would, and reports `errors`/`warnings`/`notices` — 0 (unexpected) errors means the package itself is structurally sound, which rules out "the extension is actually broken" before you even open a browser.

- `validate:firefox` is a plain `web-ext lint` call — genuinely 0 errors today (a few pre-existing `innerHTML` and `data_collection_permissions` warnings, tracked separately, not corruption).
- `validate` (Chrome) goes through `scripts/validate-chrome.mjs`, a thin wrapper around the same `web-ext lint`, because that tool is Firefox/AMO-oriented and always flags this build's `background.service_worker` (MV3, Chrome-only, no `background.scripts` fallback) as an error — that fallback only matters if one manifest is meant to run on both browsers, and this repo deliberately ships two separate manifests instead. The wrapper ignores exactly that one expected finding (`BACKGROUND_SERVICE_WORKER_NOFALLBACK`) and fails on anything else. `npm run validate:raw` runs the unfiltered `web-ext lint` directly if you want to see everything it reports, including that expected one.

`make test-extension-firefox` runs the Firefox build + `validate:firefox` + `zip:firefox` for you, then prints load instructions (see below).

## Testing locally

**Chrome**: `chrome://extensions` → enable Developer mode → "Load unpacked" → select `.output/chrome-mv3/`.

**Firefox — if it's a native/`.deb`/Mozilla-tarball install**: open `about:debugging#/runtime/this-firefox` → "Load Temporary Add-on…" → select `.output/firefox-mv2/manifest.json` directly (not a folder, not a zip). This is Firefox's equivalent of Chrome's "Load unpacked" — no signing required, but it unloads when Firefox restarts.

**Firefox — if it's a snap or Flatpak install** (the default on Ubuntu — check with `snap list | grep firefox` or `flatpak list | grep firefox`): selecting the bare `manifest.json` above will fail with *"This add-on could not be installed because it appears to be corrupt"*, even though the package is valid. Load the **zip or xpi** instead: `npm run zip:firefox` (produces both), then in `about:debugging#/runtime/this-firefox` → "Load Temporary Add-on…", select the generated `.output/*-firefox.zip` or `.output/*-firefox.xpi` — they're identical bytes, pick whichever (not the `*-sources.zip` — that's a separate archive of the source tree, for AMO review submissions). Reading one self-contained file doesn't hit the sandbox restriction that breaks the sibling-file case. This also works fine on a non-snap Firefox, so it's the safer default if you're not sure which install type you have.

`make test-extension-firefox` detects which case applies on the current machine and prints the matching instructions.

### Why this happens: two unrelated causes of the same error message

**1. Snap/Flatpak sandboxing (the one above).** Snap and Flatpak confine Firefox's filesystem access to whatever the file picker explicitly unlocked. Picking a single `manifest.json` only unlocks that one file — not the sibling `background.js`, `popup.html`, `content-scripts/`, etc. in the same directory — so Firefox can't actually read the rest of the extension and reports the whole thing as corrupt. This is a known Firefox bug, not specific to this extension: [Mozilla bug 1852990](https://bugzilla.mozilla.org/show_bug.cgi?id=1852990), "Firefox Flatpak (and Firefox snap on Ubuntu) cannot load unpacked temporary Add-ons from directory." Loading a zip instead sidesteps it, since a zip is read as one file.

**2. Using the wrong install dialog.** Firefox's regular Add-ons Manager (`about:addons` → gear icon → "Install Add-on From File") is for installing a packaged `.zip`/`.xpi` with `manifest.json` at the archive root — it is **not** an unpacked-folder-or-file loader, and it doesn't go through `about:debugging`'s temporary-install path at all. Selecting a loose `manifest.json` there makes Firefox try to unzip a JSON file and fail with the exact same *"appears to be corrupt"* message. Always use `about:debugging` (above) for local testing, never `about:addons`, for an unsigned build.

Either way, `npm run validate:firefox`/`npm run validate` (see above) is how you rule out "the manifest/package is actually broken" before chasing either of these — if it reports 0 errors, the corruption is in the install method, not the build.

If you need a real signed install (not just local testing), a permanently-installed zip on release/beta Firefox still requires AMO signing (or `xpinstall.signatures.required = false` in `about:config`, only available on Developer Edition/Nightly/ESR) — that's a separate step from everything above.
