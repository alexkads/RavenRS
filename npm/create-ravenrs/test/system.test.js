import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile, rm } from 'node:fs/promises';
import { dirname } from 'node:path';
import { spawn } from 'node:child_process';
import { system } from '../src/system.js';

test('real subprocess is asynchronous and its complete stdout/stderr is saved', async () => {
  const ctx = system();
  let heartbeat = false;
  const timer=setTimeout(()=>{ heartbeat=true; },20);
  try {
    await ctx.run(process.execPath,['-e','console.log("BUILD_START");setTimeout(()=>console.error("BUILD_DONE"),80)']);
    assert.equal(heartbeat,true);
    const log=await readFile(ctx.logFile,'utf8');
    assert.match(log,/BUILD_START/);
    assert.match(log,/BUILD_DONE/);
  } finally { clearTimeout(timer); if(ctx.logFile) await rm(dirname(ctx.logFile),{recursive:true,force:true}); }
});

test('real failing child preserves diagnostic output and log path', async () => {
  const ctx=system();
  try {
    await assert.rejects(ctx.run(process.execPath,['-e','console.error("SPECIFIC_BUILD_ERROR");process.exit(7)']), error => {
      assert.match(error.message,/terminou com 7/);
      assert.match(error.message,/SPECIFIC_BUILD_ERROR/);
      assert.ok(error.message.includes(ctx.logFile));
      return true;
    });
    assert.match(await readFile(ctx.logFile,'utf8'),/SPECIFIC_BUILD_ERROR/);
  } finally { if(ctx.logFile) await rm(dirname(ctx.logFile),{recursive:true,force:true}); }
});

test('SIGINT cancels the installer and kills a descendant that ignores SIGINT', {skip:process.platform==='win32'}, async () => {
  const moduleURL=new URL('../src/system.js',import.meta.url).href;
  const grandchild='process.on("SIGINT",()=>{});console.log("GRANDCHILD_READY="+process.pid);setInterval(()=>{},1000)';
  const task=`const {spawn}=require('node:child_process');const g=spawn(process.execPath,['-e',${JSON.stringify(grandchild)}],{stdio:['ignore','pipe','ignore']});g.stdout.pipe(process.stdout);console.log("CHILD_PID="+process.pid);setInterval(()=>{},1000)`;
  const source=`import {system} from ${JSON.stringify(moduleURL)}; const c=system();try{await c.run(process.execPath,['-e',${JSON.stringify(task)}]);process.exitCode=9}catch(e){console.log("CANCEL_CODE="+e.code);console.log("LOG="+c.logFile);process.exitCode=e.code==='RAVENRS_CANCELLED'?130:1}`;
  const child=spawn(process.execPath,['--input-type=module','-e',source],{stdio:['ignore','pipe','pipe']});
  let output='';
  let pid,grandchildPid,interrupted=false;
  const timer=setTimeout(()=>child.kill('SIGKILL'),5000);
  child.stdout.on('data', chunk=>{
    output+=chunk;
    pid=Number(output.match(/CHILD_PID=(\d+)/)?.[1]) || pid;
    grandchildPid=Number(output.match(/GRANDCHILD_READY=(\d+)/)?.[1]) || grandchildPid;
    if(pid && grandchildPid && !interrupted){interrupted=true;child.kill('SIGINT');}
  });
  try {
    const code=await new Promise((resolve,reject)=>{child.on('close',resolve);child.on('error',reject);});
    assert.equal(code,130,output);
    assert.match(output,/CANCEL_CODE=RAVENRS_CANCELLED/);
    for(const descendant of [pid,grandchildPid]) {
      for(let attempt=0;attempt<30;attempt++){
        try{process.kill(descendant,0);}catch(error){if(error.code==='ESRCH')break;throw error;}
        await new Promise(resolve=>setTimeout(resolve,20));
      }
      assert.throws(()=>process.kill(descendant,0),{code:'ESRCH'});
    }
  } finally {
    clearTimeout(timer);
    const log=output.match(/LOG=(.+)/)?.[1];
    if(log) await rm(dirname(log.trim()),{recursive:true,force:true});
    for(const descendant of [pid,grandchildPid]) if(descendant) try{process.kill(descendant,'SIGKILL');}catch{}
  }
});
