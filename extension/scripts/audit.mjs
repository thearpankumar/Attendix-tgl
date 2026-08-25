#!/usr/bin/env node
// `npm audit` currently reports 4 high-severity findings in this workspace,
// all tracing back to the same two GitHub advisories for `image-size` (a
// transitive dependency of `addons-linter` -> `web-ext` -> `wxt`, this
// extension's build tool — never bundled into the built extension a browser
// loads). Both advisories list "Patched versions: None" and an affected
// range of "<=2.0.2" — i.e. every published image-size release, including
// old 1.x ones, is marked affected, so there is no version bump or
// `npm overrides` pin that clears this today (verified: `addons-linter`
// pins an exact `image-size` version, and overriding it to 1.2.1 still
// leaves `npm audit` reporting the same two advisories against the override
// too). `npm audit fix --force`'s only suggestion is downgrading `wxt`
// itself to 0.20.27 (a semver-major regression to the core build tool) —
// not a real fix.
//
// Rather than blanket-ignoring this step's exit code (which would also hide
// any NEW, unrelated vulnerability introduced later), this allowlists
// exactly these two known advisory IDs and fails on anything else.
//
// This allowlist doesn't self-update — nothing here notices when upstream
// finally ships a fix. Two things force that to surface instead of staying
// silent forever:
//   1. `reviewBy` on each entry: once that date passes, this script fails
//      again even if nothing else changed, so someone re-checks (rerun the
//      investigation above, then either delete the entry if fixed, or push
//      `reviewBy` out with the same reasoning if still unfixed).
//   2. If an allowlisted id stops appearing in `npm audit` output at all
//      (i.e. it's already fixed upstream and nobody's removed the entry
//      yet), this prints a note saying so — see the "already fixed" check
//      near the bottom.
import { execFileSync } from 'node:child_process';

const ALLOWED_ADVISORIES = new Map([
  ['GHSA-w3rx-r6r6-pgpr', { reviewBy: '2026-11-25', note: 'image-size: ICNS parser DoS — no fix available' }],
  ['GHSA-5p2g-fcmc-qvqq', { reviewBy: '2026-11-25', note: 'image-size: JXL/HEIF parsers DoS — no fix available' }],
]);

// npm audit exits non-zero whenever it finds anything — the JSON report
// still lands on stdout, just attached to the thrown error.
let raw;
try {
  raw = execFileSync('npm', ['audit', '--json'], { encoding: 'utf8' });
} catch (err) {
  if (!err.stdout) throw err;
  raw = err.stdout;
}

const report = JSON.parse(raw);
const seen = new Set();
const unexpected = [];
const expired = [];
const today = new Date().toISOString().slice(0, 10);

for (const vuln of Object.values(report.vulnerabilities ?? {})) {
  if (vuln.severity !== 'high' && vuln.severity !== 'critical') continue;
  for (const via of vuln.via) {
    if (typeof via !== 'object' || !via.url) continue; // string entries just name a downstream package, not their own advisory
    const id = via.url.split('/').pop();
    const allowed = ALLOWED_ADVISORIES.get(id);
    if (!allowed) {
      unexpected.push({ id, title: via.title, severity: vuln.severity, url: via.url });
    } else if (today > allowed.reviewBy) {
      expired.push({ id, reviewBy: allowed.reviewBy, note: allowed.note });
    } else {
      seen.add(id);
    }
  }
}

if (unexpected.length > 0) {
  console.error('Unexpected high/critical severity advisory(ies) found — not on the known/allowed list:');
  console.error(JSON.stringify(unexpected, null, 2));
  console.error(
    '\nIf this is a genuinely new, real finding, fix it (or if it is a false positive / another confirmed-unfixable case, add its GHSA id to ALLOWED_ADVISORIES in this file with the same reasoning as the existing entries).',
  );
}

if (expired.length > 0) {
  console.error('Allowlisted advisory(ies) past their review date — re-verify, then either fix, remove the entry, or push reviewBy out:');
  console.error(JSON.stringify(expired, null, 2));
}

if (unexpected.length > 0 || expired.length > 0) {
  process.exit(1);
}

for (const [id, { note }] of ALLOWED_ADVISORIES) {
  if (!seen.has(id)) {
    console.log(`Note: allowlisted advisory ${id} (${note}) no longer appears in npm audit output — likely fixed upstream. Consider removing this entry.`);
  }
}

console.log(
  seen.size > 0
    ? `npm audit: 0 unexpected high/critical findings (${[...seen].sort().join(', ')} already known and tracked — see this script's header comment).`
    : 'npm audit: 0 high/critical findings.',
);
