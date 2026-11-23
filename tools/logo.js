'use strict';

/**
 * 渲染 logo：多尺寸 PNG + 展示图
 * 用法：electron tools/logo.js
 */
const path = require('node:path');
const fs = require('node:fs');
const { app, BrowserWindow } = require('electron');

app.disableHardwareAcceleration();

const ROOT = path.join(__dirname, '..');
const BUILD = path.join(ROOT, 'build');
const ICONS = path.join(BUILD, 'icons');
const SIZES = [16, 20, 24, 32, 40, 48, 64, 96, 128, 256];
const LOG = path.join(BUILD, 'logo-log.txt');

const log = (m) => {
  try {
    fs.mkdirSync(BUILD, { recursive: true });
    fs.appendFileSync(LOG, `${new Date().toISOString()} ${m}\n`);
  } catch { /* ignore */ }
};

process.on('uncaughtException', (e) => log('UNCAUGHT ' + ((e && e.stack) || e)));
process.on('unhandledRejection', (e) => log('UNHANDLED ' + ((e && e.stack) || e)));

app.whenReady().then(async () => {
  log('ready');
  const win = new BrowserWindow({
    show: false,
    width: 900,
    height: 600,
    webPreferences: { backgroundThrottling: false, offscreen: false }
  });
  win.webContents.on('console-message', (e) => {
    log('console ' + (e && e.message ? e.message : JSON.stringify(e)));
  });
  await win.loadFile(path.join(__dirname, 'logo.html'));
  log('loaded');

  const has = await win.webContents.executeJavaScript('typeof png');
  log('typeof png = ' + has);
  if (has !== 'function') throw new Error('page not ready');

  fs.mkdirSync(ICONS, { recursive: true });

  for (const size of SIZES) {
    const url = await win.webContents.executeJavaScript(`png(${size})`);
    const buf = Buffer.from(url.split(',')[1], 'base64');
    fs.writeFileSync(path.join(ICONS, `icon-${size}.png`), buf);
    log('wrote ' + size + ' ' + buf.length);
  }

  const big = await win.webContents.executeJavaScript('png(1024)');
  fs.writeFileSync(path.join(BUILD, 'icon.png'), Buffer.from(big.split(',')[1], 'base64'));
  log('wrote 1024');

  const show = await win.webContents.executeJavaScript('showcase()');
  fs.writeFileSync(path.join(BUILD, 'logo-showcase.png'), Buffer.from(show.split(',')[1], 'base64'));
  log('wrote showcase');

  const sheet = await win.webContents.executeJavaScript('sheet()');
  fs.writeFileSync(path.join(BUILD, 'icon-sheet.png'), Buffer.from(sheet.split(',')[1], 'base64'));
  log('wrote sheet');

  app.exit(0);
});
