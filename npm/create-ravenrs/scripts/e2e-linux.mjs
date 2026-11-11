// Explicit integration test. Docker Desktop runs this in its Linux VM on macOS/Windows.
// A temporary Git fixture supplies the not-yet-released CLI tag; production URLs are unchanged.
import { mkdtemp, mkdir, cp, readFile, writeFile, rm, access } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFileSync, spawnSync } from 'node:child_process';
const npmRoot = fileURLToPath(new URL('../', import.meta.url));
const source = resolve(process.argv[2] || join(npmRoot, '../..'));
await access(join(source, 'crates/cargo-ravenrs/Cargo.toml'));
const temp = await mkdtemp(join(tmpdir(), 'ravenrs-e2e-'));
const image = `ravenrs-create-e2e:${Date.now()}`;
function run(command, args, cwd) {
  const p = spawnSync(command, args, { cwd, stdio: 'inherit' });
  if (p.error || p.status !== 0) throw p.error || new Error(`${command} failed: ${p.status}`);
}
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
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates git build-essential pkg-config libx11-dev libxcursor-dev libxi-dev libxrandr-dev libxkbcommon-dev libwayland-dev libasound2-dev libfontconfig1-dev mesa-vulkan-drivers && rm -rf /var/lib/apt/lists/*
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
npx --yes --package /package.tgz create-ravenrs shop --yes
PATH=/root/.cargo/bin:$PATH
export PATH
cd shop
cargo test --test screen
cd ..
before=$(stat -c %Y /root/.cargo/bin/cargo-ravenrs)
npx --yes --package /package.tgz create-ravenrs shop --yes
after=$(stat -c %Y /root/.cargo/bin/cargo-ravenrs)
test "$before" = "$after"
cd shop
npx --yes --package /package.tgz create-ravenrs doctor
printf '\\nE2E passed: no Rust initially; project checks, renders, doctor passes; rerun skips install.\\n'
`);
  run('docker', ['build', '-t', image, temp]);
  run('docker', ['run', '--rm', image]);
} finally {
  spawnSync('docker', ['image','rm',image], { stdio: 'ignore' });
  await rm(temp, { recursive: true, force: true });
}
