'use strict';

/* 前端与 Rust 主进程之间的桥。对上层暴露与之前 Electron preload 同名的 window.api。
   传输方式：JSON over window.ipc.postMessage；返回值通过 window.__ipcDone 回调。 */
(function () {
  const pending = new Map();
  const handlers = Object.create(null);
  let seq = 1;

  function invoke(cmd, args) {
    return new Promise((resolve, reject) => {
      const id = seq++;
      pending.set(id, { resolve, reject });
      try {
        window.ipc.postMessage(JSON.stringify({ id, cmd, args: args || {} }));
      } catch (err) {
        pending.delete(id);
        reject(err);
      }
    });
  }

  function on(name, fn) {
    (handlers[name] = handlers[name] || []).push(fn);
  }

  window.__ipcDone = function (id, res) {
    const p = pending.get(id);
    if (!p) return;
    pending.delete(id);
    p.resolve(res);
  };

  window.__ipcEvent = function (name, payload) {
    const list = handlers[name] || [];
    for (const fn of list) {
      try { fn(payload); } catch (err) { console.error(err); }
    }
  };

  function b64ToBytes(b64) {
    const bin = atob(b64);
    const out = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
    return out;
  }

  /** 大块字节转 base64：分段拼接，避免 String.fromCharCode 参数过多爆栈 */
  function bytesToB64(bytes) {
    const CH = 0x8000;
    let s = '';
    for (let i = 0; i < bytes.length; i += CH) {
      s += String.fromCharCode.apply(null, bytes.subarray(i, i + CH));
    }
    return btoa(s);
  }

  // 页面本身就跑在 http://app.localhost 上，这里必须用同源相对路径，
  // 写成 app://localhost/... 浏览器认不出来（图片会加载失败）。
  const fileUrl = (p) => '/file?p=' + encodeURIComponent(p || '');

  window.api = {
    fileUrl,
    info: (p) => invoke('info', { path: p }),
    readFile: async (p) => b64ToBytes(await invoke('readFile', { path: p })),
    pickFiles: () => invoke('pickFiles', {}),
    pickFolder: () => invoke('pickFolder', {}),
    listFolder: (dir) => invoke('listFolder', { dir }),
    ocrRun: (o) => invoke('ocrRun', { path: o.path, langs: o.langs }),
    copyText: (text) => invoke('copyText', { text }),
    saveText: (o) => invoke('saveText', o),
    saveImage: (o) => invoke('saveImage', o),
    /** 内容识别填充：像素与选区都以 base64 传送，返回填充后的 RGBA */
    inpaint: (o) => invoke('inpaint', o),
    bytesToB64,
    b64ToBytes,
    assocStatus: () => invoke('assocStatus', {}),
    assocMakeDefault: () => invoke('assocMakeDefault', {}),
    assocUnregister: () => invoke('assocUnregister', {}),
    quit: () => invoke('quit', {}),
    openDefaultApps: () => invoke('openDefaultApps', {}),
    onMenu: (fn) => on('menu', fn),
    onOpenPath: (fn) => on('open', fn),
    onDrop: (fn) => on('drop', fn),
    onDragOver: (fn) => on('dragover', fn),
    onDragLeave: (fn) => on('dragleave', fn),
    onOcrProgress: () => {},
    pathForFile: () => ''
  };

  const ready = () => {
    try { window.ipc.postMessage(JSON.stringify({ id: 0, cmd: '__ready', args: {} })); } catch (err) { /* ignore */ }
  };
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', ready);
  } else {
    ready();
  }
})();
