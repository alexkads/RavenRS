import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, copyFile, writeFile, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';

test('packaging permits npm-only patches while rejecting stale Rust pins and incompatible versions', async () => {
  const root = await mkdtemp(join(tmpdir(), 'ravenrs-release-'));
  try {
    const pkg = join(root, 'npm/create-ravenrs');
    await mkdir(join(pkg, 'scripts'), { recursive: true });
    await copyFile(new URL('../scripts/prepare-release.mjs', import.meta.url), join(pkg, 'scripts/prepare-release.mjs'));
    await writeFile(join(root, 'Cargo.toml'), '[workspace.package]\nversion = "27.1.23"\n');
    await writeFile(join(root, 'rust-toolchain.toml'), 'channel = "stable"\ncomponents = ["rustfmt", "clippy"]\n');
    for (const name of ['LICENSE-MIT', 'LICENSE-APACHE']) await writeFile(join(root, name), `License fixture: ${name}`);
    const run = (...args) => spawnSync(process.execPath, [join(pkg, 'scripts/prepare-release.mjs'), ...args], {encoding:'utf8'});
    await writeFile(join(pkg, 'package.json'), JSON.stringify({version:'27.1.24'}));
    assert.equal(run().status, 0);
    assert.equal(JSON.parse(await readFile(join(pkg, 'package.json'))).version, '27.1.24');
    const release = JSON.parse(await readFile(join(pkg, 'release.json')));
    assert.equal(release.version, '27.1.23');
    assert.equal(release.tag, 'v27.1.23');
    assert.equal(run('--check').status, 0);
    await writeFile(join(pkg, 'release.json'), JSON.stringify({...release,tag:'v27.1.24'}));
    assert.notEqual(run('--check').status, 0);
    assert.equal(run().status, 0);
    for (const version of ['27.1.22', '27.2.24', '28.1.24']) {
      await writeFile(join(pkg, 'package.json'), JSON.stringify({version}));
      assert.notEqual(run('--check').status, 0, version);
    }
  } finally { await rm(root, {recursive:true,force:true}); }
});

test('public version reports the npm launcher and its pinned Rust release', async () => {
  const result = spawnSync(process.execPath, [new URL('../bin/create-ravenrs.js', import.meta.url).pathname, '--version'], {encoding:'utf8'});
  assert.equal(result.status, 0);
  const pkg = JSON.parse(await readFile(new URL('../package.json', import.meta.url)));
  const release = JSON.parse(await readFile(new URL('../release.json', import.meta.url)));
  assert.equal(result.stdout.trim(), `create-ravenrs ${pkg.version} (RavenRS ${release.version})`);
});
