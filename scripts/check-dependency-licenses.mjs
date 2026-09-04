import fs from 'node:fs';

const [cargoMetadataPath, packageLockPath] = process.argv.slice(2);
if (!cargoMetadataPath || !packageLockPath) {
  throw new Error('usage: node check-dependency-licenses.mjs <cargo-metadata.json> <package-lock.json>');
}

const cargo = JSON.parse(fs.readFileSync(cargoMetadataPath, 'utf8'));
const npm = JSON.parse(fs.readFileSync(packageLockPath, 'utf8'));
const missing = [];
const incompatible = [];
const permissiveAlternative = /MIT|Apache|BSD|ISC|Zlib|Unlicense|CC0/;
const restricted = /(?:^|[^A-Z])(AGPL|GPL|SSPL|BUSL)-/;

for (const pkg of cargo.packages ?? []) {
  if (!pkg.license) missing.push(`cargo:${pkg.name}@${pkg.version}`);
  else if (restricted.test(pkg.license) && !permissiveAlternative.test(pkg.license)) {
    incompatible.push(`cargo:${pkg.name}@${pkg.version}:${pkg.license}`);
  }
}

for (const [path, pkg] of Object.entries(npm.packages ?? {})) {
  if (!path || pkg.link) continue;
  const label = `npm:${pkg.name ?? path.replace(/^node_modules\//, '')}@${pkg.version ?? 'unknown'}`;
  if (!pkg.license) missing.push(label);
  else if (restricted.test(pkg.license) && !permissiveAlternative.test(pkg.license)) {
    incompatible.push(`${label}:${pkg.license}`);
  }
}

if (missing.length || incompatible.length) {
  if (missing.length) console.error(`Dependencies without license metadata:\n${missing.join('\n')}`);
  if (incompatible.length) console.error(`Dependencies requiring explicit license review:\n${incompatible.join('\n')}`);
  process.exit(1);
}

console.log(`Dependency license metadata passed: ${cargo.packages.length} Cargo packages, ${Object.keys(npm.packages).length - 1} npm packages.`);
