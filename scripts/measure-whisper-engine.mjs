#!/usr/bin/env node
import { existsSync, readFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawn } from 'node:child_process';

const rootDir = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const args = parseArgs(process.argv.slice(2));
const modelPath = requiredArg(args, 'model');
const audioPath = requiredArg(args, 'audio');
const enginePath = args.engine ?? join(
  rootDir,
  'src-tauri',
  'target',
  'release',
  process.platform === 'win32' ? 'lpt-whisper-engine.exe' : 'lpt-whisper-engine'
);
const firstPartialBudgetMs = Number(args.firstPartialBudgetMs ?? 2000);
const finalFlushBudgetMs = Number(args.finalFlushBudgetMs ?? 1500);

if (!existsSync(enginePath)) {
  throw new Error(`lpt-whisper-engine not found: ${enginePath}`);
}
if (!existsSync(modelPath)) {
  throw new Error(`model not found: ${modelPath}`);
}

const wav = readPcm16MonoWav(audioPath);
const child = spawn(enginePath, [], { stdio: ['pipe', 'pipe', 'pipe'] });
const metrics = [];
const transcripts = [];
let stderr = '';
let stdoutPending = '';

child.stderr.setEncoding('utf8');
child.stderr.on('data', chunk => {
  stderr += chunk;
});
child.stdout.setEncoding('utf8');
child.stdout.on('data', chunk => {
  stdoutPending += chunk;
  drainStdoutLines(false);
});

writeJsonLine(child, { type: 'config', modelPath, language: 'auto', sampleRate: wav.sampleRate });
let seq = 0;
let timestampMs = 0;
const frameSamples = Math.round(wav.sampleRate * 0.75);
for (let offset = 0; offset < wav.samples.length; offset += frameSamples) {
  const frame = wav.samples.subarray(offset, Math.min(offset + frameSamples, wav.samples.length));
  writeJsonLine(child, {
    type: 'audio',
    stream: 'mic',
    seq,
    timestampMs,
    pcm16Base64: Buffer.from(frame.buffer, frame.byteOffset, frame.byteLength).toString('base64')
  });
  seq += 1;
  timestampMs += Math.round(frame.length * 1000 / wav.sampleRate);
}
writeJsonLine(child, { type: 'flush', stream: 'mic' });
writeJsonLine(child, { type: 'shutdown' });
child.stdin.end();

const status = await new Promise(resolveStatus => {
  child.on('exit', code => resolveStatus(code ?? 1));
});
if (status !== 0) {
  throw new Error(`lpt-whisper-engine exited with ${status}: ${stderr.trim()}`);
}
drainStdoutLines(true);

const firstPartial = metricValues(metrics, 'first_partial_latency_ms');
const finalFlush = metricValues(metrics, 'whisper_final_flush_ms');
if (firstPartial.length === 0) {
  throw new Error('no first_partial_latency_ms metric was emitted');
}
if (finalFlush.length === 0) {
  throw new Error('no whisper_final_flush_ms metric was emitted');
}

const firstPartialP95 = percentile(firstPartial, 0.95);
const finalFlushP95 = percentile(finalFlush, 0.95);
const result = {
  firstPartialP95,
  finalFlushP95,
  transcriptCount: transcripts.length,
  firstPartialBudgetMs,
  finalFlushBudgetMs
};
console.log(JSON.stringify(result, null, 2));

if (firstPartialP95 > firstPartialBudgetMs || finalFlushP95 > finalFlushBudgetMs) {
  process.exit(1);
}

function writeJsonLine(childProcess, value) {
  childProcess.stdin.write(`${JSON.stringify(value)}\n`);
}

function drainStdoutLines(finish) {
  while (stdoutPending.includes('\n')) {
    const newline = stdoutPending.indexOf('\n');
    const line = stdoutPending.slice(0, newline);
    stdoutPending = stdoutPending.slice(newline + 1);
    handleOutputLine(line);
  }
  if (finish && stdoutPending.trim().length > 0) {
    handleOutputLine(stdoutPending);
    stdoutPending = '';
  }
}

function handleOutputLine(line) {
  if (line.trim().length === 0) {
    return;
  }
  const output = JSON.parse(line);
  if (output.type === 'metric') {
    metrics.push(output);
  }
  if (output.type === 'transcript') {
    transcripts.push(output);
  }
}

function metricValues(items, name) {
  return items.filter(item => item.name === name).map(item => Number(item.value));
}

function percentile(values, ratio) {
  const sorted = [...values].sort((left, right) => left - right);
  const index = Math.min(sorted.length - 1, Math.ceil(sorted.length * ratio) - 1);
  return sorted[index];
}

function readPcm16MonoWav(path) {
  const data = readFileSync(path);
  if (data.toString('ascii', 0, 4) !== 'RIFF' || data.toString('ascii', 8, 12) !== 'WAVE') {
    throw new Error(`not a WAV file: ${path}`);
  }

  let offset = 12;
  let format = null;
  let pcm = null;
  while (offset + 8 <= data.length) {
    const id = data.toString('ascii', offset, offset + 4);
    const size = data.readUInt32LE(offset + 4);
    const body = offset + 8;
    if (id === 'fmt ') {
      format = {
        audioFormat: data.readUInt16LE(body),
        channels: data.readUInt16LE(body + 2),
        sampleRate: data.readUInt32LE(body + 4),
        bitsPerSample: data.readUInt16LE(body + 14)
      };
    }
    if (id === 'data') {
      pcm = data.subarray(body, body + size);
    }
    offset = body + size + (size % 2);
  }

  if (!format || !pcm) {
    throw new Error(`missing fmt or data chunk: ${path}`);
  }
  if (format.audioFormat !== 1 || format.channels !== 1 || format.bitsPerSample !== 16) {
    throw new Error('measurement WAV must be 16-bit PCM mono');
  }
  if (format.sampleRate !== 16000) {
    throw new Error('measurement WAV must be 16 kHz');
  }

  return {
    sampleRate: format.sampleRate,
    samples: new Int16Array(pcm.buffer, pcm.byteOffset, pcm.byteLength / 2)
  };
}

function parseArgs(rawArgs) {
  const parsed = {};
  for (let index = 0; index < rawArgs.length; index += 1) {
    const arg = rawArgs[index];
    if (!arg.startsWith('--')) {
      continue;
    }
    parsed[arg.slice(2)] = rawArgs[index + 1];
    index += 1;
  }
  return parsed;
}

function requiredArg(parsed, name) {
  if (!parsed[name]) {
    throw new Error(`missing --${name}`);
  }
  return parsed[name];
}
