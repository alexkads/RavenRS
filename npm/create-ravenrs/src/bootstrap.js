import { readFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { system } from './system.js';
import { banner, step, nextSteps, choices, summary, paint } from './ui.js';
const packageInfo = JSON.parse(await readFile(new URL('../package.json', import.meta.url), 'utf8'));
const release = JSON.parse(await readFile(new URL('../release.json', import.meta.url), 'utf8'));
const CLI_VERSION = release.cliVersion || release.version;
const REPO = 'https://github.com/alexkads/RavenRS';
const TARGETS = new Set(['desktop', 'web', 'both']);

export function parse(args) {
  const options = { yes: false, doctor: false, forwarded: [] };
  for (let i = 0; i < args.length; i++) {
    const arg = args[i];
    if (['--help', '-h', '--version', '-V'].includes(arg)) return { info: arg };
    if (arg === 'doctor' && !options.name) { options.doctor = true; continue; }
    if (arg === '--verbose') { options.verbose = true; continue; }
    if (arg === '--yes' || arg === '-y') { options.yes = true; options.forwarded.push('--yes'); continue; }
    if (['--target', '--template'].includes(arg)) {
      const value = args[++i];
      if (!value || value.startsWith('-')) throw new Error(`Valor ausente para ${arg}`);
      options[arg.slice(2)] = value;
      options.forwarded.push(arg, value);
    } else if (['--no-ai', '--no-git', '--no-check', '--plugin'].includes(arg)) {
      options.forwarded.push(arg);
    } else if (arg.startsWith('-')) {
      throw new Error(`Flag desconhecida: ${arg}`);
    } else if (!options.name && !options.doctor) { options.name = arg; }
    else { throw new Error(`Argumento inesperado: ${arg}`); }
  }
  if (options.target && !TARGETS.has(options.target)) throw new Error('Alvo: desktop, web ou both.');
  if (options.template && !['hello', 'app', 'app-sqlite', 'relatorios'].includes(options.template)) throw new Error('Modelo: hello, app, app-sqlite ou relatorios.');
  return options;
}

export function rustInstaller(platform, arch) {
  const cpu = { arm64: 'aarch64', x64: 'x86_64' }[arch];
  const os = { darwin: 'apple-darwin', linux: 'unknown-linux-gnu', win32: 'pc-windows-msvc' }[platform];
  if (!cpu || !os) throw new Error(`Sistema não suportado: ${platform}/${arch}. Veja https://rustup.rs`);
  return { url: `https://static.rust-lang.org/rustup/dist/${cpu}-${os}/rustup-init${platform === 'win32' ? '.exe' : ''}`, args: ['-y', '--no-modify-path', '--profile', 'minimal', '--default-toolchain', release.toolchain] };
}

async function consent(ctx, options, question) {
  if (options.yes) return;
  const accepted = ctx.confirm ? await ctx.confirm(question) : ['s', 'sim', 'y', 'yes'].includes((await ctx.ask(`${question} (s/N)`, 'n')).toLowerCase());
  if (!accepted) throw new Error('Instalação cancelada. Nenhuma próxima etapa foi executada.');
}

async function linuxDependencies(ctx, native) {
  if (ctx.platform !== 'linux' || !native) return true;
  const missing = ['x11', 'xcursor', 'xi', 'xrandr', 'xkbcommon', 'wayland-client', 'alsa', 'fontconfig'].filter(lib => !ctx.probe('pkg-config', ['--exists', lib]).ok);
  if (!ctx.probe('cc', ['--version']).ok) missing.push('C compiler');
  if (missing.length === 0) { ctx.log(step(4, 'Bibliotecas Linux', 'tudo certo', ctx.color)); return true; }
  const distro = await ctx.read('/etc/os-release');
  let install;
  if (/debian|ubuntu|linuxmint|pop/i.test(distro)) install = 'sudo apt-get install build-essential pkg-config libx11-dev libxcursor-dev libxi-dev libxrandr-dev libxkbcommon-dev libwayland-dev libasound2-dev libfontconfig1-dev mesa-vulkan-drivers';
  else if (/fedora|rhel|centos/i.test(distro)) install = 'sudo dnf install gcc gcc-c++ pkgconf-pkg-config libX11-devel libXcursor-devel libXi-devel libXrandr-devel libxkbcommon-devel wayland-devel alsa-lib-devel fontconfig-devel mesa-vulkan-drivers';
  else if (/arch|manjaro/i.test(distro)) install = 'sudo pacman -S --needed base-devel pkgconf libx11 libxcursor libxi libxrandr libxkbcommon wayland alsa-lib fontconfig vulkan-icd-loader';
  else if (/alpine/i.test(distro)) install = 'sudo apk add build-base pkgconf libx11-dev libxcursor-dev libxi-dev libxrandr-dev libxkbcommon-dev wayland-dev alsa-lib-dev fontconfig-dev mesa-vulkan-swrast';
  else install = 'Instale compilador C, pkg-config e os pacotes de desenvolvimento das bibliotecas acima pelo gerenciador da sua distribuição.';
  ctx.log(`[4/6] Dependências Linux ausentes: ${missing.join(', ')}\nExecute manualmente:\n  ${install}`);
  return false;
}

async function windowsTools(ctx, native) {
  if (ctx.platform !== 'win32' || !native) return true;
  const vswhere = join(ctx.env['ProgramFiles(x86)'] || 'C:\\Program Files (x86)', 'Microsoft Visual Studio', 'Installer', 'vswhere.exe');
  if (ctx.probe('cl.exe', []).ok || (await ctx.exists(vswhere) && ctx.probe(vswhere, ['-latest', '-products', '*', '-requires', 'Microsoft.VisualStudio.Component.VC.Tools.x86.x64', '-property', 'installationPath']).stdout.trim())) return true;
  ctx.log('[4/6] Instale Visual Studio Build Tools com Desktop development with C++ e Windows SDK:\n  https://visualstudio.microsoft.com/visual-cpp-build-tools/');
  return false;
}

/** Bootstrap can be tested with an injected OS boundary without executing installers. */
export async function bootstrap(args, ctx = system()) {
  const options = parse(args);
  if (options.info) {
    ctx.log(options.info.includes('version') || options.info === '-V' ? `create-ravenrs ${packageInfo.version} (CLI ${CLI_VERSION} · RavenRS ${release.version})` : 'Uso: npx create-ravenrs <nome> [--template hello|app|app-sqlite|relatorios] [--target desktop|web|both] [--yes] [--no-git] [--no-ai] [--no-check] [--plugin] [--verbose]\n     npx create-ravenrs doctor (somente leitura)');
    return;
  }
  ctx.verbose = Boolean(options.verbose);
  ctx.log(banner(packageInfo.version, ctx.color, options.doctor, ctx.columns));
  if (!options.name && !options.doctor) options.name = options.yes ? 'meu-app' : await ctx.ask('Qual é o nome do projeto?', 'meu-app');
  let existing;
  if (!options.doctor && await ctx.exists(resolve(ctx.cwd, options.name))) {
    const state = await ctx.read(join(resolve(ctx.cwd, options.name), '.ravenrs-project.toml'));
    const field = key => state.match(new RegExp(`^${key}\\s*=\\s*"([^"\\n]+)"\\s*(?:#.*)?$`, 'm'))?.[1];
    existing = { template: field('template'), target: field('target'), version: field('version') };
    if (!choices.template.some(item => item.value === existing.template) || !TARGETS.has(existing.target) || existing.version !== release.version) {
      throw new Error(`A pasta ${options.name} já existe sem metadados compatíveis. Use outra pasta; os arquivos existentes não foram alterados.`);
    }
  }
  if (!options.doctor) ctx.log(summary(options.name, options.template || existing?.template || (options.yes ? 'hello' : undefined), options.target || existing?.target || (options.yes ? 'desktop' : undefined), ctx.color));
  if (!options.doctor && !options.target) {
    options.target = options.yes ? existing?.target || 'desktop' : await ctx.select('Onde sua ideia vai rodar?', choices.target, existing?.target || 'desktop', 'alvo');
    if (!TARGETS.has(options.target)) throw new Error('Alvo inválido.');
    options.forwarded.push('--target', options.target);
  }
  if (!options.doctor && !options.template) {
    options.template = options.yes ? existing?.template || 'hello' : await ctx.select('Por onde você quer começar?', choices.template, existing?.template || 'hello', 'modelo');
    if (!choices.template.some(item => item.value === options.template)) throw new Error('Modelo inválido.');
    options.forwarded.push('--template', options.template);
  }
  if (existing && (existing.template !== options.template || existing.target !== options.target)) {
    throw new Error(`A pasta ${options.name} já contém ${existing.template}/${existing.target}; você escolheu ${options.template}/${options.target}. Use outro nome de pasta para criar esse modelo/alvo. O projeto existente não foi alterado.`);
  }
  // Doctor never prompts, downloads or installs, including rustup's automatic installs.
  if (options.doctor) {
    const state = await ctx.read(join(ctx.cwd, '.ravenrs-project.toml'));
    options.target = state.match(/^target = "([^"]+)"/m)?.[1];
  }
  const native = options.target !== 'web';
  // The CLI itself needs the native MSVC linker even for a web-only project.
  // Check before rustup-init so it cannot offer to install Build Tools itself.
  if (!await windowsTools(ctx, true)) throw new Error('Prepare as dependências do sistema e rode o comando de novo. Build Tools não foram instaladas.');
  let rustup = ctx.probe('rustup', ['--version']).ok;
  const rust = ctx.probe('rustc', ['--version']).ok && ctx.probe('cargo', ['--version']).ok;
  ctx.log(step(1, 'Rust', rust ? 'encontrado' : 'vamos preparar o ambiente', ctx.color));
  if (!rust && !rustup) {
    if (options.doctor) throw new Error('Rust não instalado. Veja https://rustup.rs');
    const installer = rustInstaller(ctx.platform, ctx.arch);
    ctx.log(`Instalador oficial: ${installer.url}\nEquivalente documentado: ${ctx.platform === 'win32' ? 'https://rustup.rs' : 'curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh'}`);
    await consent(ctx, options, 'Instalar Rust com rustup?');
    await ctx.installRust(installer);
    rustup = ctx.probe('rustup', ['--version']).ok;
    if (!rustup) throw new Error('rustup não encontrado após a instalação.');
  }
  if (!rustup) throw new Error('Use uma instalação gerenciada por rustup para respeitar o rust-toolchain.toml do modelo: https://rustup.rs');
  const toolchains = ctx.probe('rustup', ['toolchain', 'list']);
  if (!toolchains.stdout.split('\n').some(line => line.startsWith(`${release.toolchain}-`) || line.startsWith(`${release.toolchain} `))) {
    if (options.doctor) throw new Error(`Toolchain ${release.toolchain} ausente.`);
    await consent(ctx, options, `Instalar toolchain ${release.toolchain}?`);
    await ctx.run('rustup', ['toolchain', 'install', release.toolchain, '--profile', 'minimal'], undefined, 'Preparando a toolchain Rust');
  }
  ctx.env.RUSTUP_TOOLCHAIN = release.toolchain;
  ctx.log(step(2, 'Toolchain', `${release.toolchain} · mesma versão do modelo`, ctx.color));
  const installedComponents = ctx.probe('rustup', ['component', 'list', '--installed', '--toolchain', release.toolchain]);
  const missingComponents = release.components.filter(component => !installedComponents.stdout.split('\n').some(line => line.startsWith(`${component}-`) || line.startsWith(`${component} `)));
  if (missingComponents.length) {
    if (options.doctor) throw new Error(`Componentes do modelo ausentes: ${missingComponents.join(', ')}`);
    await consent(ctx, options, `Instalar componentes do modelo: ${missingComponents.join(', ')}?`);
    await ctx.run('rustup', ['component', 'add', ...missingComponents, '--toolchain', release.toolchain], undefined, 'Preparando rustfmt e Clippy');
  }
  if (options.target !== 'desktop' && !options.doctor) {
    const targets = ctx.probe('rustup', ['target', 'list', '--installed', '--toolchain', release.toolchain]);
    if (!targets.stdout.split(/\s+/).includes('wasm32-unknown-unknown')) {
      await consent(ctx, options, 'Instalar o alvo wasm32-unknown-unknown?');
      await ctx.run('rustup', ['target', 'add', 'wasm32-unknown-unknown', '--toolchain', release.toolchain], undefined, 'Adicionando suporte ao navegador');
    }
  }
  const cli = ctx.probe('cargo', ['ravenrs', '--version']);
  const installed = cli.ok ? cli.stdout.trim().split(/\s+/).at(-1) : null;
  if (installed !== CLI_VERSION) {
    if (options.doctor) throw new Error(`cargo-ravenrs: ${installed || 'ausente'}; esperado ${CLI_VERSION}.`);
    await consent(ctx, options, installed ? `Atualizar cargo-ravenrs de ${installed} para ${CLI_VERSION}?` : `Instalar cargo-ravenrs ${CLI_VERSION}? (a primeira compilação demora alguns minutos)`);
    await ctx.run('cargo', ['install', '--git', REPO, '--tag', release.tag, '--locked', ...(installed ? ['--force'] : []), 'cargo-ravenrs'], undefined, 'Compilando e instalando o CLI RavenRS');
    const installedNow = ctx.probe('cargo', ['ravenrs', '--version']);
    if (!installedNow.ok || installedNow.stdout.trim().split(/\s+/).at(-1) !== CLI_VERSION) throw new Error('cargo-ravenrs instalado não corresponde à versão do CLI registrada em release.json.');
  }
  ctx.log(step(3, 'cargo-ravenrs', CLI_VERSION, ctx.color));
  const libraries = await linuxDependencies(ctx, native);
  if (ctx.platform === 'win32') ctx.log(step(4, 'Build Tools', 'MSVC encontrado', ctx.color));
  if (!libraries) throw new Error('Prepare as dependências do sistema e rode o comando de novo. Nenhum comando de administrador foi executado.');
  if (ctx.platform !== 'linux' && ctx.platform !== 'win32') ctx.log(step(4, 'Sistema', 'tudo certo', ctx.color));
  const root = options.doctor ? ctx.cwd : resolve(ctx.cwd, options.name);
  if (!options.doctor) {
    ctx.log(step(5, 'Seu projeto', `${options.name} · código${options.forwarded.includes('--no-ai') ? '' : ' e ferramentas de IA'}`, ctx.color));
    await ctx.run('cargo', ['ravenrs', 'new', options.name, ...options.forwarded], undefined, 'Criando e validando sua aplicação');
  } else {
    ctx.log(step(5, 'Seu projeto', 'verificação sem alterações', ctx.color));
  }
  ctx.log(step(6, 'Doctor', 'conferindo os últimos detalhes', ctx.color));
  await ctx.run('cargo', ['ravenrs', 'doctor'], root, 'Validando projeto e conexões MCP');
  if (ctx.logFile && options.verbose) ctx.log(`${paint('│', '2', ctx.color)}  ${paint('Log completo', '2', ctx.color)}  ${ctx.logFile}`);
  if (options.doctor) ctx.log(`\n  ${paint('✓ Tudo certo. Seu ambiente está pronto.', '1;32', ctx.color)}\n`);
  if (!options.doctor) ctx.log(nextSteps(options.name, options.target === 'web', ctx.color, !options.forwarded.includes('--no-ai')));
}
