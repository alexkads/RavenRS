// Pin the Rust release and toolchain. npm-only patches may advance independently.
import { readFile, writeFile, copyFile } from 'node:fs/promises';
const root = new URL('../../../', import.meta.url);
const manifest = await readFile(new URL('Cargo.toml', root), 'utf8');
const version = manifest.match(/\[workspace\.package\][\s\S]*?\nversion = "([^"]+)"/)[1];
const toolchain = await readFile(new URL('rust-toolchain.toml', root), 'utf8');
const channel = toolchain.match(/channel = "([^"]+)"/)[1];
const components = JSON.parse(toolchain.match(/components = (\[[^\]]+\])/)[1]);
const release = JSON.stringify({ version, tag: `v${version}`, toolchain: channel, components }, null, 2) + '\n';
const packagePath = new URL('../package.json', import.meta.url);
const releasePath = new URL('../release.json', import.meta.url);
const pkg = JSON.parse(await readFile(packagePath, 'utf8'));
const parseVersion = value => {
  const parts = /^(\d+)\.(\d+)\.(\d+)$/.exec(value);
  if (!parts) throw new Error(`Expected a stable X.Y.Z version, got ${value}`);
  return parts.slice(1).map(Number);
};
const [npmMajor, npmMinor, npmPatch] = parseVersion(pkg.version);
const [rustMajor, rustMinor, rustPatch] = parseVersion(version);
const compatible = npmMajor === rustMajor && npmMinor === rustMinor && npmPatch >= rustPatch;
if (process.argv.includes('--check')) {
  for (const name of ['LICENSE-MIT', 'LICENSE-APACHE']) {
    if (await readFile(new URL(name, root), 'utf8') !== await readFile(new URL(`../${name}`, import.meta.url), 'utf8'))
      throw new Error('Run npm run prepare-release to refresh the package licenses.');
  }
  if (!compatible || await readFile(releasePath, 'utf8') !== release) {
    throw new Error('Run npm run prepare-release before packaging this workspace release.');
  }
} else {
  if (!compatible) pkg.version = version;
  await writeFile(packagePath, JSON.stringify(pkg, null, 2) + '\n');
  await writeFile(releasePath, release);
  for (const name of ['LICENSE-MIT', 'LICENSE-APACHE'])
    await copyFile(new URL(name, root), new URL(`../${name}`, import.meta.url));
}
