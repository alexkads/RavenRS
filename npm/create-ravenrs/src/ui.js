// RavenRS terminal identity. Node built-ins only; colors are optional.
const ESC = '\u001b[';
export const paint = (value, code, color) => color ? `${ESC}${code === '2' ? '38;5;245' : code}m${value}${ESC}0m` : value;
export const clean = value => String(value).replace(/\u001b\[[0-?]*[ -/]*[@-~]/g, '').replace(/[\x00-\x08\x0b-\x1f\x7f]/g, '');
export const badge = (value, color, tone = 99) => paint(` ${value.padEnd(7)} `, `1;38;5;231;48;5;${tone}`, color);
export const choices = {
  target: [
    { value: 'desktop', label: 'Desktop', hint: 'Janela nativa · macOS, Windows e Linux' },
    { value: 'web', label: 'Web', hint: 'No navegador · WebAssembly + GPU' },
    { value: 'both', label: 'Desktop + Web', hint: 'Uma interface Rust nos dois ambientes' },
  ],
  template: [
    { value: 'hello', label: 'Hello, RavenRS', hint: 'Comece pequeno · cartão, estado e botões' },
    { value: 'app', label: 'Aplicação', hint: 'Pronta para crescer · navegação, listas e formulários' },
    { value: 'app-sqlite', label: 'Aplicação + banco', hint: 'Dados persistidos · SQLite / IndexedDB' },
    { value: 'relatorios', label: 'Designer de relatórios', hint: 'Crie layouts e explore os relatórios RavenRS' },
  ],
};

export function banner(version, color = false, doctor = false, columns = 80) {
  const logo = ['█▀█ ▄▀█ █ █ █▀▀ █▄ █ █▀█ █▀', '█▀▄ █▀█ ▀▄▀ ██▄ █ ▀█ █▀▄ ▄█'];
  const gradient = text => [...text].map((letter, i) => paint(letter, `1;38;5;${[141, 141, 135, 99, 69, 75, 81][Math.min(6, Math.floor(i / 4))]}`, color)).join('');
  return [
    '',
    ...(columns >= 42 ? logo.map(line => `  ${gradient(line)}`) : [`  ${paint('R A V E N R S', '1;36', color)}`]),
    `  ${paint(`create-ravenrs v${version}`, '2', color)}`,
    '',
    `  ${badge('ravenrs', color, 63)}  ${paint(doctor ? 'Tudo em ordem para a sua próxima ideia.' : 'Sua próxima ideia. Rust. Em qualquer tela.', '1', color)}`,
    '',
  ].join('\n');
}

export function step(number, title, detail, color = false) {
  const tags = ['rust', 'tools', 'cli', 'sistema', 'projeto', 'mcp'];
  return `  ${badge(tags[number - 1], color)}  ${paint(title, '1', color)}${detail ? ` ${paint('·', '2', color)} ${detail}` : ''}`;
}

export function summary(name, template, target, color = false) {
  const model = choices.template.find(item => item.value === template)?.label;
  const platform = choices.target.find(item => item.value === target)?.label;
  const detail = [model, platform].filter(Boolean).join(' · ');
  return `  ${badge('dir', color)}  ${paint('Seu projeto', '1', color)}\n             ${clean(name)}${detail ? `  ${paint(detail, '2', color)}` : ''}\n`;
}

export function nextSteps(name, web, color = false, ai = true) {
  const commands = [`cd ${shellQuote(name)}`, ...(web ? ['cargo check --target wasm32-unknown-unknown'] : ['cargo run'])];
  return [
    '',
    `  ${badge('pronto', color, 29)}  ${paint('Seu projeto está pronto.', '1', color)}`,
    `             ${paint(ai ? 'Rust, código e contexto para sua IA. Tudo conectado.' : 'Seu projeto Rust está preparado.', '2', color)}`,
    '',
    ...commands.map(command => `             ${paint('$', '35', color)} ${paint(command, '1', color)}`),
    '',
    ...(ai ? [`             ${paint('Com sua IA', '1', color)}  ${paint('claude  ·  code .  ·  cursor .  ·  codex', '2', color)}`] : []),
    ...(web ? ['             AGENTS.md explica como compilar e servir web/.'] : []),
    `             ${paint('Docs', '1', color)}        https://ravenrs.vercel.app/docs/`,
    '',
    `  ${paint('Agora, a próxima ideia é sua.', '1;35', color)}`,
    '',
  ].join('\n');
}

function shellQuote(text) {
  return /^[\w./-]+$/.test(text) ? text : `'${text.replaceAll("'", "'\\''")}'`;
}

export function failure(message, color = false, cancelled = false) {
  return `\n  ${badge(cancelled ? 'pausa' : 'erro', color, cancelled ? 136 : 160)}  ${paint(cancelled ? 'Operação cancelada.' : 'Não foi possível concluir.', '1', color)}\n   ${clean(message)}\n`;
}
