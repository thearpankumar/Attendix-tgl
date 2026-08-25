#!/usr/bin/env node
// An .xpi is just a zip with Firefox's native extension-package name — same
// byte format `wxt zip -b firefox` already produces, `wxt` just doesn't have
// a flag to name it .xpi directly. Firefox accepts either extension for
// `about:debugging -> Load Temporary Add-on`, but .xpi is the one it uses
// for its own installs/downloads, so this copies the freshly built
// *-firefox.zip to a matching *-firefox.xpi for anyone who wants that.
import { copyFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

const outputDir = '.output';
const zipName = readdirSync(outputDir)
  .filter((f) => f.endsWith('-firefox.zip'))
  .sort()
  .at(-1);

if (!zipName) {
  console.error(
    `No *-firefox.zip found in ${outputDir}/ — run "npm run zip:firefox" (or "wxt zip -b firefox") first.`,
  );
  process.exit(1);
}

const xpiName = zipName.replace(/\.zip$/, '.xpi');
copyFileSync(join(outputDir, zipName), join(outputDir, xpiName));
console.log(`${outputDir}/${xpiName}`);
