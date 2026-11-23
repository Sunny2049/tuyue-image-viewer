'use strict';

/** 把 build/icons/*.png 打包成多尺寸 Windows ICO（PNG 内嵌，Vista+ 支持） */
const fs = require('node:fs');
const path = require('node:path');

const ICON_DIR = path.join(__dirname, '..', 'build', 'icons');
const OUT = path.join(__dirname, '..', 'build', 'icon.ico');
const SIZES = [16, 20, 24, 32, 40, 48, 64, 96, 128, 256];

const entries = [];
for (const size of SIZES) {
  const p = path.join(ICON_DIR, `icon-${size}.png`);
  if (!fs.existsSync(p)) {
    process.stderr.write(`缺少 ${p}，先运行 electron tools/logo.js\n`);
    process.exit(1);
  }
  entries.push({ size, buf: fs.readFileSync(p) });
}

const header = Buffer.alloc(6);
header.writeUInt16LE(0, 0);
header.writeUInt16LE(1, 2);
header.writeUInt16LE(entries.length, 4);

const dir = Buffer.alloc(16 * entries.length);
let offset = 6 + 16 * entries.length;
entries.forEach((e, i) => {
  const o = i * 16;
  dir.writeUInt8(e.size >= 256 ? 0 : e.size, o);
  dir.writeUInt8(e.size >= 256 ? 0 : e.size, o + 1);
  dir.writeUInt8(0, o + 2);
  dir.writeUInt8(0, o + 3);
  dir.writeUInt16LE(1, o + 4);
  dir.writeUInt16LE(32, o + 6);
  dir.writeUInt32LE(e.buf.length, o + 8);
  dir.writeUInt32LE(offset, o + 12);
  offset += e.buf.length;
});

fs.writeFileSync(OUT, Buffer.concat([header, dir, ...entries.map((e) => e.buf)]));
process.stdout.write(`ico ${entries.length} sizes, ${offset} bytes\n`);
