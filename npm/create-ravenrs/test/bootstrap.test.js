import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
const { version: VERSION } = JSON.parse(readFileSync(new URL('../release.json', import.meta.url), 'utf8'));
import { bootstrap, parse, rustInstaller } from '../src/bootstrap.js';

function machine(overrides = {}) {
  const state = { rust: true, rustup: true, toolchain: true, components: true, wasm: true, cli: VERSION, libs: true, runs: [], logs: [], installs: [], questions: [] , ...overrides };
  const ctx = {
    platform: state.platform || 'linux', arch: 'x64', cwd: '/work', env: {},
    log: s => state.logs.push(s),
    read: async path => path.endsWith('os-release') ? 'ID=debian' : '',
    exists: async () => false,
    select: async (question, options, initial) => initial,
    ask: async q => { state.questions.push(q); return state.answer || 'n'; },
    installRust: async installer => { state.installs.push(installer); state.rust = state.rustup = state.toolchain = true; },
    probe(command, args) {
      if (command === 'rustup' && args[0] === '--version') return { ok: state.rustup, stdout: '' };
      if (command === 'rustc' || command === 'cargo' && args[0] === '--version') return { ok: state.rust, stdout: 'rust 1.99' };
      if (command === 'rustup' && args[0] === 'toolchain') return { ok: true, stdout: state.toolchain ? 'stable-x86_64-unknown-linux-gnu (default)' : '' };
      if (command === 'rustup' && args[0] === 'component') return { ok: true, stdout: state.components ? 'rustfmt-x86_64-unknown-linux-gnu\nclippy-x86_64-unknown-linux-gnu' : '' };
      if (command === 'rustup' && args[0] === 'target') return { ok: true, stdout: state.wasm ? 'wasm32-unknown-unknown' : '' };
      if (command === 'cargo' && args[0] === 'ravenrs') return { ok: !!state.cli, stdout: state.cli ? `cargo ravenrs ${state.cli}\n` : '' };
      if (command === 'pkg-config' || command === 'cc') return { ok: state.libs, stdout: '' };
      return { ok: false, stdout: '' };
    },
    run(command, args, cwd) {
      state.runs.push({ command, args, cwd });
      if (command === 'cargo' && args[0] === 'install') state.cli = VERSION;
      if (command === 'rustup' && args[0] === 'target') state.wasm = true;
      if (command === 'rustup' && args[0] === 'component') state.components = true;
      if (command === 'rustup' && args[0] === 'toolchain') state.toolchain = true;
    },
  };
  return { state, ctx };
}

test('existing environment skips every installation, forwards flags and runs doctor in the project', async () => {
  const { state, ctx } = machine();
  await bootstrap(['my-app', '--yes', '--template', 'app-sqlite', '--target', 'both', '--no-git', '--plugin'], ctx);
  assert.equal(state.installs.length, 0);
  assert.equal(state.questions.length, 0);
  assert.deepEqual(state.runs, [
    { command: 'cargo', args: ['ravenrs', 'new', 'my-app', '--yes', '--template', 'app-sqlite', '--target', 'both', '--no-git', '--plugin'], cwd: undefined },
    { command: 'cargo', args: ['ravenrs', 'doctor'], cwd: '/work/my-app' },
  ]);
});

