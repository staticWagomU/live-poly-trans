#!/usr/bin/env node
import { execFile, spawn } from 'node:child_process';
import { mkdtemp, rm, mkdir, readFile, writeFile } from 'node:fs/promises';
import { homedir, tmpdir } from 'node:os';
import path from 'node:path';
import process from 'node:process';

const repoRoot = path.resolve(import.meta.dirname, '..');
const appBundle = path.join(
  repoRoot,
  'src-tauri',
  'target',
  'debug',
  'bundle',
  'macos',
  'LivePolyTrans.app'
);
const appExecutable = path.join(appBundle, 'Contents', 'MacOS', 'live-poly-trans');
const appDataDir = path.join(
  homedir(),
  'Library',
  'Application Support',
  'com.staticwagomu.live-poly-trans'
);
const recordingsDir = path.join(appDataDir, 'recordings');

function run(command, args, options = {}) {
  return new Promise((resolve) => {
    execFile(command, args, { encoding: 'utf8', ...options }, (error, stdout, stderr) => {
      resolve({
        ok: error === null,
        code: error?.code ?? 0,
        stdout: stdout.trim(),
        stderr: stderr.trim()
      });
    });
  });
}

function fail(message) {
  console.error(`verify-macos-runtime: ${message}`);
  process.exitCode = 1;
}

async function sleep(ms) {
  await new Promise((resolve) => setTimeout(resolve, ms));
}

async function livePolyTransWindows() {
  const swift = `
import CoreGraphics
import Foundation

let options = CGWindowListOption(arrayLiteral: .optionOnScreenOnly, .excludeDesktopElements)
let windows = CGWindowListCopyWindowInfo(options, kCGNullWindowID) as? [[String: Any]] ?? []
for window in windows {
  let owner = window[kCGWindowOwnerName as String] as? String ?? ""
  if owner.lowercased().contains("livepolytrans") || owner.lowercased().contains("live-poly-trans") {
    let number = window[kCGWindowNumber as String] ?? "?"
    let bounds = window[kCGWindowBounds as String] as? [String: Any] ?? [:]
    let layer = window[kCGWindowLayer as String] ?? "?"
    let alpha = window[kCGWindowAlpha as String] ?? "?"
    let x = bounds["X"] ?? "?"
    let y = bounds["Y"] ?? "?"
    let width = bounds["Width"] ?? "?"
    let height = bounds["Height"] ?? "?"
    print("window=\\(number) owner=\\(owner) layer=\\(layer) alpha=\\(alpha) x=\\(x) y=\\(y) width=\\(width) height=\\(height)")
  }
}
`;
  const result = await run('xcrun', ['swift', '-e', swift], {
    env: {
      ...process.env,
      SDKROOT: '',
      NIX_CFLAGS_COMPILE: '',
      NIX_LDFLAGS: '',
      DEVELOPER_DIR: '/Applications/Xcode.app/Contents/Developer'
    }
  });
  return result.stdout
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean);
}

async function waitForWindow(timeoutMs = 10_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const windows = await livePolyTransWindows();
    if (windows.length > 0) {
      return windows;
    }
    await sleep(250);
  }
  return [];
}

async function waitForExit(child, timeoutMs = 10_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (child.exitCode !== null || child.signalCode !== null) {
      return true;
    }
    await sleep(250);
  }
  return false;
}

async function main() {
  const stat = await run('test', ['-x', appExecutable]);
  if (!stat.ok) {
    fail(`debug app executable not found: ${appExecutable}`);
    console.error('Build it first with: bun tauri build --debug --bundles app --no-sign');
    return;
  }

  const tempRoot = await mkdtemp(path.join(tmpdir(), 'lpt-runtime-verify-'));
  const recordingId = `rec-quit-verify-${new Date().toISOString().replace(/[^0-9]/g, '').slice(0, 14)}`;
  const recordingDir = path.join(recordingsDir, recordingId);
  const metaPath = path.join(recordingDir, 'meta.json');
  let app = null;

  try {
    app = spawn(appExecutable, [], {
      cwd: repoRoot,
      stdio: ['ignore', 'pipe', 'pipe']
    });
    app.stdout.setEncoding('utf8');
    app.stderr.setEncoding('utf8');

    const windows = await waitForWindow();
    if (windows.length === 0) {
      fail('LivePolyTrans main window was not visible in CoreGraphics window list');
    } else {
      console.log(`Observed ${windows.length} LivePolyTrans window(s):`);
      for (const window of windows) {
        console.log(`  ${window}`);
      }
    }

    await mkdir(recordingDir, { recursive: true });
    await writeFile(
      metaPath,
      `${JSON.stringify({ id: recordingId, startedAt: '2026-08-05T19:10:00+09:00' }, null, 2)}\n`
    );

    const quitResult = await run('osascript', ['-e', 'tell application "LivePolyTrans" to quit']);
    if (!quitResult.ok) {
      fail(`AppleScript quit failed: ${quitResult.stderr || quitResult.stdout}`);
    }

    const exited = await waitForExit(app);
    if (!exited) {
      fail('LivePolyTrans did not exit after AppleScript quit');
      app.kill('TERM');
    }

    const meta = JSON.parse(await readFile(metaPath, 'utf8'));
    if (typeof meta.endedAt !== 'string' || meta.endedAt.length === 0) {
      fail(`quit cleanup did not set endedAt in ${metaPath}`);
    } else {
      console.log(`Quit cleanup set endedAt=${meta.endedAt}`);
    }
  } finally {
    if (app && app.exitCode === null && app.signalCode === null) {
      app.kill('TERM');
    }
    await rm(recordingDir, { recursive: true, force: true });
    await rm(tempRoot, { recursive: true, force: true });
  }
}

await main();
