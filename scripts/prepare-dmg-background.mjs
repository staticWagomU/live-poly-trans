#!/usr/bin/env bun

import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { deflateSync } from 'node:zlib';

const width = 760;
const height = 430;
const outputPath = join(process.cwd(), 'src-tauri/icons/dmg-background.png');
const pixels = Buffer.alloc(width * height * 4);

const font = {
  A: ['01110', '10001', '10001', '11111', '10001', '10001', '10001'],
  B: ['11110', '10001', '10001', '11110', '10001', '10001', '11110'],
  C: ['01111', '10000', '10000', '10000', '10000', '10000', '01111'],
  D: ['11110', '10001', '10001', '10001', '10001', '10001', '11110'],
  E: ['11111', '10000', '10000', '11110', '10000', '10000', '11111'],
  F: ['11111', '10000', '10000', '11110', '10000', '10000', '10000'],
  G: ['01111', '10000', '10000', '10111', '10001', '10001', '01111'],
  I: ['11111', '00100', '00100', '00100', '00100', '00100', '11111'],
  L: ['10000', '10000', '10000', '10000', '10000', '10000', '11111'],
  N: ['10001', '11001', '10101', '10011', '10001', '10001', '10001'],
  O: ['01110', '10001', '10001', '10001', '10001', '10001', '01110'],
  P: ['11110', '10001', '10001', '11110', '10000', '10000', '10000'],
  R: ['11110', '10001', '10001', '11110', '10100', '10010', '10001'],
  S: ['01111', '10000', '10000', '01110', '00001', '00001', '11110'],
  T: ['11111', '00100', '00100', '00100', '00100', '00100', '00100'],
  U: ['10001', '10001', '10001', '10001', '10001', '10001', '01110'],
  V: ['10001', '10001', '10001', '10001', '10001', '01010', '00100'],
  Y: ['10001', '10001', '01010', '00100', '00100', '00100', '00100'],
};

function mix(start, end, amount) {
  return start.map((channel, index) => Math.round(channel + (end[index] - channel) * amount));
}

function setPixel(x, y, color) {
  if (x < 0 || y < 0 || x >= width || y >= height) return;
  const index = (Math.floor(y) * width + Math.floor(x)) * 4;
  pixels[index] = color[0];
  pixels[index + 1] = color[1];
  pixels[index + 2] = color[2];
  pixels[index + 3] = color[3] ?? 255;
}

function blendPixel(x, y, color, alpha) {
  if (x < 0 || y < 0 || x >= width || y >= height) return;
  const index = (Math.floor(y) * width + Math.floor(x)) * 4;
  pixels[index] = Math.round(pixels[index] * (1 - alpha) + color[0] * alpha);
  pixels[index + 1] = Math.round(pixels[index + 1] * (1 - alpha) + color[1] * alpha);
  pixels[index + 2] = Math.round(pixels[index + 2] * (1 - alpha) + color[2] * alpha);
}

function fillCircle(cx, cy, radius, color, alpha) {
  for (let y = Math.floor(cy - radius); y <= Math.ceil(cy + radius); y += 1) {
    for (let x = Math.floor(cx - radius); x <= Math.ceil(cx + radius); x += 1) {
      const distance = Math.hypot(x - cx, y - cy);
      if (distance <= radius) {
        blendPixel(x, y, color, alpha * (1 - distance / radius * 0.45));
      }
    }
  }
}

function fillRoundedRect(x, y, w, h, radius, color) {
  for (let py = y; py < y + h; py += 1) {
    for (let px = x; px < x + w; px += 1) {
      const dx = Math.max(x + radius - px, 0, px - (x + w - radius));
      const dy = Math.max(y + radius - py, 0, py - (y + h - radius));
      if (Math.hypot(dx, dy) <= radius) setPixel(px, py, color);
    }
  }
}

function strokeRoundedRect(x, y, w, h, radius, color) {
  fillRoundedRect(x, y, w, h, radius, color);
  fillRoundedRect(x + 2, y + 2, w - 4, h - 4, radius - 2, [248, 250, 246, 238]);
}

function drawLine(x1, y1, x2, y2, thickness, color) {
  const steps = Math.max(Math.abs(x2 - x1), Math.abs(y2 - y1));
  for (let step = 0; step <= steps; step += 1) {
    const amount = step / steps;
    const x = x1 + (x2 - x1) * amount;
    const y = y1 + (y2 - y1) * amount;
    fillCircle(x, y, thickness / 2, color, 1);
  }
}

