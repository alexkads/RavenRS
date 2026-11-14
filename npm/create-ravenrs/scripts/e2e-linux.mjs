// Explicit integration test. Docker Desktop runs this in its Linux VM on macOS/Windows.
// A temporary Git fixture supplies the not-yet-released CLI tag; production URLs are unchanged.
//
//   node scripts/e2e-linux.mjs [CHECKOUT] [--memory 4g] [--jobs 2] [--logs DIR] [--case TEMPLATE/TARGET ...]
//
// Each case runs in a container limited to --memory (4 GB by default, what a small machine or
// Docker Desktop gives), with the low-memory settings of the README: LLD, --jobs jobs and no debug
// info in dev and test. The GNU linker with full debug info was killed past that budget on
// app-sqlite/both (#220). The whole output of each case is kept in --logs, with the memory the
// container saw, so a linker killed by the system can still be read after the container is gone.
import { mkdtemp, mkdir, cp, readFile, writeFile, rm, access } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFileSync, spawnSync } from 'node:child_process';
const npmRoot = fileURLToPath(new URL('../', import.meta.url));
const args = process.argv.slice(2);
const option = (name, fallback) => {
  const at = args.indexOf(name);
  return at >= 0 ? args.splice(at, 2)[1] : fallback;
};
const memory = option('--memory', '4g');
const jobs = option('--jobs', '2');
const logs = resolve(option('--logs', join(tmpdir(), 'ravenrs-e2e-logs')));
const cases = [];
for (let at = args.indexOf('--case'); at >= 0; at = args.indexOf('--case')) cases.push(args.splice(at, 2)[1]);
if (!cases.length) cases.push('hello/both', 'app-sqlite/both', 'relatorios/both');
const source = resolve(args[0] || join(npmRoot, '../..'));
await access(join(source, 'crates/cargo-ravenrs/Cargo.toml'));
const temp = await mkdtemp(join(tmpdir(), 'ravenrs-e2e-'));
const image = `ravenrs-create-e2e:${Date.now()}`;
function run(command, args, cwd) {
  const p = spawnSync(command, args, { cwd, stdio: 'inherit' });
  if (p.error || p.status !== 0) throw p.error || new Error(`${command} failed: ${p.status}`);
}
await mkdir(logs, { recursive: true });
try {
  const files = execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard', '-z'], { cwd: source, maxBuffer: 32 * 1024 * 1024 }).toString().split('\0').filter(Boolean);
  for (const file of files) {
    const target = join(temp, 'snapshot', file);
    await mkdir(dirname(target), { recursive: true });
    await cp(join(source,file),target, { dereference: true });
  }
  run('npm', ['pack', '--pack-destination', temp], npmRoot);
  const { version } = JSON.parse(await readFile(join(npmRoot,'package.json'),'utf8'));
  const release = JSON.parse(await readFile(join(npmRoot,'release.json'),'utf8'));
  await writeFile(join(temp,'Dockerfile'), `FROM node:22-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates git build-essential pkg-config lld procps libx11-dev libxcursor-dev libxi-dev libxrandr-dev libxkbcommon-dev libwayland-dev libasound2-dev libfontconfig1-dev mesa-vulkan-drivers && rm -rf /var/lib/apt/lists/*
COPY snapshot /fixture
COPY create-ravenrs-${version}.tgz /package.tgz
COPY run.sh /run.sh
CMD ["sh", "/run.sh"]
`);
  await writeFile(join(temp,'run.sh'), `set -eu
! command -v rustc
! command -v cargo
cd /fixture
git init -q
git add .
git -c user.name=Fixture -c user.email=fixture@example.invalid commit -qm fixture
git tag ${release.tag}
git config --global url.file:///fixture.insteadOf https://github.com/alexkads/RavenRS
# Validate unpublished crates from this checkout; manifests still contain published versions.
mkdir -p /root/.cargo
node --input-type=module -e '
import { readdirSync, readFileSync, writeFileSync } from "node:fs";
let config = "[patch.crates-io]\\n";
for (const entry of readdirSync("/fixture/crates")) {
 const text = readFileSync("/fixture/crates/" + entry + "/Cargo.toml", "utf8");
 const name = text.match(/^name = "([^"]+)"/m)?.[1];
 if (name?.startsWith("ravenrs")) config += name + " = { path = \\"/fixture/crates/" + entry + "\\" }\\n";
}
writeFileSync("/root/.cargo/config.toml", config);
'
export CARGO_NET_GIT_FETCH_WITH_CLI=true
export CARGO_TARGET_DIR=/tmp/ravenrs-target
mkdir /work
cd /work
npx --yes --package /package.tgz create-ravenrs shop --template "$TEMPLATE" --target "$TARGET" --yes
PATH=/root/.cargo/bin:$PATH
export PATH
cd shop
# The low-memory settings of the README, in the project.
mkdir -p .cargo
printf '[build]\\njobs = %s\\n\\n[target.%s-unknown-linux-gnu]\\nrustflags = ["-C", "link-arg=-fuse-ld=lld"]\\n\\n[profile.dev]\\ndebug = 0\\n\\n[profile.test]\\ndebug = 0\\n' "$JOBS" "$(uname -m)" > .cargo/config.toml
free -m
cargo test --test screen
cd ..
before=$(stat -c %Y /root/.cargo/bin/cargo-ravenrs)
npx --yes --package /package.tgz create-ravenrs shop --template "$TEMPLATE" --target "$TARGET" --yes
after=$(stat -c %Y /root/.cargo/bin/cargo-ravenrs)
test "$before" = "$after"
cd shop
npx --yes --package /package.tgz create-ravenrs doctor
printf '\\nE2E passed (%s/%s): no Rust initially; project checks, renders, doctor passes; rerun skips install.\\n' "$TEMPLATE" "$TARGET"
`);
  run('docker', ['build', '-t', image, temp]);
  let failed = 0;
  for (const entry of cases) {
    const [template, target] = entry.split('/');
    const log = join(logs, `${template}-${target}.log`);
    console.log(`== ${entry} with ${memory} and ${jobs} jobs: ${log}`);
    const started = Date.now();
    const p = spawnSync('docker', ['run', '--rm', '--memory', memory, '--memory-swap', memory,
      '-e', `TEMPLATE=${template}`, '-e', `TARGET=${target}`, '-e', `JOBS=${jobs}`, image], { maxBuffer: 256 * 1024 * 1024 });
    const output = `${p.stdout ?? ''}${p.stderr ?? ''}`;
    await writeFile(log, output);
    const seconds = Math.round((Date.now() - started) / 1000);
    if (p.status === 0) {
      console.log(`   passed in ${seconds}s`);
    } else {
      failed += 1;
      console.log(`   FAILED in ${seconds}s (status ${p.status}); the end of the log:`);
      console.log(output.split('\n').slice(-20).map((line) => `   ${line}`).join('\n'));
      const killed = output.split('\n').filter((line) => /signal: 9|signal 9|Killed|out of memory/i.test(line));
      if (killed.length) console.log(`   killed by the system, likely out of memory:\n   ${killed.join('\n   ')}`);
    }
  }
  if (failed) throw new Error(`${failed} of ${cases.length} cases failed; logs in ${logs}`);
} finally {
  spawnSync('docker', ['image','rm',image], { stdio: 'ignore' });
  await rm(temp, { recursive: true, force: true });
}
