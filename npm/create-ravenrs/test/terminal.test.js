import test from 'node:test';
import assert from 'node:assert/strict';
import { PassThrough } from 'node:stream';
import { select, activity } from '../src/terminal.js';
import { clean, banner } from '../src/ui.js';

function terminal(columns = 80) {
  const input = new PassThrough();
  input.isRaw = false;
  input.setRawMode = value => { input.isRaw = value; };
  const output = new PassThrough();
  output.columns = columns;
  let text = '';
  output.on('data', chunk => { text += chunk; });
  return { input, output, text: () => text };
}
const options = [{value:'no',label:'Não instalar'}, {value:'yes',label:'Instalar',hint:'Escolha explícita'}];

test('arrow selection confirms the actual value and restores input and cursor', async () => {
  const t = terminal();
  const result = select({...t, question:'Continuar?',options,initial:'no',color:true});
  assert.equal(t.input.isRaw, true);
  t.input.write('\u001b[B');
  t.input.write('\r');
  assert.equal(await result, 'yes');
  assert.equal(t.input.isRaw, false);
  assert.equal(t.input.listenerCount('keypress'), 0);
  assert.equal(t.output.listenerCount('resize'), 0);
  assert.ok(t.text().includes('\u001b[?25h'));
  assert.match(clean(t.text()), /Continuar\?[\s\S]*Instalar/);
});

test('Ctrl+C cancels and restores terminal state without resolving a choice', async () => {
  const t = terminal();
  const result = select({...t,question:'Continuar?',options,initial:'no'});
  t.input.write('\u0003');
  await assert.rejects(result, {code:'RAVENRS_CANCELLED'});
  assert.equal(t.input.isRaw, false);
  assert.equal(t.input.listenerCount('keypress'), 0);
  assert.ok(t.text().endsWith('\u001b[?25h'));
});

test('Escape cancels and resizing keeps selection while clipping long descriptions', async () => {
  const t = terminal(24);
  const result = select({...t,question:'Uma pergunta muito longa para uma tela pequena',options,initial:'yes'});
  t.output.columns=30;
  t.output.emit('resize');
  t.input.emit('keypress','', {name:'escape'});
  await assert.rejects(result,{code:'RAVENRS_CANCELLED'});
  for (const line of clean(t.text()).split('\n')) assert.ok([...line].length <= 28, line);
});

test('NO_COLOR presentation has cursor controls but no color escapes; plain activity emits none', async () => {
  const t = terminal();
  const result = select({...t,question:'Continuar?',options,initial:'no',color:false});
  t.input.write('\r');
  await result;
  assert.ok(!/\u001b\[[\d;]*m/.test(t.text()));
  const plain = terminal();
  const progress = activity(plain.output,'Instalando',{animated:false});
  progress.update('Compiling foo');
  progress.stop();
  assert.equal(plain.text(),'');
  assert.ok(!banner('1.2.3',false,false,30).includes('█'));
});
