import { emitKeypressEvents } from 'node:readline';
import { paint, clean, badge } from './ui.js';

export function cancelled() {
  const error = new Error('Nenhuma próxima etapa será executada.');
  error.code = 'RAVENRS_CANCELLED';
  return error;
}

// Separate from process streams so keyboard lifecycle and resizing can be tested.
export function select({ input, output, question, options, initial, color = false, tag = 'escolha' }) {
  return new Promise((resolve, reject) => {
    let selected = Math.max(0, options.findIndex(option => option.value === initial));
    let height = 0;
    const raw = input.isRaw;
    const flowing = input.readableFlowing;
    const width = () => Math.max(12, (output.columns || 80) - 2);
    const fit = text => {
      const chars = [...clean(text)];
      return chars.length <= width() ? chars.join('') : chars.slice(0, width() - 1).join('') + '…';
    };
    const erase = () => {
      if (height) output.write(`\u001b[${height}A\u001b[J`);
    };
    const draw = () => {
      erase();
      const lines = [`  ${badge(tag, color)}  ${paint(fit(question).slice(0, Math.max(10, width() - 13)), '1', color)}`];
      for (const [i, option] of options.entries()) {
        const active = i === selected;
        lines.push(paint(fit(`             ${active ? '●' : '○'} ${option.label}`), active ? '1;36' : '2', color));
        if (active && option.hint) lines.push(paint(fit(`               ${option.hint}`), '2', color));
      }
      lines.push(paint(fit('             ↑ ↓ escolher · enter confirmar · esc sair'), '2', color));
      height = lines.length;
      output.write(lines.join('\n') + '\n');
    };
    const finish = (error) => {
      input.removeListener('keypress', onKey);
      output.removeListener('resize', onResize);
      input.setRawMode(Boolean(raw));
      if (!flowing) input.pause();
      erase();
      output.write('\u001b[?25h');
      if (error) reject(error);
      else {
        output.write(`  ${badge(tag, color)}  ${paint(question, '1', color)}\n             ${paint(options[selected].label, '36', color)}\n\n`);
        resolve(options[selected].value);
      }
    };
    const onKey = (text, key = {}) => {
      if ((key.ctrl && key.name === 'c') || key.name === 'escape') return finish(cancelled());
      if (key.name === 'return') return finish();
      if (key.name === 'up') selected = (selected + options.length - 1) % options.length;
      else if (key.name === 'down') selected = (selected + 1) % options.length;
      else if (key.name === 'home') selected = 0;
      else if (key.name === 'end') selected = options.length - 1;
      else return;
      draw();
    };
    // Old wrapped rows cannot be erased reliably after a resize. Keep them as history.
    const onResize = () => { height = 0; draw(); };
    emitKeypressEvents(input);
    input.setRawMode(true);
    input.resume();
    input.on('keypress', onKey);
    output.on('resize', onResize);
    output.write('\u001b[?25l');
    draw();
  });
}

export function activity(output, label, { color = false, animated = false } = {}) {
  const start = Date.now();
  const frames = ['█░░░', '██░░', '███░', '░███', '░░██', '░░░█'];
  let tick = 0;
  let detail = '';
  let stopped = false;
  const draw = () => {
    const seconds = Math.floor((Date.now() - start) / 1000);
    const suffix = seconds ? ` · ${seconds}s` : '';
    const available = Math.max(10, (output.columns || 80) - 9);
    const text = [...`${label}${suffix}${detail ? ` · ${detail}` : ''}`].slice(0, available).join('');
    output.write(`\r\u001b[2K  ${paint(frames[tick++ % frames.length], '36', color)}  ${text}`);
  };
  if (animated) draw();
  const timer = animated ? setInterval(draw, 90) : null;
  return {
    update(text) { detail = clean(text).trim(); },
    stop(ok = true) {
      if (stopped) return;
      stopped = true;
      if (timer) clearInterval(timer);
      if (animated) {
        const seconds = ((Date.now() - start) / 1000).toFixed(1);
        output.write(`\r\u001b[2K             ${paint(ok ? '✓' : '■', ok ? '32' : '31', color)}  ${label} ${paint(`(${seconds}s)`, '2', color)}\n`);
      }
    },
  };
}
