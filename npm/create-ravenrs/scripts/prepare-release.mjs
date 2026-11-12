// One public version for framework, CLI, plugin and npm. Runtime always uses a tag.
import { readFile, writeFile, copyFile } from 'node:fs/promises';
const root = new URL('../../../', import.meta.url);
const manifest = await readFile(new URL('Cargo.toml', root), 'utf8');
const version = manifest.match(/\[workspace\.package\][\s\S]*?\nversion = "([^"]+)"/)[1];
const cli = await readFile(new URL('crates/cargo-ravenrs/Cargo.toml', root), 'utf8');
const explicitCLI = cli.match(/^version = "([^"]+)"/m)?.[1];
if (explicitCLI && explicitCLI !== version) throw new Error('The CLI must match the workspace release.');
const plugin = JSON.parse(await readFile(new URL('plugins/ravenrs/.claude-plugin/plugin.json', root), 'utf8'));
if (plugin.version !== version) throw new Error('The plugin must match the workspace release.');
const skillPath = new URL('plugins/ravenrs/skills/ravenrs/SKILL.md', root);
const skill = await readFile(skillPath, 'utf8');
const marker = `<!-- cargo-ravenrs ${version} -->`;
if (process.argv.includes('--check')) {
  if (!skill.includes(marker)) throw new Error('The skill must match the workspace release.');
} else {
  if (!/<!-- cargo-ravenrs [^>]+ -->/.test(skill)) throw new Error('Missing skill version marker.');
  await writeFile(skillPath, skill.replace(/<!-- cargo-ravenrs [^>]+ -->/, marker));
}
const toolchain = await readFile(new URL('rust-toolchain.toml', root), 'utf8');
const channel = toolchain.match(/channel = "([^"]+)"/)[1];
const components = JSON.parse(toolchain.match(/components = (\[[^\]]+\])/)[1]);
const release = JSON.stringify({ version, cliVersion: version, tag: `v${version}`, toolchain: channel, components }, null, 2) + '\n';
const packagePath = new URL('../package.json', import.meta.url);
const releasePath = new URL('../release.json', import.meta.url);
const pkg = JSON.parse(await readFile(packagePath, 'utf8'));
if (process.argv.includes('--check')) {
  for (const name of ['LICENSE-MIT', 'LICENSE-APACHE']) {
    if (await readFile(new URL(name, root), 'utf8') !== await readFile(new URL(`../${name}`, import.meta.url), 'utf8'))
      throw new Error('Run npm run prepare-release to refresh the package licenses.');
  }
  if (pkg.version !== version || await readFile(releasePath, 'utf8') !== release)
    throw new Error('Run npm run prepare-release to align npm and its Rust pins.');
} else {
  pkg.version = version;
  await writeFile(packagePath, JSON.stringify(pkg, null, 2) + '\n');
  await writeFile(releasePath, release);
  for (const name of ['LICENSE-MIT', 'LICENSE-APACHE'])
    await copyFile(new URL(name, root), new URL(`../${name}`, import.meta.url));
}
