# Attendix Mentor

React Native + Expo (TypeScript) mentor app for the Attendance GEOTAG System. Replicates `frontend/mentor` (login, dashboard, session roster marking, settings) with a native login/biometric-unlock flow, talking to the same Rust backend (`backend-rust/`) over `Authorization: Bearer <token>` — no cookies.

## Requirements

Biometric unlock (`expo-local-authentication` + `expo-secure-store`'s `requireAuthentication`) and screenshot protection (`expo-screen-capture`) need native config that **plain Expo Go cannot provide** (custom `NSFaceIDUsageDescription`, etc.). Development must use a dev client:

```bash
npm install
npx expo prebuild   # only needed once, or after adding a new native dependency
npx expo run:ios    # or: npx expo run:android
```

Day-to-day after the first native build, `npx expo start` + the installed dev client is enough — no need to `prebuild`/`run` again unless native deps change.

## Pointing at a backend

The API base URL comes from `app.config.ts`, which reads `EXPO_PUBLIC_API_BASE_URL` and falls back to the production URL (`https://attendixv2.talenciaglobal.com/api`, set in `app.json`'s `extra.apiBaseUrl`).

Copy `.env.example` to `.env` and pick the right value for how you're running the app. Caddy (the dev stack's reverse proxy) publishes ports 80/443 on every interface, so a physical device on the same Wi-Fi/LAN, an Android emulator, and this PC's own browser can all reach `docker compose up` in the repo root the same way — see `.env.example`'s comments for the exact URLs.

`eas.json`'s `development`/`preview`/`production` build profiles each set `EXPO_PUBLIC_API_BASE_URL` via their own `env` block — update those if the team sets up a shared staging backend.

## Backend prerequisite

Writes (login, mark/undo attendance, change password) require the CSRF-exemption patch in `backend-rust/src/middleware/csrf.rs` that lets cookie-less `Authorization: Bearer` requests through — see that file's comments. Without it, every POST/PATCH/DELETE from this app gets a 403.

## Testing

```bash
npm run typecheck   # tsc --noEmit
npm run lint        # expo lint
npm run test         # jest
npm run test:coverage
```

Tests use `jest-expo` + `@testing-library/react-native` (v14 — note both `render` and `renderHook` are `async` in this version, unlike older RNTL). Coverage focuses on business logic worth testing in isolation — theme-agnostic utils, `sessionState`/`applyLocalStatus` (the dashboard/roster categorization and optimistic-update math), `secureSession` (the two-tier SecureStore design), and `AuthContext` (login role-bounce, session persistence, logout) — plus a couple of component smoke tests. Screens under `src/app/` are intentionally not unit-tested (thin composition over the above, higher-effort to mock: react-query + expo-router + multiple contexts + gesture handlers).

## CI / Release

- `.github/workflows/ci.yml` — `mobile-test` job runs lint/typecheck/test/coverage plus an `expo export` bundle check (catches broken imports/JSX without needing Xcode or an Android SDK on the runner) on every PR/push to `master`.
- `.github/workflows/mobile-release.yml` — on push to `production` (when `mobile/**` changed), builds Android + iOS via EAS Build and publishes a tagged GitHub Release with the APK/IPA attached and auto-generated release notes. Needs an `EXPO_TOKEN` secret and, for iOS, a one-time local `eas credentials -p ios` run — see that workflow's header comment for details.

## Project layout

- `src/app/` — expo-router routes (`login`, `unlock`, `enroll-biometric`, `(app)/` protected group)
- `src/context/` — `AuthContext` (session/login/logout) and `BiometricLockContext` (app-lock)
- `src/auth/secureSession.ts` — two-tier `expo-secure-store` design backing the biometric lock
- `src/api/` — axios client + endpoint modules, typed against the backend's actual responses
- `src/theme/` — design tokens ported from `frontend/mentor/src/index.css`
- `src/components/`, `src/components/ui/` — screens/reusable UI ported from the web app's CSS classes
