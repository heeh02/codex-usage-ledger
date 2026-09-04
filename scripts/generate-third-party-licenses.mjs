import fs from 'node:fs';
import path from 'node:path';

const [cargoMetadataPath, packageLockPath, outputPath] = process.argv.slice(2);
if (!cargoMetadataPath || !packageLockPath || !outputPath) {
  throw new Error(
    'usage: node generate-third-party-licenses.mjs <cargo-metadata.json> <package-lock.json> <output.txt>',
  );
}

const cargo = JSON.parse(fs.readFileSync(cargoMetadataPath, 'utf8'));
const npm = JSON.parse(fs.readFileSync(packageLockPath, 'utf8'));
const exceptions = JSON.parse(
  fs.readFileSync(new URL('./license-text-exceptions.json', import.meta.url), 'utf8'),
).map((entry) => ({ ...entry, regex: new RegExp(entry.pattern) }));
const workspaceMembers = new Set(cargo.workspace_members ?? []);
const packageLockRoot = path.dirname(path.resolve(packageLockPath));
const licenseName = /^(license|copying|notice|copyright)(?:[._-].*)?$/i;
const missing = [];
let notInstalled = 0;
const entries = [];

function licenseFiles(directory) {
  if (!fs.existsSync(directory)) return [];
  return fs
    .readdirSync(directory, { withFileTypes: true })
    .filter((entry) => entry.isFile() && licenseName.test(entry.name))
    .map((entry) => entry.name)
    .sort((left, right) => left.localeCompare(right, 'en'));
}

function addEntry(ecosystem, name, version, license, directory) {
  if (!fs.existsSync(directory)) {
    notInstalled += 1;
    return;
  }
  const files = licenseFiles(directory);
  const coordinate = `${ecosystem}:${name}@${version}`;
  const exception =
    files.length === 0
      ? exceptions.find((entry) => entry.license === license && entry.regex.test(coordinate))
      : undefined;
  if (files.length === 0) {
    if (!exception) missing.push(coordinate);
  }
  entries.push({ ecosystem, name, version, license, directory, files, exception });
}

for (const pkg of cargo.packages ?? []) {
  if (workspaceMembers.has(pkg.id)) continue;
  addEntry(
    'cargo',
    pkg.name,
    pkg.version,
    pkg.license ?? 'UNKNOWN',
    path.dirname(pkg.manifest_path),
  );
}

for (const [packagePath, pkg] of Object.entries(npm.packages ?? {})) {
  if (!packagePath || pkg.link) continue;
  addEntry(
    'npm',
    pkg.name ?? packagePath.replace(/^node_modules\//, ''),
    pkg.version ?? 'unknown',
    pkg.license ?? 'UNKNOWN',
    path.join(packageLockRoot, packagePath),
  );
}

entries.sort((left, right) =>
  `${left.ecosystem}:${left.name}@${left.version}`.localeCompare(
    `${right.ecosystem}:${right.name}@${right.version}`,
    'en',
  ),
);

if (missing.length > 0) {
  throw new Error(
    `installed dependencies without license text or a reviewed exception:\n${missing.join('\n')}`,
  );
}

const output = [
  'THIRD-PARTY LICENSE RECEIPT',
  '',
  'Generated from the locked Cargo and npm dependency trees.',
  'Paths from the build machine are intentionally omitted.',
  '',
];
for (const entry of entries) {
  output.push(
    '='.repeat(78),
    `${entry.ecosystem}:${entry.name}@${entry.version}`,
    `Declared license: ${entry.license}`,
    '='.repeat(78),
    '',
  );
  for (const file of entry.files) {
    const contents = fs
      .readFileSync(path.join(entry.directory, file), 'utf8')
      .replace(/\r\n?/g, '\n')
      .trimEnd();
    output.push(`--- ${file} ---`, contents, '');
  }
  if (entry.files.length === 0) {
    output.push(
      'No standalone license file was present in the installed package.',
      `Reviewed source: ${entry.exception.source}`,
      `Review note: ${entry.exception.reason}`,
      '',
    );
  }
}

fs.writeFileSync(outputPath, `${output.join('\n')}\n`, 'utf8');
console.log(
  `Third-party license receipt contains ${entries.length} installed dependency packages ` +
    `(${entries.filter((entry) => entry.files.length > 0).length} with license text, ` +
    `${entries.filter((entry) => entry.files.length === 0).length} reviewed declared-only; ` +
    `${notInstalled} lockfile-only platform packages excluded).`,
);
