import { spawnSync } from 'node:child_process';
import { access, readFile, mkdtemp, writeFile, chmod, rm } from 'node:fs/promises';
import { constants } from 'node:fs';
import { homedir, tmpdir } from 'node:os';
import { delimiter, join } from 'node:path';
import { createInterface } from 'node:readline/promises';

export function system() {
  const env = { ...process.env, RUSTUP_AUTO_INSTALL: '0' };
  const cargoBin = join(env.CARGO_HOME || join(homedir(), '.cargo'), 'bin');
  // Do not require a terminal restart after rustup installation.
  env.PATH = `${cargoBin}${delimiter}${env.PATH || ''}`;
  return {
    platform: process.platform,
    arch: process.arch,
    cwd: process.cwd(),
    env,
    color: Boolean(process.stdout.isTTY) && process.env.NO_COLOR === undefined && process.env.TERM !== 'dumb',
    log: console.log,
    async read(path) { try { return await readFile(path, 'utf8'); } catch (e) { if (e.code === 'ENOENT') return ''; throw e; } },
    async exists(path) { try { await access(path, constants.F_OK); return true; } catch { return false; } },
    probe(command, args) {
      const p = spawnSync(command, args, { env, encoding: 'utf8', timeout: 20_000, windowsHide: true, shell: false });
      return { ok: !p.error && p.status === 0, stdout: p.stdout || '', stderr: p.stderr || '' };
    },
    run(command, args, cwd = process.cwd()) {
      const p = spawnSync(command, args, { cwd, env, stdio: 'inherit', shell: false });
      if (p.error) throw p.error;
      if (p.status !== 0) throw new Error(`${command} ${args.join(' ')} terminou com ${p.status ?? p.signal}`);
    },
    async ask(question, fallback) {
      if (!process.stdin.isTTY) throw new Error(`${question} Reexecute com --yes para autorizar instalações em modo não interativo.`);
      const rl = createInterface({ input: process.stdin, output: process.stdout });
      try { return (await rl.question(`${question} [${fallback}] `)).trim() || fallback; } finally { rl.close(); }
    },
    async installRust({ url, args }) {
      const dir = await mkdtemp(join(tmpdir(), 'ravenrs-rustup-'));
      try {
        const response = await fetch(url, { signal: AbortSignal.timeout(120_000) });
        if (!response.ok) throw new Error(`rustup: HTTP ${response.status}`);
        const installer = join(dir, process.platform === 'win32' ? 'rustup-init.exe' : 'rustup-init');
        await writeFile(installer, Buffer.from(await response.arrayBuffer()));
        await chmod(installer, 0o700);
        this.run(installer, args);
      } finally { await rm(dir, { recursive: true, force: true }); }
    },
  };
}
