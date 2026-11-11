// Small terminal presentation layer; no packages, shell commands or ANSI in redirected logs.
const ESC = '\u001b[';
const paint = (value, code, color) => color ? `${ESC}${code}m${value}${ESC}0m` : value;

export function banner(version, color = false, doctor = false) {
  const title = 'R A V E N R S';
  const subtitle = doctor ? 'Um check-up para o seu próximo passo.' : 'Uma ideia. Um comando. Seu app em Rust.';
  const width = 52;
  const line = value => `│  ${value}${' '.repeat(Math.max(0, width - value.length - 2))}│`;
  return [
    '',
    paint(`╭${'─'.repeat(width)}╮`, '36', color),
    paint(line(title), '1;36', color),
    line(subtitle),
    paint(line(`create-ravenrs ${version}`), '2', color),
    paint(`╰${'─'.repeat(width)}╯`, '36', color),
    '',
  ].join('\n');
}

export function step(number, title, detail, color = false) {
  return `${paint(`  ${number}/6`, '1;36', color)}  ${paint(title, '1', color)}${detail ? ` ${paint('·', '2', color)} ${detail}` : ''}`;
}

export function nextSteps(name, web, color = false) {
  const commands = [
    `cd ${shellQuote(name)}`,
    ...(web ? ['cargo check --target wasm32-unknown-unknown'] : ['cargo run']),
    'claude  # ou code .',
  ];
  return [
    '',
    `  ${paint('✓ Seu projeto está pronto.', '1;32', color)}`,
    paint('  A próxima ideia é sua.', '2', color),
    '',
    ...commands.map(command => `    ${paint('$', '36', color)} ${paint(command, '1', color)}`),
    ...(web ? ['', paint('  Veja AGENTS.md para compilar e servir web/.', '2', color)] : []),
    '',
  ].join('\n');
}

function shellQuote(text) {
  return /^[\w./-]+$/.test(text) ? text : `'${text.replaceAll("'", "'\\''")}'`;
}

export function failure(message, color = false) {
  return `\n  ${paint('✕ Não foi possível concluir.', '1;31', color)}\n  ${message}\n`;
}
