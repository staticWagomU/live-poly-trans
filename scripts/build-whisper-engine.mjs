#!/usr/bin/env node
import { chmodSync, copyFileSync, existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const rootDir = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const tauriDir = join(rootDir, 'src-tauri');
const profile = process.env.LPT_WHISPER_ENGINE_PROFILE ?? 'release';
const isRelease = profile === 'release';
const executable = process.platform === 'win32' ? 'lpt-whisper-engine.exe' : 'lpt-whisper-engine';

const suffix = targetSuffix(process.platform, process.arch);
const outputName = process.platform === 'win32'
  ? `lpt-whisper-engine-${suffix}.exe`
  : `lpt-whisper-engine-${suffix}`;
const source = join(tauriDir, 'target', profile, executable);
const outputDir = join(tauriDir, 'binaries');
const output = join(outputDir, outputName);

mkdirSync(outputDir, { recursive: true });
if (!existsSync(output)) {
  writeFileSync(output, '');
  if (process.platform !== 'win32') {
    chmodSync(output, 0o755);
  }
}

const cargoArgs = [
  'build',
  '--manifest-path',
  join(tauriDir, 'Cargo.toml'),
  '--bin',
  'lpt-whisper-engine'
];
if (isRelease) {
  cargoArgs.push('--release');
}

const cargo = spawnSync('cargo', cargoArgs, { stdio: 'inherit' });
if (cargo.status !== 0) {
  process.exit(cargo.status ?? 1);
}

copyFileSync(source, output);
if (process.platform !== 'win32') {
  chmodSync(output, 0o755);
}

console.log(output);

function targetSuffix(platform, arch) {
  if (platform === 'darwin' && arch === 'arm64') {
    return 'aarch64-apple-darwin';
  }
  if (platform === 'darwin' && arch === 'x64') {
    return 'x86_64-apple-darwin';
  }
  if (platform === 'win32' && arch === 'x64') {
    return 'x86_64-pc-windows-msvc';
  }
  if (platform === 'linux' && arch === 'x64') {
    return 'x86_64-unknown-linux-gnu';
  }

  throw new Error(`Unsupported platform for lpt-whisper-engine: ${platform}/${arch}`);
}
