import test from 'node:test';
import assert from 'node:assert/strict';
import { banner, step, nextSteps, failure } from '../src/ui.js';

test('redirected terminal presentation contains no ANSI escapes', () => {
  const output = [banner('27.1.22'), step(1, 'Rust', 'encontrado'), nextSteps('minha-loja', false), failure('Erro')].join('\n');
  assert.ok(!output.includes('\u001b'));
  assert.match(output, /R A V E N R S/);
  assert.match(output, /\$ cd minha-loja/);
  assert.match(output, /\$ cargo run/);
});

test('interactive output uses colors and gives the web command when appropriate', () => {
  const output = banner('27.1.22', true) + nextSteps('minha-loja', true, true);
  assert.ok(output.includes('\u001b['));
  assert.match(output, /wasm32-unknown-unknown/);
  assert.match(output, /AGENTS.md/);
});
