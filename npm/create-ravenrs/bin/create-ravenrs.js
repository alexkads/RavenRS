#!/usr/bin/env node
import { bootstrap } from '../src/bootstrap.js';
import { failure } from '../src/ui.js';
try {
  await bootstrap(process.argv.slice(2));
} catch (error) {
  console.error(failure(error.message, Boolean(process.stderr.isTTY) && process.env.NO_COLOR === undefined && process.env.TERM !== 'dumb', error.code === 'RAVENRS_CANCELLED'));
  process.exitCode = error.code === 'RAVENRS_CANCELLED' ? 130 : 1;
}