function fillTriangle(points, color) {
  const minX = Math.floor(Math.min(...points.map(([x]) => x)));
  const maxX = Math.ceil(Math.max(...points.map(([x]) => x)));
  const minY = Math.floor(Math.min(...points.map(([, y]) => y)));
  const maxY = Math.ceil(Math.max(...points.map(([, y]) => y)));
  const [a, b, c] = points;
  const area = (b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1]);

  for (let y = minY; y <= maxY; y += 1) {
    for (let x = minX; x <= maxX; x += 1) {
      const w1 = ((b[0] - x) * (c[1] - y) - (c[0] - x) * (b[1] - y)) / area;
      const w2 = ((c[0] - x) * (a[1] - y) - (a[0] - x) * (c[1] - y)) / area;
      const w3 = 1 - w1 - w2;
      if (w1 >= 0 && w2 >= 0 && w3 >= 0) setPixel(x, y, color);
    }
  }
}

function drawText(text, x, y, scale, color) {
  let cursor = x;
  for (const character of text.toUpperCase()) {
    if (character === ' ') {
      cursor += scale * 4;
      continue;
    }
    const glyph = font[character];
    if (!glyph) {
      cursor += scale * 4;
      continue;
    }
    glyph.forEach((row, rowIndex) => {
      [...row].forEach((cell, columnIndex) => {
        if (cell === '1') {
          for (let dy = 0; dy < scale; dy += 1) {
            for (let dx = 0; dx < scale; dx += 1) {
              setPixel(cursor + columnIndex * scale + dx, y + rowIndex * scale + dy, color);
            }
          }
        }
      });
    });
    cursor += scale * 6;
  }
}

for (let y = 0; y < height; y += 1) {
  for (let x = 0; x < width; x += 1) {
    const amount = (x / width) * 0.55 + (y / height) * 0.45;
    setPixel(x, y, [...mix([219, 242, 233], [250, 226, 193], amount), 255]);
  }
}

fillCircle(30, 18, 155, [43, 143, 127], 0.22);
fillCircle(720, 405, 170, [230, 103, 48], 0.18);
fillCircle(380, 12, 110, [255, 255, 255], 0.22);

drawText('DRAG LIVEPOLYTRANS', 124, 54, 5, [15, 43, 45, 255]);
drawText('TO APPLICATIONS', 190, 102, 5, [15, 43, 45, 255]);

strokeRoundedRect(70, 185, 210, 145, 28, [45, 82, 86, 255]);
strokeRoundedRect(480, 185, 210, 145, 28, [45, 82, 86, 255]);

drawText('LIVEPOLY', 116, 230, 4, [24, 56, 59, 255]);
drawText('TRANS', 145, 265, 4, [24, 56, 59, 255]);
drawText('APPLICATIONS', 507, 248, 3, [24, 56, 59, 255]);

drawLine(315, 255, 445, 255, 12, [14, 92, 86, 255]);
fillTriangle([[455, 255], [425, 232], [425, 278]], [14, 92, 86, 255]);

drawText('OPEN GUIDE FIRST', 218, 372, 3, [36, 63, 66, 255]);

function crc32(buffer) {
  let crc = 0xffffffff;
  for (const byte of buffer) {
    crc ^= byte;
    for (let bit = 0; bit < 8; bit += 1) {
      crc = (crc >>> 1) ^ (crc & 1 ? 0xedb88320 : 0);
    }
  }
  return (crc ^ 0xffffffff) >>> 0;
}

function chunk(type, data) {
  const typeBuffer = Buffer.from(type);
  const length = Buffer.alloc(4);
  length.writeUInt32BE(data.length);
  const checksum = Buffer.alloc(4);
  checksum.writeUInt32BE(crc32(Buffer.concat([typeBuffer, data])));
  return Buffer.concat([length, typeBuffer, data, checksum]);
}

const raw = Buffer.alloc((width * 4 + 1) * height);
for (let y = 0; y < height; y += 1) {
  const rowStart = y * (width * 4 + 1);
  raw[rowStart] = 0;
  pixels.copy(raw, rowStart + 1, y * width * 4, (y + 1) * width * 4);
}

const ihdr = Buffer.alloc(13);
ihdr.writeUInt32BE(width, 0);
ihdr.writeUInt32BE(height, 4);
ihdr[8] = 8;
ihdr[9] = 6;
ihdr[10] = 0;
ihdr[11] = 0;
ihdr[12] = 0;

mkdirSync(dirname(outputPath), { recursive: true });
writeFileSync(
  outputPath,
  Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', ihdr),
    chunk('IDAT', deflateSync(raw)),
    chunk('IEND', Buffer.alloc(0)),
  ]),
);

console.log(`Prepared DMG background: ${outputPath}`);
