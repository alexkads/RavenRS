import { spawn, spawnSync } from 'node:child_process';
import { access, readFile, mkdtemp, writeFile, chmod, rm, open } from 'node:fs/promises';
import { constants, writeSync } from 'node:fs';
import { homedir, tmpdir } from 'node:os';
import { delimiter, join } from 'node:path';
import { createInterface } from 'node:readline/promises';
import { activity, select, cancelled } from './terminal.js';
import { paint, clean } from './ui.js';

export function system() {
  const env = { ...process.env, RUSTUP_AUTO_INSTALL: '0' };
  const cargoBin = join(env.CARGO_HOME || join(homedir(), '.cargo'), 'bin');
  env.PATH = `${cargoBin}${delimiter}${env.PATH || ''}`;
  const color = Boolean(process.stdout.isTTY) && process.env.NO_COLOR === undefined && process.env.TERM !== 'dumb';
  const interactive = Boolean(process.stdin.isTTY && process.stdout.isTTY && process.stdin.setRawMode) && process.env.TERM !== 'dumb' && !process.env.CI && (process.stdout.columns || 80) >= 42 && (process.stdout.rows || 24) >= 12;
  let logFile;
  const ctx = {
    platform: process.platform,
    arch: process.arch,
    cwd: process.cwd(),
    env, color, interactive,
    columns: process.stdout.columns || 80,
    verbose: false,
    get logFile() { return logFile; },
    log: console.log,
    async read(path) { try { return await readFile(path, 'utf8'); } catch (e) { if (e.code === 'ENOENT') return ''; throw e; } },
    async exists(path) { try { await access(path, constants.F_OK); return true; } catch { return false; } },
    probe(command, args) {
      const p = spawnSync(command, args, { env, encoding: 'utf8', timeout: 20_000, windowsHide: true, shell: false });
      return { ok: !p.error && p.status === 0, stdout: p.stdout || '', stderr: p.stderr || '' };
    },
    async run(command, args, cwd = process.cwd(), label = 'Preparando seu ambiente') {
      if (!logFile) logFile = join(await mkdtemp(join(tmpdir(), 'ravenrs-install-')), 'installation.log');
      const file = await open(logFile, 'a', 0o600);
      await file.write(`\n$ ${command} ${args.map(clean).join(' ')}\n`);
      const progress = activity(process.stdout, label, { color, animated: interactive && !ctx.verbose });
      let tail = '';
      let interrupted = false;
      let killTimer;
      let child;
      const stop = (signal = 'SIGINT') => {
        if (!child?.pid) return;
        try {
          if (process.platform === 'win32') spawnSync('taskkill', ['/pid', String(child.pid), '/T', '/F'], { windowsHide: true, stdio: 'ignore' });
          else process.kill(-child.pid, signal);
        } catch (error) { if (error.code !== 'ESRCH') child.kill(signal); }
      };
      const interrupt = () => {
        interrupted = true;
        stop();
        killTimer ??= setTimeout(() => stop('SIGKILL'), 2000);
        killTimer.unref();
      };
      try {
        const status = await new Promise((resolve, reject) => {
          child = spawn(command, args, {
            cwd, env: { ...env, CARGO_TERM_COLOR: 'never' }, shell: false,
            detached: process.platform !== 'win32', windowsHide: true,
            stdio: ['ignore', 'pipe', 'pipe'],
          });
          process.on('SIGINT', interrupt);
          process.on('SIGTERM', interrupt);
          const receive = (stream, chunk) => {
            // A single append descriptor keeps output order without buffering all build logs.
            writeSync(file.fd, chunk);
            tail = (tail + chunk.toString()).slice(-12_000);
            const lines = clean(tail).split(/[\r\n]/).filter(line => line.trim());
            progress.update(lines.at(-1) || '');
            if (!interactive || ctx.verbose) stream.write(chunk);
          };
          child.stdout.setEncoding('utf8');
          child.stderr.setEncoding('utf8');
          child.stdout.on('data', chunk => { try { receive(process.stdout, chunk); } catch (error) { stop('SIGKILL'); reject(error); } });
          child.stderr.on('data', chunk => { try { receive(process.stderr, chunk); } catch (error) { stop('SIGKILL'); reject(error); } });
          child.once('error', reject);
          child.once('close', (code, signal) => resolve({ code, signal }));
        });
        if (interrupted) throw cancelled();
        if (status.code !== 0) throw new Error(`${command} terminou com ${status.code ?? status.signal}.\n${clean(tail).split('\n').slice(-12).join('\n')}\nLog completo: ${logFile}`);
        progress.stop(true);
      } catch (error) {
        progress.stop(false);
        if (error.code !== 'RAVENRS_CANCELLED' && !error.message.includes(logFile)) error.message += `\nLog completo: ${logFile}`;
        throw error;
      } finally {
        if (interrupted) stop('SIGKILL');
        clearTimeout(killTimer);
        process.removeListener('SIGINT', interrupt);
        process.removeListener('SIGTERM', interrupt);
        await file.close();
      }
    },
    async ask(question, fallback) {
      if (!process.stdin.isTTY) throw new Error(`${question} Reexecute com --yes para autorizar instalações em modo não interativo.`);
      const rl = createInterface({ input: process.stdin, output: process.stdout });
      try {
        const value = (await rl.question(`${paint('◆', '35', color)}  ${question} ${paint(`(${fallback})`, '2', color)} › `)).trim() || fallback;
        return value;
      } finally { rl.close(); }
    },
    async select(question, options, initial, tag) {
      if (interactive) return select({ input: process.stdin, output: process.stdout, question, options, initial, color, tag });
      console.log(options.map(option => `  ${option.value}: ${option.label} — ${option.hint || ''}`).join('\n'));
      return ctx.ask(question, initial);
    },
    async confirm(question) {
      if (interactive) return await ctx.select(question, [
        { value: 's', label: 'Sim, continuar', hint: question.includes('cargo-ravenrs') ? 'Compilação local · a primeira vez demora alguns minutos' : 'Preparar os requisitos e seguir com o projeto' },
        { value: 'n', label: 'Não, cancelar', hint: 'Encerrar antes da instalação' },
      ], 'n', 'setup') === 's';
      return ['s', 'sim', 'y', 'yes'].includes((await ctx.ask(`${question} (s/N)`, 'n')).toLowerCase());
    },
    async installRust({ url, args }) {
      const dir = await mkdtemp(join(tmpdir(), 'ravenrs-rustup-'));
      const controller = new AbortController();
      let interrupted = false;
      const interrupt = () => { interrupted = true; controller.abort(); };
      const timer = setTimeout(() => controller.abort(), 120_000);
      const progress = activity(process.stdout, 'Baixando o instalador oficial do Rust', {color, animated: interactive && !ctx.verbose});
      process.on('SIGINT', interrupt);
      process.on('SIGTERM', interrupt);
      try {
        const response = await fetch(url, { signal: controller.signal });
        if (!response.ok) throw new Error(`rustup: HTTP ${response.status}`);
        const installer = join(dir, process.platform === 'win32' ? 'rustup-init.exe' : 'rustup-init');
        await writeFile(installer, Buffer.from(await response.arrayBuffer()));
        await chmod(installer, 0o700);
        progress.stop(true);
        clearTimeout(timer);
        process.removeListener('SIGINT', interrupt);
        process.removeListener('SIGTERM', interrupt);
        await ctx.run(installer, args, undefined, 'Instalando o Rust');
      } catch (error) {
        progress.stop(false);
        throw interrupted ? cancelled() : error;
      } finally {
        clearTimeout(timer);
        process.removeListener('SIGINT', interrupt);
        process.removeListener('SIGTERM', interrupt);
        await rm(dir, { recursive: true, force: true });
      }
    },
  };
  return ctx;
}
