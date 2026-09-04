import fs from 'node:fs';

const releasePath = '.github/workflows/release.yml';
const release = fs.readFileSync(releasePath, 'utf8');
const marker = '\n  publish-release:';
const markerIndex = release.indexOf(marker);
if (markerIndex < 0) {
  throw new Error('release workflow must contain an isolated publish-release job');
}

const buildJobs = release.slice(0, markerIndex);
const publisher = release.slice(markerIndex);
const writeGrants = release.match(/^[ \t]*contents:[ \t]*write[ \t]*$/gm) ?? [];

if (!/^permissions:\n  contents: read$/m.test(release)) {
  throw new Error('release workflow must default to contents: read');
}
if (writeGrants.length !== 1 || buildJobs.includes('contents: write')) {
  throw new Error('only the source-free publisher may receive contents: write');
}
if (!/^    permissions:\n      contents: write$/m.test(publisher)) {
  throw new Error('publish-release must declare its write grant explicitly');
}
if (publisher.includes('actions/checkout@')) {
  throw new Error('publish-release must not check out or execute repository source');
}

const checkoutBlocks = [...buildJobs.matchAll(/actions\/checkout@[0-9a-f]+[^\n]*\n((?:[^\n]*\n){0,5})/g)];
if (checkoutBlocks.length === 0) {
  throw new Error('release workflow contains no source checkout to validate');
}
for (const block of checkoutBlocks) {
  if (!/persist-credentials:[ \t]*false/.test(block[1])) {
    throw new Error('every release build checkout must disable persisted credentials');
  }
}

console.log(
  `Release workflow least privilege passed: ${checkoutBlocks.length} read-only checkouts, one source-free publisher.`,
);