test('missing Rust is installed only with authorization and installed from the official host', async () => {
  const { state, ctx } = machine({ rust: false, rustup: false, components: false, wasm: false, cli: null });
  await bootstrap(['my-app', '--yes', '--target', 'web'], ctx);
  assert.equal(state.installs.length, 1);
  assert.match(state.installs[0].url, /^https:\/\/static\.rust-lang\.org\/rustup\/dist\//);
  assert.deepEqual(state.runs[0].args, ['component', 'add', 'rustfmt', 'clippy', '--toolchain', 'stable']);
  assert.deepEqual(state.runs[1].args, ['target', 'add', 'wasm32-unknown-unknown', '--toolchain', 'stable']);
  assert.deepEqual(state.runs[2].args, ['install', '--git', 'https://github.com/alexkads/RavenRS', '--tag', `v${VERSION}`, '--locked', 'cargo-ravenrs']);
  assert.equal(ctx.env.RUSTUP_TOOLCHAIN, 'stable');
});

test('declining Rust installation runs no installer or subsequent command', async () => {
  const { state, ctx } = machine({ rust: false, rustup: false });
  await assert.rejects(bootstrap(['app', '--target', 'desktop'], ctx), /cancelada/);
  assert.equal(state.questions.length, 1);
  assert.equal(state.installs.length, 0);
  assert.equal(state.runs.length, 0);
});

test('different CLI version asks before updating and respects refusal', async () => {
  const { state, ctx } = machine({ cli: '0.1.0' });
  await assert.rejects(bootstrap(['app', '--target', 'desktop'], ctx), /cancelada/);
  assert.match(state.questions[0], /Atualizar.*0\.1\.0/);
  assert.equal(state.runs.length, 0);
});

test('CLI updates use the matching tag and --force only after consent', async () => {
  const { state, ctx } = machine({ cli: '0.1.0', answer: 's' });
  await bootstrap(['app', '--target', 'desktop'], ctx);
  assert.ok(state.runs[0].args.includes('--force'));
  assert.ok(state.runs[0].args.includes(`v${VERSION}`));
});

test('missing toolchain is installed after consent without reinstalling rustup', async () => {
  const { state, ctx } = machine({ rust: false, toolchain: false });
  await bootstrap(['app', '--yes'], ctx);
  assert.equal(state.installs.length, 0);
  assert.deepEqual(state.runs[0].args, ['toolchain', 'install', 'stable', '--profile', 'minimal']);
});

test('doctor performs no installation, prompts or project generation', async () => {
  const { state, ctx } = machine();
  await bootstrap(['doctor'], ctx);
  assert.deepEqual(state.runs, [{ command: 'cargo', args: ['ravenrs', 'doctor'], cwd: '/work' }]);
  assert.equal(state.questions.length, 0);
  assert.equal(state.installs.length, 0);
});

test('doctor reports missing tools without installing them even with --yes', async () => {
  for (const absent of [{ rust: false, rustup: false }, { components: false }, { toolchain: false }, { cli: null }]) {
    const { state, ctx } = machine(absent);
    await assert.rejects(bootstrap(['doctor', '--yes'], ctx));
    assert.equal(state.runs.length, 0);
    assert.equal(state.installs.length, 0);
    assert.equal(state.questions.length, 0);
  }
});

test('Linux prerequisites are reported with a distro command and never run sudo', async () => {
  const { state, ctx } = machine({ libs: false });
  await assert.rejects(bootstrap(['app', '--yes'], ctx), /dependências do sistema/);
  assert.ok(state.logs.some(s => s.includes('sudo apt-get install')));
  assert.equal(state.runs.length, 0);
});

test('web projects skip native Linux dependencies', async () => {
  const { state, ctx } = machine({ libs: false });
  await bootstrap(['app', '--yes', '--target', 'web'], ctx);
  assert.equal(state.runs.length, 2);
});

test('Windows does not install build tools automatically', async () => {
  const { state, ctx } = machine({ platform: 'win32' });
  await assert.rejects(bootstrap(['app', '--yes'], ctx), /dependências do sistema/);
  assert.ok(state.logs.some(s => s.includes('https://visualstudio.microsoft.com/visual-cpp-build-tools/')));
  assert.equal(state.runs.length, 0);
});

test('Windows prerequisites are checked before any Rust installer, including web projects', async () => {
  const { state, ctx } = machine({ platform: 'win32', rust: false, rustup: false, cli: null });
  await assert.rejects(bootstrap(['app', '--yes', '--target', 'web'], ctx), /dependências do sistema/);
  assert.equal(state.installs.length, 0);
  assert.equal(state.runs.length, 0);
});

test('macOS and Windows rustup URLs follow the selected architecture', () => {
  assert.match(rustInstaller('darwin', 'arm64').url, /aarch64-apple-darwin\/rustup-init$/);
  assert.match(rustInstaller('win32', 'x64').url, /x86_64-pc-windows-msvc\/rustup-init\.exe$/);
  assert.throws(() => rustInstaller('linux', 'mips'));
});

test('parser rejects invalid requests before any side effect', () => {
  assert.throws(() => parse(['app', '--target', 'invalid']));
  assert.throws(() => parse(['app', '--template', 'invalid']));
  assert.throws(() => parse(['app', '--target']));
  assert.throws(() => parse(['app', '--bad']));
  assert.throws(() => parse(['app', 'extra']));
});

test('info requests have no side effects', async () => {
  const { state, ctx } = machine({ rust: false });
  await bootstrap(['--help'], ctx);
  await bootstrap(['--version'], ctx);
  assert.equal(state.runs.length, 0);
  assert.equal(state.installs.length, 0);
});

test('repeated bootstrap skips downloads and reinstalls', async () => {
  const { state, ctx } = machine({ rust: false, rustup: false, cli: null, wasm: false });
  await bootstrap(['app', '--yes', '--target', 'both'], ctx);
  state.runs = [];
  await bootstrap(['app', '--yes', '--target', 'both'], ctx);
  assert.equal(state.installs.length, 1);
  assert.equal(state.runs.length, 2);
  assert.equal(state.runs[0].args[1], 'new');
});

test('interactive choices happen before installation and are forwarded to the Rust CLI', async () => {
  const {state,ctx}=machine({cli:null});
  const selected=[];
  ctx.select=async(question,options)=>{
    assert.equal(state.runs.length,0);
    selected.push(question);
    return options.some(item=>item.value==='both')?'both':'app';
  };
  ctx.confirm=async()=>true;
  await bootstrap(['my-app'],ctx);
  assert.equal(selected.length,2);
  assert.ok(state.runs.at(-2).args.includes('app'));
  assert.ok(state.runs.at(-2).args.includes('both'));
});

test('asynchronous generation errors stop before doctor; verbose is not forwarded to Rust', async () => {
  const {state,ctx}=machine();
  const calls=[];
  ctx.run=async(command,args)=>{calls.push(args);await new Promise(resolve=>setTimeout(resolve,5));throw new Error('compile failed');};
  await assert.rejects(bootstrap(['app','--yes','--verbose'],ctx),/compile failed/);
  assert.equal(ctx.verbose,true);
  assert.equal(calls.length,1);
  assert.equal(calls[0][1],'new');
  assert.equal(state.runs.length,0);
  assert.ok(!parse(['app','--verbose']).forwarded.includes('--verbose'));
});
