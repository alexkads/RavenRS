// Keep the npm version and toolchain tied to the workspace release, never master at runtime.
import { readFile, writeFile } from 'node:fs/promises';
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
if (process.argv.includes('--check')) {
  if (pkg.version !== version || await readFile(releasePath, 'utf8') !== release) {
    throw new Error('Run npm run prepare-release before packaging this workspace release.');
  }
} else {
  pkg.version = version;
  await writeFile(packagePath, JSON.stringify(pkg, null, 2) + '\n');
  await writeFile(releasePath, release);
}
