'use strict';

/* TIFF / HEIC 等浏览器不能直接显示的格式，在这里解码成 canvas
   入参是文件字节（Uint8Array），避免自定义协议与 canvas 污染问题 */
(function () {
  const TIFF_EXT = new Set(['tif', 'tiff']);
  const HEIC_EXT = new Set(['heic', 'heif', 'hif']);

  const extOf = (p) => {
    const m = /\.([^./\\]+)$/.exec(p || '');
    return m ? m[1].toLowerCase() : '';
  };

  function canvasToUrl(canvas, type, quality) {
    return new Promise((resolve) => {
      canvas.toBlob(
        (blob) => (blob ? resolve(URL.createObjectURL(blob)) : resolve(canvas.toDataURL('image/png'))),
        type || 'image/png',
        quality
      );
    });
  }

  let libheifLoading = null;
  function loadLibheif() {
    if (libheifLoading) return libheifLoading;
    libheifLoading = new Promise((resolve, reject) => {
      const s = document.createElement('script');
      s.src = 'vendor/libheif-bundle.js';
      s.onload = async () => {
        try {
          const factory = window.libheif || window.Libheif || window.heif;
          if (!factory) throw new Error('HEIC 解码库未就绪');
          // libheif-js 的全局量是工厂函数，调用后才拿到 HeifDecoder
          const mod = typeof factory === 'function' ? await factory() : factory;
          if (!mod || typeof mod.HeifDecoder !== 'function') throw new Error('HEIC 解码库初始化失败');
          resolve(mod);
        } catch (err) {
          reject(err);
        }
      };
      s.onerror = () => reject(new Error('HEIC 解码库加载失败'));
      document.head.appendChild(s);
    });
    return libheifLoading;
  }

  function decodeTiff(bytes) {
    const buf = bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);
    const ifds = window.UTIF.decode(buf);
    if (!ifds || !ifds.length) throw new Error('TIFF 解码失败');
    window.UTIF.decodeImage(buf, ifds[0]);
    const rgba = window.UTIF.toRGBA8(ifds[0]);
    const w = ifds[0].width;
    const h = ifds[0].height;
    const canvas = document.createElement('canvas');
    canvas.width = w;
    canvas.height = h;
    const ctx = canvas.getContext('2d');
    ctx.putImageData(new ImageData(new Uint8ClampedArray(rgba), w, h), 0, 0);
    return { canvas, width: w, height: h, pages: ifds.length };
  }

  async function decodeHeic(bytes) {
    const lib = await loadLibheif();
    const decoder = new lib.HeifDecoder();
    const images = decoder.decode(bytes);
    if (!images || !images.length) throw new Error('HEIC 解码失败（可能是不支持的编码）');
    const img = images[0];
    const w = img.get_width();
    const h = img.get_height();
    const canvas = document.createElement('canvas');
    canvas.width = w;
    canvas.height = h;
    const ctx = canvas.getContext('2d');
    const imageData = ctx.createImageData(w, h);
    await new Promise((resolve, reject) => {
      img.display(imageData, (out) => (out ? resolve() : reject(new Error('HEIC 渲染失败'))));
    });
    ctx.putImageData(imageData, 0, 0);
    return { canvas, width: w, height: h, pages: images.length };
  }

  window.Decoder = {
    extOf,
    canvasToUrl,
    isTiff: (p) => TIFF_EXT.has(extOf(p)),
    isHeic: (p) => HEIC_EXT.has(extOf(p)),
    isSpecial: (p) => TIFF_EXT.has(extOf(p)) || HEIC_EXT.has(extOf(p)),
    decodeTiff,
    decodeHeic
  };
})();
