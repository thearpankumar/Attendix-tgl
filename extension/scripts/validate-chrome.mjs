#!/usr/bin/env node
// `web-ext lint` is a Firefox/AMO-oriented tool: run against the Chrome-only
// MV3 build, it always flags BACKGROUND_SERVICE_WORKER_NOFALLBACK (a
// "background.service_worker needs a background.scripts fallback for
// Firefox" advisory) — that fallback only matters if one manifest is meant
// to run on both browsers. This repo deliberately ships two separate
// manifests (this one is Chrome-only; extension/scripts/build.mjs builds the
// Firefox one separately), so that specific finding is expected and doesn't
// fail the build here. Any other finding still does.
import { execFileSync } from 'node:child_process';

const EXPECTED_CODE = 'BACKGROUND_SERVICE_WORKER_NOFALLBACK';

// web-ext lint exits non-zero whenever `errors > 0`, which is exactly the
// case we need to inspect here — the JSON report still lands on stdout, just
// attached to the thrown error instead of returned normally.
let raw;
try {
  raw = execFileSync(
    'npx',
    ['web-ext', 'lint', '--source-dir=.output/chrome-mv3', '--output=json'],
    { encoding: 'utf8' },
  );
} catch (err) {
  if (!err.stdout) throw err;
  raw = err.stdout;
}

const report = JSON.parse(raw);
const unexpected = report.errors.filter((e) => e.code !== EXPECTED_CODE);

if (unexpected.length > 0) {
  console.error('Unexpected chrome-mv3 lint error(s):');
  console.error(JSON.stringify(unexpected, null, 2));
  process.exit(1);
}

const expectedCount = report.errors.length - unexpected.length;
console.log(
  expectedCount > 0
    ? `chrome-mv3: 0 unexpected errors (${expectedCount} expected Firefox-fallback advisory ignored).`
    : 'chrome-mv3: 0 errors.',
);
