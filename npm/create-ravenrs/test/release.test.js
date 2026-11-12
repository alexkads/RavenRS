import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, copyFile, writeFile, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';

test('packaging requires a unified workspace, CLI, plugin, npm and tag', async () => {
  const root = await mkdtemp(join(tmpdir(), 'ravenrs-release-'));
  try {
    const pkg = join(root, 'npm/create-ravenrs');
    await mkdir(join(pkg, 'scripts'), {recursive:true});
    await copyFile(new URL('../scripts/prepare-release.mjs', import.meta.url), join(pkg, 'scripts/prepare-release.mjs'));
    await writeFile(join(root, 'Cargo.toml'), '[workspace.package]\nversion = "27.1.27"\n');
    await writeFile(join(root, 'rust-toolchain.toml'), 'channel = "stable"\ncomponents = ["rustfmt", "clippy"]\n');
    for (const name of ['LICENSE-MIT','LICENSE-APACHE']) await writeFile(join(root,name), `License fixture: ${name}`);
    await mkdir(join(root,'crates/cargo-ravenrs'), {recursive:true});
    const cli = join(root,'crates/cargo-ravenrs/Cargo.toml');
    await writeFile(cli, '[package]\nversion.workspace = true\n');
    await mkdir(join(root,'plugins/ravenrs/.claude-plugin'), {recursive:true});
    const plugin = join(root,'plugins/ravenrs/.claude-plugin/plugin.json');
    await writeFile(plugin, JSON.stringify({version:'27.1.27'}));
    await mkdir(join(root,'plugins/ravenrs/skills/ravenrs'),{recursive:true});
    await writeFile(join(root,'plugins/ravenrs/skills/ravenrs/SKILL.md'),'<!-- cargo-ravenrs 27.1.27 -->\n');
    await writeFile(join(pkg,'package.json'),JSON.stringify({version:'27.1.26'}));
    const run = (...args) => spawnSync(process.execPath,[join(pkg,'scripts/prepare-release.mjs'),...args],{encoding:'utf8'});
    assert.equal(run().status,0);
    assert.equal(JSON.parse(await readFile(join(pkg,'package.json'))).version,'27.1.27');
    const release = JSON.parse(await readFile(join(pkg,'release.json')));
    assert.equal(release.version,'27.1.27');
    assert.equal(release.cliVersion,'27.1.27');
    assert.equal(release.tag,'v27.1.27');
    assert.equal(run('--check').status,0);
    for(const version of ['27.1.26','27.1.28','27.2.27']){
      await writeFile(join(pkg,'package.json'),JSON.stringify({version}));
      assert.notEqual(run('--check').status,0,version);
    }
    assert.equal(run().status,0);
    await writeFile(join(pkg,'release.json'),JSON.stringify({...release,tag:'cargo-ravenrs-v27.1.27'}));
    assert.notEqual(run('--check').status,0);
    assert.equal(run().status,0);
    await writeFile(cli,'[package]\nversion = "27.1.28"\n');
    assert.notEqual(run().status,0);
    await writeFile(cli,'[package]\nversion.workspace = true\n');
    await writeFile(plugin,JSON.stringify({version:'27.1.26'}));
    assert.notEqual(run('--check').status,0);
    await writeFile(plugin,JSON.stringify({version:'27.1.27'}));
    await writeFile(join(root,'plugins/ravenrs/skills/ravenrs/SKILL.md'),'<!-- cargo-ravenrs 27.1.26 -->\n');
    assert.notEqual(run('--check').status,0);
    assert.equal(run().status,0);
    await writeFile(join(pkg,'LICENSE-MIT'),'stale license');
    assert.notEqual(run('--check').status,0);
  } finally { await rm(root,{recursive:true,force:true}); }
});

test('public version reports the npm launcher and its pinned Rust release', async () => {
  const result = spawnSync(process.execPath, [new URL('../bin/create-ravenrs.js', import.meta.url).pathname, '--version'], {encoding:'utf8'});
  assert.equal(result.status, 0);
  const pkg = JSON.parse(await readFile(new URL('../package.json', import.meta.url)));
  const release = JSON.parse(await readFile(new URL('../release.json', import.meta.url)));
  assert.equal(result.stdout.trim(), `create-ravenrs ${pkg.version} (CLI ${release.cliVersion || release.version} · RavenRS ${release.version})`);
});
