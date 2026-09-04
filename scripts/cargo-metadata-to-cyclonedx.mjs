import fs from 'node:fs';

const [inputPath, outputPath] = process.argv.slice(2);
if (!inputPath || !outputPath) {
  throw new Error('usage: node cargo-metadata-to-cyclonedx.mjs <cargo-metadata.json> <rust.cdx.json>');
}

const metadata = JSON.parse(fs.readFileSync(inputPath, 'utf8'));
const packageById = new Map(metadata.packages.map((pkg) => [pkg.id, pkg]));
const bomRef = (pkg) => `pkg:cargo/${encodeURIComponent(pkg.name)}@${encodeURIComponent(pkg.version)}`;
const root = packageById.get(metadata.resolve?.root) ?? metadata.packages.find((pkg) => pkg.name === 'codex-usage-ledger');
if (!root) throw new Error('Cargo metadata does not identify the root package');

const components = metadata.packages
  .filter((pkg) => pkg.id !== root.id)
  .sort((left, right) => left.id.localeCompare(right.id))
  .map((pkg) => ({
    type: 'library',
    'bom-ref': bomRef(pkg),
    name: pkg.name,
    version: pkg.version,
    purl: bomRef(pkg),
    ...(pkg.license ? { licenses: [{ expression: pkg.license }] } : {}),
    ...(pkg.repository ? { externalReferences: [{ type: 'vcs', url: pkg.repository }] } : {}),
  }));

const dependencies = (metadata.resolve?.nodes ?? [])
  .map((node) => ({
    ref: bomRef(packageById.get(node.id)),
    dependsOn: node.deps
      .map((dependency) => packageById.get(dependency.pkg))
      .filter(Boolean)
      .map(bomRef)
      .sort(),
  }))
  .sort((left, right) => left.ref.localeCompare(right.ref));

const document = {
  bomFormat: 'CycloneDX',
  specVersion: '1.6',
  version: 1,
  metadata: {
    component: {
      type: 'application',
      'bom-ref': bomRef(root),
      name: root.name,
      version: root.version,
      purl: bomRef(root),
      ...(root.license ? { licenses: [{ expression: root.license }] } : {}),
    },
  },
  components,
  dependencies,
};

fs.writeFileSync(outputPath, `${JSON.stringify(document, null, 2)}\n`);
console.log(`Rust CycloneDX SBOM contains ${components.length} dependency components.`);
