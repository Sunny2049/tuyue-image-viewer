'use strict';

/* window.api 由 preload 注入（全局属性，不能再用 const 声明同名变量） */
const $ = (id) => document.getElementById(id);

const IMAGE_EXTS = new Set([
  'jpg', 'jpeg', 'jpe', 'jfif', 'png', 'apng', 'gif', 'webp',
  'bmp', 'dib', 'ico', 'svg', 'avif', 'tif', 'tiff', 'heic', 'heif', 'hif'
]);

const MIME = {
  jpg: 'image/jpeg', jpeg: 'image/jpeg', jpe: 'image/jpeg', jfif: 'image/jpeg',
  png: 'image/png', apng: 'image/apng', gif: 'image/gif', webp: 'image/webp',
  bmp: 'image/bmp', dib: 'image/bmp', ico: 'image/x-icon', svg: 'image/svg+xml',
  avif: 'image/avif', tif: 'image/tiff', tiff: 'image/tiff',
  heic: 'image/heic', heif: 'image/heif', hif: 'image/heif'
};

const els = {
  titleName: $('titleName'),
  railList: $('railList'), railCount: $('railCount'), railEmpty: $('railEmpty'),
  stage: $('stage'), stageInner: $('stageInner'), viewer: $('viewer'), boxes: $('boxes'),
  stageEmpty: $('stageEmpty'), stageLoading: $('stageLoading'), stageLoadingText: $('stageLoadingText'),
  stageError: $('stageError'),
  zoomLabel: $('zoomLabel'),
  stName: $('stName'), stMeta: $('stMeta'), stIndex: $('stIndex'),
  panel: $('panel'), ocrText: $('ocrText'), ocrStat: $('ocrStat'),
  selLang: $('selLang'), chkBoxes: $('chkBoxes'), chkSelect: $('chkSelect'),
  selChip: $('selChip'), selChipCount: $('selChipCount'), selChipCopy: $('selChipCopy'),
  progressWrap: $('progressWrap'), progressBar: $('progressBar'), progressLabel: $('progressLabel'),
  dropMask: $('dropMask'), toast: $('toast'),
  menuPop: $('menuPop'), btnMore: $('btnMore'),
  modal: $('modal'), modalTitle: $('modalTitle'), modalBody: $('modalBody'),
  textLayer: null
};

const state = {
  files: [],
  index: -1,
  curPath: '',
  zoom: 1,
  fitZoom: 1,
  panX: 0,
  panY: 0,
  rotation: 0,
  natural: { w: 0, h: 0 },
  pages: 1,
  size: 0,
  loadSeq: 0,
  objUrl: null,
  ocr: { busy: false, jobId: '', scale: 1, words: [], text: '' }
};

/* ---------------- 工具 ---------------- */
const extOf = (p) => {
  const m = /\.([^./\\]+)$/.exec(p || '');
  return m ? m[1].toLowerCase() : '';
};
const baseName = (p) => (p || '').split(/[\\/]/).pop() || '';
const dirName = (p) => {
  const i = Math.max((p || '').lastIndexOf('\\'), (p || '').lastIndexOf('/'));
  return i > 0 ? p.slice(0, i) : p;
};

/** Windows 路径 -> 应用内协议 URL（由 Rust 主进程读字节返回，同源，不污染 canvas） */
const fileUrl = (p) => api.fileUrl(p);
const mimeOf = (p) => MIME[extOf(p)] || 'application/octet-stream';

function humanSize(n) {
  if (!n) return '—';
  if (n < 1024) return n + ' B';
  if (n < 1024 * 1024) return (n / 1024).toFixed(0) + ' KB';
  return (n / 1024 / 1024).toFixed(n < 10 * 1024 * 1024 ? 1 : 0) + ' MB';
}

let toastTimer = 0;
function toast(msg) {
  els.toast.textContent = msg;
  els.toast.classList.add('show');
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => els.toast.classList.remove('show'), 1900);
}

/* ---------------- 加载图片 ---------------- */
function showLoading(text) {
  if (!text) { els.stageLoading.classList.remove('show'); return; }
  els.stageLoadingText.textContent = text;
  els.stageLoading.classList.add('show');
}
const hideLoading = () => els.stageLoading.classList.remove('show');

function showError(msg) {
  els.stageInner.classList.remove('ready');
  els.stageError.textContent = msg;
  els.stageError.classList.add('show');
}
function hideError() {
  els.stageError.classList.remove('show');
  els.stageError.textContent = '';
}

function loadImageElement(url) {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = () => reject(new Error('无法显示该图片（格式损坏或不受支持）'));
    img.src = url;
  });
}

async function resolveSource(path) {
  if (window.Decoder.isSpecial(path)) {
    const bytes = await api.readFile(path);
    const r = window.Decoder.isTiff(path) ? window.Decoder.decodeTiff(bytes) : await window.Decoder.decodeHeic(bytes);
    const url = await window.Decoder.canvasToUrl(r.canvas, 'image/png');
    return { url, pages: r.pages, revoke: true };
  }
  return { url: fileUrl(path), pages: 1, revoke: false };
}

async function openAt(index) {
  if (index < 0 || index >= state.files.length) return;
  state.index = index;
  const file = state.files[index];
  state.curPath = file.path;
  state.size = file.size || 0;

  hideError();
  hideLoading();
  clearBoxes();
  clearTextLayer();
  resetEditState();
  state.ocr.words = [];
  state.ocr.text = '';
  state.ocr.path = '';
  // 换图即清空上一张的识别结果，否则右侧还留着旧文字，看着像"又自动识别了"
  els.ocrText.value = '';
  els.ocrStat.textContent = '';
  state.rotation = 0;
  state.zoom = 1;
  state.panX = 0;
  state.panY = 0;
  els.stageInner.classList.remove('ready');

  els.titleName.textContent = file.name;
  document.title = file.name + ' — 图阅';
  els.stName.textContent = file.name;
  els.stMeta.textContent = '';
  updateIndex();
  markActive(index);

  const seq = ++state.loadSeq;
  const ext = extOf(file.path).toUpperCase();

  if (window.Decoder.isTiff(file.path)) showLoading('TIFF 解码中…');
  else if (window.Decoder.isHeic(file.path)) showLoading('HEIC 解码中…');

  try {
    const src = await resolveSource(file.path);
    if (seq !== state.loadSeq) {
      if (src.revoke) URL.revokeObjectURL(src.url);
      return;
    }
    if (state.objUrl) {
      URL.revokeObjectURL(state.objUrl);
      state.objUrl = null;
    }
    if (src.revoke) state.objUrl = src.url;
    state.pages = src.pages || 1;

    await loadImageElement(src.url);
    if (seq !== state.loadSeq) return;

    els.viewer.src = src.url;
    state.natural = { w: els.viewer.naturalWidth, h: els.viewer.naturalHeight };
    els.stageInner.classList.add('ready');
    hideLoading();
    fitToView();

    const parts = [ext || 'IMG', `${state.natural.w} × ${state.natural.h}`, humanSize(state.size)];
    if (state.pages > 1) parts.push(`共 ${state.pages} 页（显示第 1 页）`);
    els.stMeta.textContent = parts.join(' · ');
  } catch (err) {
    if (seq !== state.loadSeq) return;
    hideLoading();
    showError(String(err && err.message ? err.message : err));
    els.stMeta.textContent = '';
  }
}

function updateIndex() {
  const total = state.files.length;
  els.stIndex.textContent = total ? `${state.index + 1} / ${total}` : '0 / 0';
}

function markActive(index) {
  const nodes = els.railList.children;
  for (let i = 0; i < nodes.length; i++) {
    nodes[i].classList.toggle('active', i === index);
  }
  const active = nodes[index];
  if (active && active.scrollIntoView) active.scrollIntoView({ block: 'nearest' });
}

/* ---------------- 缩略图列表 ---------------- */
function makeThumb(file, index) {
  const item = document.createElement('div');
  item.className = 'thumb';
  item.dataset.index = String(index);
  item.title = file.path;

  const pic = document.createElement('div');
  pic.className = 'thumb-pic';

  if (window.Decoder.isSpecial(file.path)) {
    const tag = document.createElement('span');
    tag.className = 'ext';
    tag.textContent = extOf(file.path).toUpperCase();
    pic.appendChild(tag);
  } else {
    const im = document.createElement('img');
    im.loading = 'lazy';
    im.decoding = 'async';
    im.src = fileUrl(file.path);
    im.onerror = () => {
      pic.innerHTML = '';
      const tag = document.createElement('span');
      tag.className = 'ext';
      tag.textContent = extOf(file.path).toUpperCase();
      pic.appendChild(tag);
    };
    pic.appendChild(im);
  }

  const meta = document.createElement('div');
  meta.className = 'thumb-meta';
  const name = document.createElement('div');
  name.className = 'thumb-name';
  name.textContent = file.name;
  const size = document.createElement('div');
  size.className = 'thumb-size';
  size.textContent = humanSize(file.size);
  meta.appendChild(name);
  meta.appendChild(size);

  item.appendChild(pic);
  item.appendChild(meta);
  item.addEventListener('click', () => openAt(index));
  return item;
}

function setFiles(files, preferredPath) {
  state.files = files || [];
  els.railList.innerHTML = '';
  els.railCount.textContent = String(state.files.length);

  if (!state.files.length) {
    els.railEmpty.style.display = 'flex';
    els.stageEmpty.classList.remove('hide');
    state.index = -1;
    els.stName.textContent = '—';
    els.stMeta.textContent = '';
    els.titleName.textContent = '未打开图片';
    document.title = '图阅';
    updateIndex();
    return;
  }

  els.railEmpty.style.display = 'none';
  els.stageEmpty.classList.add('hide');

  const frag = document.createDocumentFragment();
  state.files.forEach((f, i) => frag.appendChild(makeThumb(f, i)));
  els.railList.appendChild(frag);

  let idx = 0;
  if (preferredPath) {
    const found = state.files.findIndex((f) => f.path === preferredPath);
    if (found >= 0) idx = found;
  }
  openAt(idx);
}

/* ---------------- 缩放 / 平移 / 旋转 ---------------- */
const clamp = (v, a, b) => Math.min(b, Math.max(a, v));

function applyTransform() {
  els.stageInner.style.transform =
    `translate(${state.panX}px, ${state.panY}px) rotate(${state.rotation}deg) scale(${state.zoom})`;
  els.zoomLabel.textContent = Math.round(state.zoom * 100) + '%';
}

function fitToView() {
  if (!state.natural.w || !state.natural.h) return;
  const rect = els.stage.getBoundingClientRect();
  const rot = ((state.rotation % 360) + 360) % 360;
  const ew = rot % 180 ? state.natural.h : state.natural.w;
  const eh = rot % 180 ? state.natural.w : state.natural.h;
  const pad = 44;
  const fit = Math.min((rect.width - pad) / ew, (rect.height - pad) / eh);
  state.fitZoom = clamp(fit, 0.02, 8) || 1;
  state.zoom = state.fitZoom;
  state.panX = 0;
  state.panY = 0;
  applyTransform();
}

function setZoom(z) {
  state.zoom = clamp(z, 0.02, 20);
  applyTransform();
}

function zoomAt(clientX, clientY, factor) {
  const rect = els.stage.getBoundingClientRect();
  const cx = clientX - (rect.left + rect.width / 2);
  const cy = clientY - (rect.top + rect.height / 2);
  const z2 = clamp(state.zoom * factor, 0.02, 20);
  const k = z2 / state.zoom;
  state.panX = cx - (cx - state.panX) * k;
  state.panY = cy - (cy - state.panY) * k;
  state.zoom = z2;
  applyTransform();
}

function rotate() {
  state.rotation = (state.rotation + 90) % 360;
  fitToView();
}

/* ---------------- 文字框 ---------------- */
function clearBoxes() {
  els.boxes.innerHTML = '';
}

function renderBoxes() {
  clearBoxes();
  if (!els.chkBoxes.checked) return;
  const { w, h } = state.natural;
  const s = state.ocr.scale || 1;
  if (!w || !h) return;
  const frag = document.createDocumentFragment();
  for (const word of state.ocr.words) {
    if (!word.text || !word.text.trim()) continue;
    const d = document.createElement('div');
    d.className = 'word-box' + (word.confidence < 60 ? ' low' : '');
    d.style.left = (word.x0 / s / w) * 100 + '%';
    d.style.top = (word.y0 / s / h) * 100 + '%';
    d.style.width = Math.max(0.4, ((word.x1 - word.x0) / s / w) * 100) + '%';
    d.style.height = Math.max(0.4, ((word.y1 - word.y0) / s / h) * 100) + '%';
    frag.appendChild(d);
  }
  els.boxes.appendChild(frag);
}

/* ---------------- 图片上可直接选字 ----------------
   把每行识别结果铺成一层透明文字，位置与图上文字严格重合。
   透明不等于没有：它是真实的 DOM 文本，鼠标划选、双击选词、
   Ctrl+C、右键复制全部走系统原生行为。 */
const CJK_RE = /[\u3000-\u303f\u3400-\u4dbf\u4e00-\u9fff\uf900-\ufaff\uff00-\uffef]/;

/** 与后端 join_words 一致：中文之间不加空格，西文之间加一个空格 */
function joinLineParts(parts) {
  let out = '';
  for (const part of parts) {
    if (!part) continue;
    const prev = out.slice(-1);
    const next = part[0];
    if (prev && next && !(CJK_RE.test(prev) || CJK_RE.test(next))) out += ' ';
    out += part;
  }
  return out;
}

function groupLines(words) {
  const map = new Map();
  for (const w of words) {
    if (!w || !w.text) continue;
    const key = typeof w.line === 'number' ? w.line : Math.round(w.y0 / 6);
    let g = map.get(key);
    if (!g) {
      g = { key, parts: [], x0: Infinity, y0: Infinity, x1: -Infinity, y1: -Infinity };
      map.set(key, g);
    }
    g.parts.push(w.text);
    if (w.x0 < g.x0) g.x0 = w.x0;
    if (w.y0 < g.y0) g.y0 = w.y0;
    if (w.x1 > g.x1) g.x1 = w.x1;
    if (w.y1 > g.y1) g.y1 = w.y1;
  }
  return [...map.values()].sort((a, b) => a.key - b.key);
}

function clearTextLayer() {
  if (els.textLayer) {
    els.textLayer.remove();
    els.textLayer = null;
  }
  hideSelChip();
}

function buildTextLayer() {
  clearTextLayer();
  if (!els.chkSelect || !els.chkSelect.checked) return;
  if (!state.ocr.words.length || !state.natural.w) return;

  const s = state.ocr.scale || 1;
  const layer = document.createElement('div');
  layer.className = 'text-layer';
  const pending = [];

  for (const g of groupLines(state.ocr.words)) {
    const text = joinLineParts(g.parts);
    if (!text.trim()) continue;
    const lw = (g.x1 - g.x0) / s;
    const lh = (g.y1 - g.y0) / s;
    if (!(lw >= 1) || !(lh >= 1)) continue;

    const div = document.createElement('div');
    div.className = 'tl-line';
    div.style.left = g.x0 / s + 'px';
    div.style.top = g.y0 / s + 'px';
    div.style.height = lh + 'px';
    div.style.lineHeight = lh + 'px';
    div.style.fontSize = Math.max(1, lh * 0.82) + 'px';
    div.textContent = text;
    layer.appendChild(div);
    pending.push([div, lw]);
  }

  if (!pending.length) return;
  els.stageInner.appendChild(layer);
  els.textLayer = layer;

  // 量出文字自然宽度后再横向拉伸到识别框宽度，保证与图上位置重合
  for (const [div, targetW] of pending) {
    const measured = div.scrollWidth;
    if (measured > 0.5) {
      const k = targetW / measured;
      if (k > 0.4 && k < 2.6) {
        div.style.transform = `scaleX(${k.toFixed(4)})`;
        div.style.width = measured + 'px';
      }
    }
  }
}

/** 只认锚点和焦点都在文字层里的选区，避免误判图片拖拽 */
function selectionInLayer() {
  const sel = window.getSelection();
  if (!sel || sel.isCollapsed || !els.textLayer) return '';
  const { anchorNode, focusNode } = sel;
  if (!anchorNode || !focusNode) return '';
  if (!els.textLayer.contains(anchorNode) || !els.textLayer.contains(focusNode)) return '';
  return sel.toString();
}

/** 原生序列化会把跨行的两行粘成一句，这里按行重组，复制出来保持换行 */
function selectionText() {
  if (!selectionInLayer()) return '';
  const sel = window.getSelection();
  const range = sel.getRangeAt(0);
  const out = [];
  for (const line of els.textLayer.querySelectorAll('.tl-line')) {
    if (!range.intersectsNode(line)) continue;
    const r = document.createRange();
    r.selectNodeContents(line);
    if (line.contains(range.startContainer)) r.setStart(range.startContainer, range.startOffset);
    if (line.contains(range.endContainer)) r.setEnd(range.endContainer, range.endOffset);
    const t = r.toString();
    if (t.trim()) out.push(t);
  }
  return out.length ? out.join('\n') : sel.toString();
}

let cardTimer = 0;
function updateSelChip() {
  const text = selectionText();
  if (!text.trim()) return hideSelChip();

  const sel = window.getSelection();
  let rect = null;
  try { rect = sel.getRangeAt(0).getBoundingClientRect(); } catch { /* ignore */ }
  const count = text.replace(/\s/g, '').length;
  els.selChipCount.textContent = `已选 ${count} 字`;
  els.selChip.classList.add('show');
  if (rect && rect.width + rect.height > 0) {
    const cx = clamp(rect.left + rect.width / 2, 70, window.innerWidth - 70);
    const cy = rect.top > 56 ? rect.top - 40 : rect.bottom + 10;
    els.selChip.style.left = cx + 'px';
    els.selChip.style.top = cy + 'px';
  }
}

function hideSelChip() {
  els.selChip.classList.remove('show');
}

async function copySelection(text) {
  const t = text || selectionText();
  if (!t.trim()) return toast('先划选要复制的文字');
  const r = await api.copyText(t);
  if (r && r.ok === false) return toast('复制失败：' + (r.error || '未知错误'));
  toast(`已复制 ${t.replace(/\s/g, '').length} 字`);
}

/* ---------------- 编辑：框选填充 / 拼接 ----------------
   思路：把当前图片画进 <canvas>，之后所有修改都作用在 canvas 上，
   原文件始终不动，只有点「另存为」才落盘。 */
const EDIT = {
  canvas: null,
  ctx: null,
  dirty: false,
  sel: null,        // {x0,y0,x1,y1} 自然像素
  undo: [],         // {x,y,w,h,data}
  mode: 'pan',      // 'pan' | 'select'
  drag: null,
  busy: false
};

const MAX_UNDO = 6;

function resetEditState() {
  EDIT.canvas = null;
  EDIT.ctx = null;
  EDIT.dirty = false;
  EDIT.sel = null;
  EDIT.drag = null;
  EDIT.undo = [];
  const c = $('editCanvas');
  if (c) {
    c.classList.remove('show');
    c.width = 1;
    c.height = 1;
  }
  els.viewer.classList.remove('hide');
  $('marquee').classList.remove('show');
  $('fillChip').classList.remove('show');
  setSelectMode(false);
  updateSaveBtn();
}

function updateSaveBtn() {
  const b = $('btnSaveImg');
  if (b) b.disabled = !EDIT.dirty;
}

/** 第一次编辑时把 <img> 的内容搬进 canvas */
function ensureCanvas() {
  const c = $('editCanvas');
  const { w, h } = state.natural;
  if (!c || !w || !h) return null;
  if (EDIT.canvas === c) return c;
  c.width = w;
  c.height = h;
  const ctx = c.getContext('2d', { willReadFrequently: true });
  ctx.drawImage(els.viewer, 0, 0, w, h);
  EDIT.canvas = c;
  EDIT.ctx = ctx;
  c.classList.add('show');
  els.viewer.classList.add('hide');
  return c;
}

function markDirty() {
  EDIT.dirty = true;
  updateSaveBtn();
}

/** 屏幕坐标 → 图片自然像素 */
function toNatural(clientX, clientY) {
  const c = EDIT.canvas || $('editCanvas');
  if (!c) return { x: 0, y: 0 };
  const r = c.getBoundingClientRect();
  const w = c.width;
  const h = c.height;
  return {
    x: clamp(((clientX - r.left) / (r.width || 1)) * w, 0, w),
    y: clamp(((clientY - r.top) / (r.height || 1)) * h, 0, h)
  };
}

function setSelectMode(on) {
  EDIT.mode = on ? 'select' : 'pan';
  els.stage.classList.toggle('select-mode', !!on);
  const b = $('btnSelTool');
  if (b) b.classList.toggle('active', !!on);
  if (!on) {
    EDIT.drag = null;
    $('marquee').classList.remove('show');
    hideFillChip();
  }
}

function showMarquee(x0, y0, x1, y1) {
  const m = $('marquee');
  const x = Math.min(x0, x1);
  const y = Math.min(y0, y1);
  m.style.left = x + 'px';
  m.style.top = y + 'px';
  m.style.width = Math.abs(x1 - x0) + 'px';
  m.style.height = Math.abs(y1 - y0) + 'px';
  m.classList.add('show');
}

function hideFillChip() {
  $('fillChip').classList.remove('show');
}

function showFillChip(sel) {
  const c = EDIT.canvas;
  const r = c.getBoundingClientRect();
  const w = c.width;
  const h = c.height;
  const cx = r.left + ((sel.x0 + sel.x1) / 2 / w) * r.width;
  const cy = r.top + (sel.y1 / h) * r.height + 10;
  const chip = $('fillChip');
  $('fillChipInfo').textContent =
    `已选 ${Math.round(sel.x1 - sel.x0)} × ${Math.round(sel.y1 - sel.y0)}`;
  chip.style.left = clamp(cx, 100, window.innerWidth - 100) + 'px';
  chip.style.top = clamp(cy, 60, window.innerHeight - 16) + 'px';
  chip.classList.add('show');
}

function pushUndo(x, y, w, h, data) {
  EDIT.undo.push({ x, y, w, h, data });
  if (EDIT.undo.length > MAX_UNDO) EDIT.undo.shift();
}

function undo() {
  if (EDIT.busy) return;
  const last = EDIT.undo.pop();
  if (!last) return toast('没有可撤销的操作');
  const img = new ImageData(new Uint8ClampedArray(last.data), last.w, last.h);
  EDIT.ctx.putImageData(img, last.x, last.y);
  EDIT.dirty = EDIT.undo.length > 0;
  updateSaveBtn();
  toast('已撤销');
}

/** 核心：对一块区域做内容识别填充 */
async function inpaintRegion(sel, margin) {
  const c = ensureCanvas();
  if (!c) return null;
  const ctx = EDIT.ctx;
  const M = margin == null ? 24 : margin;
  const x0 = Math.max(0, Math.floor(sel.x0) - M);
  const y0 = Math.max(0, Math.floor(sel.y0) - M);
  const x1 = Math.min(c.width, Math.ceil(sel.x1) + M);
  const y1 = Math.min(c.height, Math.ceil(sel.y1) + M);
  const rw = x1 - x0;
  const rh = y1 - y0;
  if (rw < 3 || rh < 3) return null;
  if (rw * rh > 12000000) throw new Error('选区太大了，分几次框选试试');

  const img = ctx.getImageData(x0, y0, rw, rh);
  const mask = new Uint8Array(rw * rh);
  let hole = 0;
  const sx0 = Math.max(0, Math.round(sel.x0) - x0);
  const sy0 = Math.max(0, Math.round(sel.y0) - y0);
  const sx1 = Math.min(rw, Math.round(sel.x1) - x0);
  const sy1 = Math.min(rh, Math.round(sel.y1) - y0);
  for (let y = sy0; y < sy1; y++) {
    for (let x = sx0; x < sx1; x++) {
      mask[y * rw + x] = 1;
      hole++;
    }
  }
  if (!hole) return null;

  const before = new Uint8ClampedArray(img.data);
  window.__fillStage = 'ipc-send';
  const res = await api.inpaint({
    w: rw,
    h: rh,
    px: api.bytesToB64(new Uint8Array(img.data.buffer)),
    mask: api.bytesToB64(mask),
    iters: 4
  });
  window.__fillStage = 'ipc-back';
  if (!res || !res.ok) throw new Error((res && res.error) || '填充失败');

  const px = api.b64ToBytes(res.px);
  const out = new ImageData(new Uint8ClampedArray(px.buffer, px.byteOffset, px.byteLength), rw, rh);
  pushUndo(x0, y0, rw, rh, before);
  ctx.putImageData(out, x0, y0);
  markDirty();
  clearTextLayer();
  return { ms: res.ms, hole: res.hole, w: rw, h: rh, levels: res.levels, mode: res.mode };
}

async function runFillFromChip() {
  if (EDIT.busy) return;
  const sel = EDIT.sel;
  if (!sel) return;
  EDIT.busy = true;
  hideFillChip();
  setProgress(0.4, '识别周围纹理…');
  try {
    const r = await inpaintRegion(sel, 26);
    setProgress(1, '完成');
    setTimeout(hideProgress, 600);
    if (r) {
      const how = r.mode && r.mode.indexOf('content') === 0
        ? '只擦掉了字，背景原样保留'
        : '按周围纹理重建了整块';
      toast(`已填充 ${r.w}×${r.h} · ${how} · ${(r.ms / 1000).toFixed(2)}s`);
    }
    $('marquee').classList.remove('show');
    EDIT.sel = null;
    setSelectMode(false);
  } catch (err) {
    hideProgress();
    toast('填充失败：' + (err.message || err));
  } finally {
    EDIT.busy = false;
  }
}

/** 一键擦掉 OCR 识别到的全部文字：逐行处理，避免一次处理整张大图 */
async function eraseAllText() {
  if (EDIT.busy) return toast('正在处理，请稍候');
  if (!state.ocr.words.length) return toast('先点「识别当前图片」');
  const c = ensureCanvas();
  if (!c) return;

  const lines = groupLines(state.ocr.words);
  const s = state.ocr.scale || 1;
  EDIT.busy = true;
  $('btnEraseText').disabled = true;
  let done = 0;
  let total = 0;
  let kept = 0;

  try {
    for (const g of lines) {
      // 每行单独处理：区域小、速度快、内存也省
      // 横向多留一点：行尾标点常常没被识别成词，留在框外就擦不掉
      const padX = 28;
      const padY = 12;
      const sel = {
        x0: g.x0 / s - padX,
        y0: g.y0 / s - padY,
        x1: g.x1 / s + padX,
        y1: g.y1 / s + padY
      };
      setProgress(done / Math.max(1, lines.length), `擦除文字 ${done + 1}/${lines.length}`);
      try {
        const r = await inpaintRegion(sel, 18);
        if (r) {
          total += r.hole;
          if (r.mode && r.mode.indexOf('content') === 0) kept++;
        }
      } catch (e) {
        // 单行失败不中断整批
        console.warn('擦除失败', e);
      }
      done++;
    }
    setProgress(1, '完成');
    setTimeout(hideProgress, 700);
    clearBoxes();
    clearTextLayer();
    els.ocrStat.textContent = '文字已擦除';
    const how = kept === done ? '背景全部保留' : `${kept}/${done} 行背景原样保留`;
    toast(`已擦掉 ${done} 行文字（${total} 像素）· ${how} · Ctrl+Z 可撤销`);
  } finally {
    EDIT.busy = false;
    $('btnEraseText').disabled = !state.ocr.words.length;
  }
}

/* ---------------- 拼接 ---------------- */
async function stitchWith(path, dir) {
  if (EDIT.busy) return toast('正在处理上一件事，稍等一下再拖');
  if (!state.natural.w) return toast('先打开一张图片');
  if (!dir || dir === 'open') return handleDroppedPaths([path]);

  EDIT.busy = true;
  showLoading('拼接中…');
  try {
    const src = await resolveSource(path);
    const img = await loadImageElement(src.url);
    const c = ensureCanvas();
    const a = { w: c.width, h: c.height };
    const b = { w: img.naturalWidth, h: img.naturalHeight };
    if (!b.w || !b.h) throw new Error('这张图读不出来');

    const horizontal = dir === 'left' || dir === 'right';
    const nw = horizontal ? a.w + b.w : Math.max(a.w, b.w);
    const nh = horizontal ? Math.max(a.h, b.h) : a.h + b.h;

    const tmp = document.createElement('canvas');
    tmp.width = nw;
    tmp.height = nh;
    const tc = tmp.getContext('2d');
    tc.fillStyle = '#ffffff';
    tc.fillRect(0, 0, nw, nh);

    // 水平拼接按上边对齐，垂直拼接按左边对齐
    let ax = 0;
    let ay = 0;
    let bx = 0;
    let by = 0;
    if (dir === 'right') { ax = 0; bx = a.w; }
    else if (dir === 'left') { bx = 0; ax = b.w; }
    else if (dir === 'bottom') { ay = 0; by = a.h; }
    else if (dir === 'top') { by = 0; ay = b.h; }

    tc.drawImage(c, ax, ay);
    tc.drawImage(img, bx, by);

    pushUndo(0, 0, c.width, c.height, EDIT.ctx.getImageData(0, 0, c.width, c.height).data);
    c.width = nw;
    c.height = nh;
    EDIT.ctx = c.getContext('2d', { willReadFrequently: true });
    EDIT.ctx.drawImage(tmp, 0, 0);

    state.natural = { w: nw, h: nh };
    state.ocr.words = [];
    state.ocr.text = '';
    els.ocrText.value = '';
    clearBoxes();
    clearTextLayer();
    hideLoading();
    markDirty();
    fitToView();
    const label = { left: '左侧', right: '右侧', top: '上方', bottom: '下方' }[dir] || dir;
    els.stMeta.textContent = `已拼接 · ${nw} × ${nh}`;
    toast(`已拼到${label} · 共 ${nw} × ${nh}`);
    if (src.revoke) URL.revokeObjectURL(src.url);
  } catch (err) {
    hideLoading();
    toast('拼接失败：' + (err.message || err));
  } finally {
    EDIT.busy = false;
  }
}

/** 当前显示的图片在屏幕上的实际矩形（已含缩放/平移），拿不到返回 null */
function visibleImageRect() {
  const c = $('editCanvas');
  const el = c && c.classList.contains('show') ? c : els.viewer;
  if (!el) return null;
  const r = el.getBoundingClientRect();
  if (r.width < 4 || r.height < 4) return null;
  return r;
}

/** 边缘带宽度：按图片尺寸取比例，夹在 36–140 px 之间 */
function stitchBand(r) {
  return Math.max(36, Math.min(140, Math.min(r.width, r.height) * 0.18));
}

/**
 * 拖拽位置 → 拼接方向。
 *
 * 基准必须是「图片实际显示的区域」，不是整个画面。
 * 图缩小居中时，画面四周大片是空白：用户明明对着图片右边缘松手，
 * 按画面算却还在中间 52% 区域里 → 被判成「打开」，当前图直接被替换掉。
 */
function stitchZoneAt(cssX, cssY) {
  const st = els.stage.getBoundingClientRect();
  if (!st.width || !st.height) return '';
  if (cssX < st.left || cssY < st.top || cssX > st.right || cssY > st.bottom) return '';

  const img = visibleImageRect();
  if (!img) return 'open';

  // 落在图片之外：看哪个方向越界更多
  const ox = cssX < img.left ? img.left - cssX : cssX > img.right ? cssX - img.right : 0;
  const oy = cssY < img.top ? img.top - cssY : cssY > img.bottom ? cssY - img.bottom : 0;
  if (ox > 0 || oy > 0) {
    if (ox >= oy) return cssX < img.left ? 'left' : 'right';
    return cssY < img.top ? 'top' : 'bottom';
  }

  // 落在图片之内：看离哪条边最近
  const d = {
    left: cssX - img.left,
    right: img.right - cssX,
    top: cssY - img.top,
    bottom: img.bottom - cssY
  };
  const min = Math.min(d.left, d.right, d.top, d.bottom);
  if (min > stitchBand(img)) return 'open';
  if (min === d.left) return 'left';
  if (min === d.right) return 'right';
  if (min === d.top) return 'top';
  return 'bottom';
}

/** 把四边提示条摆到图片边上，跟随缩放/平移 */
function layoutStitchGuide() {
  const st = els.stage.getBoundingClientRect();
  const img = visibleImageRect();
  if (!st.width || !img) return;

  const band = Math.min(stitchBand(img), Math.min(img.width, img.height) / 3);
  const L = img.left - st.left;
  const T = img.top - st.top;
  const W = img.width;
  const H = img.height;
  const IW = Math.max(0, W - band * 2);
  const IH = Math.max(0, H - band * 2);

  const put = (sel, left, top, width, height) => {
    const e = document.querySelector(sel);
    if (!e) return;
    e.style.left = left + 'px';
    e.style.top = top + 'px';
    e.style.width = width + 'px';
    e.style.height = height + 'px';
    e.style.right = 'auto';
    e.style.bottom = 'auto';
  };

  put('.sg-top', L, T, W, band);
  put('.sg-bottom', L, T + H - band, W, band);
  put('.sg-left', L, T + band, band, IH);
  put('.sg-right', L + W - band, T + band, band, IH);
  put('.sg-center', L + band, T + band, IW, IH);
}

function highlightStitch(zone) {
  layoutStitchGuide();
  document.querySelectorAll('.sg-zone').forEach((z) => {
    z.classList.toggle('hot', z.dataset.dir === zone);
  });
}

function hideStitchGuide() {
  $('stitchGuide').classList.remove('show');
}

async function pickStitchFile() {
  if (!state.natural.w) return toast('先打开一张图片');
  const paths = await api.pickFiles();
  if (!paths || !paths.length) return;
  const path = paths[0];
  openModal(
    '拼接到哪一侧？',
    `<p class="modal-note">选中的是：${path.split(/[\\/]/).pop()}</p>
     <div class="stitch-pick">
       <button class="tbtn ghost" data-dir="top">上方</button>
       <button class="tbtn ghost" data-dir="left">左侧</button>
       <button class="tbtn ghost" data-dir="right">右侧</button>
       <button class="tbtn ghost" data-dir="bottom">下方</button>
     </div>`
  );
  els.modalBody.querySelectorAll('[data-dir]').forEach((b) => {
    b.addEventListener('click', () => {
      const dir = b.dataset.dir;
      closeModal();
      stitchWith(path, dir);
    });
  });
}

async function saveEdited() {
  if (!EDIT.canvas || !EDIT.dirty) return toast('还没有修改');
  const blob = await new Promise((r) => EDIT.canvas.toBlob(r, 'image/png'));
  if (!blob) return toast('导出失败');
  const buf = new Uint8Array(await blob.arrayBuffer());
  const name = (baseName(state.curPath) || '图阅').replace(/\.[^.]+$/, '') + '-编辑.png';
  const saved = await api.saveImage({ data: api.bytesToB64(buf), name });
  if (saved) toast('已保存：' + saved);
}

/* ---------------- OCR ---------------- */
function setProgress(ratio, label) {
  els.progressWrap.classList.add('show');
  els.progressBar.style.width = Math.round(clamp(ratio, 0, 1) * 100) + '%';
  els.progressLabel.textContent = label;
}
function hideProgress() {
  els.progressWrap.classList.remove('show');
  els.progressBar.style.width = '0%';
}

function setBusy(busy) {
  state.ocr.busy = busy;
  $('btnRunOcr').disabled = busy;
  $('btnBatchOcr').disabled = busy;
  $('btnRunOcr').textContent = busy ? '识别中…' : '识别当前图片';
}

/* 识别完全由用户触发：只有点「识别当前图片」或「批量识别本文件夹」才会跑 OCR。
   打开图片不做任何后台识别，也不往右侧面板里填东西。 */

async function runOcr() {
  if (state.ocr.busy) return toast('正在识别，请稍候');
  if (!state.curPath) return toast('先打开一张图片');

  const path = state.curPath;
  setBusy(true);
  setProgress(0.25, '识别中…');
  els.ocrStat.textContent = '识别中…';

  try {
    const res = await api.ocrRun({ path, langs: els.selLang.value });
    if (!res || !res.ok) throw new Error((res && res.error) || '识别失败');
    // 识别期间用户可能已经切到别的图，结果不要串台
    if (state.curPath !== path) return;

    state.ocr.path = path;
    state.ocr.scale = res.scale || 1;
    state.ocr.words = res.words || [];
    state.ocr.text = res.text || '';
    els.ocrText.value = state.ocr.text;
    renderBoxes();
    buildTextLayer();
    $('btnEraseText').disabled = !state.ocr.words.length;

    const chars = state.ocr.text.replace(/\s/g, '').length;
    els.ocrStat.textContent = `${chars} 字 · ${(res.ms / 1000).toFixed(2)}s`;
    setProgress(1, '完成');
    setTimeout(hideProgress, 700);

    if (chars) {
      if (!hintSelectShown()) {
        markSelectHint();
        toast('识别完成，可在图上按住拖选文字、Ctrl+C 复制');
      } else {
        toast('识别完成');
      }
    } else {
      toast('没识别到文字，试试切换语言');
    }
  } catch (err) {
    hideProgress();
    els.ocrStat.textContent = '识别失败';
    toast('识别失败：' + (err.message || err));
  } finally {
    setBusy(false);
  }
}

function hintSelectShown() {
  try { return localStorage.getItem('tuyue.hint.select') === '1'; } catch { return true; }
}
function markSelectHint() {
  try { localStorage.setItem('tuyue.hint.select', '1'); } catch { /* ignore */ }
}

async function runBatch() {
  if (state.ocr.busy) return toast('正在识别，请稍候');
  if (!state.files.length) return toast('先打开一个文件夹');

  setBusy(true);
  const langs = els.selLang.value;
  const blocks = [];
  let okCount = 0;
  let charCount = 0;

  try {
    for (let i = 0; i < state.files.length; i++) {
      const file = state.files[i];
      els.stName.textContent = `批量识别 ${i + 1}/${state.files.length}：${file.name}`;
      setProgress(i / state.files.length, `${i + 1} / ${state.files.length}`);

      try {
        const res = await api.ocrRun({ path: file.path, langs });
        if (res && res.ok) {
          okCount++;
          charCount += (res.text || '').replace(/\s/g, '').length;
          blocks.push(`【${file.name}】\n${res.text || '（无文字）'}`);
        } else {
          blocks.push(`【${file.name}】\n（识别失败：${(res && res.error) || '未知错误'}）`);
        }
      } catch (err) {
        blocks.push(`【${file.name}】\n（跳过：${err.message || err}）`);
      }
      els.ocrText.value = blocks.join('\n\n');
    }

    els.ocrStat.textContent = `完成 ${okCount}/${state.files.length} 张 · ${charCount} 字`;
    setProgress(1, '完成');
    setTimeout(hideProgress, 800);
    els.stName.textContent = baseName(state.curPath);
    toast('批量识别完成');
  } finally {
    setBusy(false);
  }
}

/* ---------------- 打开 ---------------- */
async function openFiles() {
  const paths = await api.pickFiles();
  if (!paths || !paths.length) return;
  const dirs = new Set(paths.map(dirName));
  if (paths.length === 1 || dirs.size === 1) {
    try {
      const listing = await api.listFolder([...dirs][0]);
      if (listing && listing.files.length) {
        setFiles(listing.files, paths[0]);
        return;
      }
    } catch { /* 目录读取失败则只显示所选文件 */ }
  }
  const files = [];
  for (const p of paths) {
    const info = await api.info(p);
    files.push({ path: p, name: baseName(p), size: info ? info.size : 0 });
  }
  setFiles(files, paths[0]);
}

async function openFolder() {
  const listing = await api.pickFolder();
  if (!listing) return;
  if (!listing.files.length) return toast('这个文件夹里没有图片');
  setFiles(listing.files, '');
}

async function openPath(path) {
  if (!path) return;
  try {
    const listing = await api.listFolder(dirName(path));
    if (listing && listing.files.length) return setFiles(listing.files, path);
  } catch { /* ignore */ }
  const info = await api.info(path);
  setFiles([{ path, name: baseName(path), size: info ? info.size : 0 }], path);
}

async function handleDroppedPaths(paths) {
  if (!paths.length) return;
  const images = paths.filter((p) => IMAGE_EXTS.has(extOf(p)));
  const others = paths.filter((p) => !IMAGE_EXTS.has(extOf(p)));

  if (!images.length && others.length === 1) {
    try {
      const listing = await api.listFolder(others[0]);
      if (listing && listing.files.length) return setFiles(listing.files, '');
      return toast('这个文件夹里没有图片');
    } catch {
      return toast('无法识别放入的内容');
    }
  }
  if (!images.length) return toast('没有可识别的图片文件');

  const files = [];
  for (const p of images) {
    const info = await api.info(p);
    files.push({ path: p, name: baseName(p), size: info ? info.size : 0 });
  }
  files.sort((a, b) => a.name.localeCompare(b.name, 'zh-CN', { numeric: true }));
  setFiles(files, files[0].path);
}

/* ---------------- 默认看图软件 ---------------- */
function renderAssoc(st) {
  const box = $('emptyAssoc');
  if (!box || !st) return;
  const txt = $('emptyAssocText');
  if (st.all) {
    box.classList.add('on');
    txt.textContent = `已是默认看图软件（${st.total} 种格式）`;
  } else if (st.ours > 0) {
    box.classList.remove('on');
    txt.textContent = `${st.ours}/${st.total} 个格式已指向图阅 · 点此完成`;
  } else {
    box.classList.remove('on');
    txt.textContent = '设为系统默认看图软件';
  }
}

async function refreshAssoc() {
  try {
    renderAssoc(await api.assocStatus());
  } catch { /* 关联查询失败不影响看图 */ }
}

async function makeDefault() {
  try {
    const r = await api.assocMakeDefault();
    if (r && r.state) renderAssoc(r.state);
    if (r && r.state && r.state.all) toast('图阅 已是默认看图软件');
    else toast('已登记，请在系统设置里把图片格式选成「图阅」');
  } catch (err) {
    toast('设置失败：' + (err.message || err));
  }
}

/* ---------------- 弹窗 ---------------- */
function openModal(title, html) {
  els.modalTitle.textContent = title;
  els.modalBody.innerHTML = html;
  els.modal.classList.add('show');
}
function closeModal() {
  els.modal.classList.remove('show');
  els.modalBody.innerHTML = '';
}

async function showAssocDetail() {
  const st = await api.assocStatus();
  const rows = (st.items || [])
    .map((it) => {
      const owner = it.ours ? '<b class="ours">图阅</b>' : it.prog_id || '（无默认程序）';
      return `<div class="assoc-row"><span class="ext">${it.ext}</span><span class="owner">${owner}</span></div>`;
    })
    .join('');
  openModal(
    `格式关联：${st.ours}/${st.total} 指向图阅`,
    `<p class="modal-note">Windows 会把默认程序的最终选择记在受保护的系统键里，所以需要你在系统设置里确认一次。已经指向图阅的格式不用再改。</p>
     <div class="assoc-grid">${rows}</div>
     <div class="modal-actions">
       <button class="tbtn primary" id="modalMakeDefault">设为默认…</button>
       <button class="tbtn ghost" id="modalOpenSettings">打开系统设置</button>
     </div>`
  );
  const btn1 = $('modalMakeDefault');
  const btn2 = $('modalOpenSettings');
  if (btn1) btn1.addEventListener('click', async () => { await makeDefault(); closeModal(); });
  if (btn2) btn2.addEventListener('click', () => api.openDefaultApps());
}

function showFormats() {
  openModal(
    '支持的图片格式',
    `<p class="modal-note">以下格式都能直接打开；TIFF 与 HEIC 由内置解码器处理，不依赖系统插件。</p>
     <div class="fmt-grid">
       ${['JPG', 'JPEG', 'PNG', 'APNG', 'GIF', '动态 GIF', 'WEBP', 'BMP', 'ICO', 'SVG', 'AVIF', 'TIFF', '多页 TIFF', 'HEIC', 'HEIF']
         .map((f) => `<span class="fmt">${f}</span>`)
         .join('')}
     </div>`
  );
}

function showAbout() {
  openModal(
    '关于图阅',
    `<p class="modal-note">图阅 1.0 · 图片查看与文字提取<br/>Rust + 系统 WebView2，离线运行，不联网。<br/>文字识别使用 Windows 自带 OCR 引擎。</p>`
  );
}

/* ---------------- 事件绑定 ---------------- */
function bindEvents() {
  $('btnOpenFile').addEventListener('click', openFiles);
  $('btnEmptyFile').addEventListener('click', openFiles);
  $('btnOpenFolder').addEventListener('click', openFolder);
  $('btnEmptyFolder').addEventListener('click', openFolder);
  $('btnMakeDefault').addEventListener('click', (e) => {
    e.stopPropagation();
    makeDefault();
  });

  document.querySelectorAll('[data-act]').forEach((btn) => {
    btn.addEventListener('click', () => {
      const act = btn.dataset.act;
      if (act === 'zoomIn') setZoom(state.zoom * 1.25);
      if (act === 'zoomOut') setZoom(state.zoom / 1.25);
      if (act === 'zoomFit') fitToView();
      if (act === 'zoomActual') { state.panX = 0; state.panY = 0; setZoom(1); }
      if (act === 'rotate') rotate();
    });
  });

  $('btnPrev').addEventListener('click', () => openAt(state.index - 1));
  $('btnNext').addEventListener('click', () => openAt(state.index + 1));

  // 编辑工具
  $('btnSelTool').addEventListener('click', () => {
    if (!state.natural.w) return toast('先打开一张图片');
    if (state.rotation % 360 !== 0) {
      state.rotation = 0;
      applyTransform();
    }
    setSelectMode(EDIT.mode !== 'select');
    if (EDIT.mode === 'select') toast('拖出一个方框，框里的东西会被抹掉并补成背景');
  });
  $('btnStitch').addEventListener('click', pickStitchFile);
  $('btnUndo').addEventListener('click', undo);
  $('btnSaveImg').addEventListener('click', saveEdited);
  $('btnEraseText').addEventListener('click', eraseAllText);
  $('fillChipRun').addEventListener('click', runFillFromChip);
  $('fillChipCancel').addEventListener('click', () => {
    hideFillChip();
    $('marquee').classList.remove('show');
    EDIT.sel = null;
    setSelectMode(false);
  });

  $('btnOcr').addEventListener('click', () => {
    els.panel.classList.remove('collapsed');
    runOcr();
  });
  $('btnClosePanel').addEventListener('click', () => els.panel.classList.add('collapsed'));
  $('btnRunOcr').addEventListener('click', runOcr);
  $('btnBatchOcr').addEventListener('click', runBatch);
  $('btnCopyText').addEventListener('click', async () => {
    if (!els.ocrText.value.trim()) return toast('没有可复制的内容');
    await api.copyText(els.ocrText.value);
    toast('已复制到剪贴板');
  });
  $('btnSaveText').addEventListener('click', async () => {
    if (!els.ocrText.value.trim()) return toast('没有可保存的内容');
    const name = (baseName(state.curPath) || '识别结果').replace(/\.[^.]+$/, '') + '.txt';
    const saved = await api.saveText({ content: els.ocrText.value, defaultName: name });
    if (saved) toast('已保存：' + saved);
  });
  $('btnClearText').addEventListener('click', () => {
    els.ocrText.value = '';
    state.ocr.words = [];
    els.ocrStat.textContent = '未识别';
    $('btnEraseText').disabled = true;
    clearBoxes();
    clearTextLayer();
  });
  els.chkBoxes.addEventListener('change', renderBoxes);
  els.chkSelect.addEventListener('change', () => {
    buildTextLayer();
    // 划选靠的是识别出来的文字层，没识别过就没有层，这里只提示、不偷跑识别
    if (els.chkSelect.checked && state.curPath && !state.ocr.words.length) {
      toast('点右侧「识别当前图片」后就能在图上划选复制');
    }
  });

  // 划选后浮出「复制」小条；点击空白处即收起
  document.addEventListener('selectionchange', () => {
    clearTimeout(cardTimer);
    cardTimer = setTimeout(updateSelChip, 80);
  });
  els.selChipCopy.addEventListener('mousedown', (e) => {
    e.preventDefault();
    e.stopPropagation();
  });
  els.selChipCopy.addEventListener('click', (e) => {
    e.stopPropagation();
    copySelection();
  });

  // 右键「复制」等原生复制路径：把跨行文字按行拼回去再写剪贴板
  document.addEventListener('copy', (e) => {
    const t = selectionText();
    if (!t) return;
    e.preventDefault();
    if (e.clipboardData) e.clipboardData.setData('text/plain', t);
    api.copyText(t);
    toast(`已复制 ${t.replace(/\s/g, '').length} 字`);
  });

  // 滚轮缩放
  els.stage.addEventListener('wheel', (e) => {
    if (!state.natural.w) return;
    e.preventDefault();
    const factor = Math.exp(-e.deltaY * 0.0016);
    zoomAt(e.clientX, e.clientY, factor);
  }, { passive: false });

  // 拖拽：框选模式下画选框，其余情况平移（在文字层上拖 = 划选文字）
  let drag = null;
  els.stage.addEventListener('pointerdown', (e) => {
    if (e.button !== 0 || !state.natural.w) return;

    if (EDIT.mode === 'select') {
      const c = ensureCanvas();
      if (!c) return;
      hideFillChip();
      EDIT.sel = null;
      const p = toNatural(e.clientX, e.clientY);
      EDIT.drag = { x0: p.x, y0: p.y };
      showMarquee(p.x, p.y, p.x, p.y);
      try { els.stage.setPointerCapture(e.pointerId); } catch { /* ignore */ }
      e.preventDefault();
      return;
    }

    if (e.target.closest && e.target.closest('.text-layer')) {
      hideSelChip();
      return;
    }
    drag = { x: e.clientX, y: e.clientY, panX: state.panX, panY: state.panY };
    els.stage.classList.add('panning');
    try { els.stage.setPointerCapture(e.pointerId); } catch { /* ignore */ }
  });
  els.stage.addEventListener('pointermove', (e) => {
    if (EDIT.drag) {
      const p = toNatural(e.clientX, e.clientY);
      showMarquee(EDIT.drag.x0, EDIT.drag.y0, p.x, p.y);
      return;
    }
    if (!drag) return;
    state.panX = drag.panX + (e.clientX - drag.x);
    state.panY = drag.panY + (e.clientY - drag.y);
    applyTransform();
  });
  const endDrag = (e) => {
    if (EDIT.drag) {
      const p = toNatural(e.clientX, e.clientY);
      const sel = {
        x0: Math.min(EDIT.drag.x0, p.x),
        y0: Math.min(EDIT.drag.y0, p.y),
        x1: Math.max(EDIT.drag.x0, p.x),
        y1: Math.max(EDIT.drag.y0, p.y)
      };
      EDIT.drag = null;
      try { els.stage.releasePointerCapture(e.pointerId); } catch { /* ignore */ }
      if (sel.x1 - sel.x0 >= 4 && sel.y1 - sel.y0 >= 4) {
        EDIT.sel = sel;
        showFillChip(sel);
      } else {
        $('marquee').classList.remove('show');
        EDIT.sel = null;
      }
      return;
    }
    if (!drag) return;
    drag = null;
    els.stage.classList.remove('panning');
    try { els.stage.releasePointerCapture(e.pointerId); } catch { /* ignore */ }
  };
  els.stage.addEventListener('pointerup', endDrag);
  els.stage.addEventListener('pointercancel', endDrag);

  els.stage.addEventListener('dblclick', (e) => {
    if (e.target.closest && e.target.closest('.text-layer')) return;
    if (Math.abs(state.zoom - 1) < 0.01) fitToView();
    else { state.panX = 0; state.panY = 0; setZoom(1); }
  });

  // 键盘（菜单已注册的快捷键不重复处理）
  document.addEventListener('keydown', (e) => {
    const tag = (e.target && e.target.tagName) || '';
    if (tag === 'TEXTAREA' || tag === 'INPUT' || tag === 'SELECT') {
      if (e.key === 'Escape') e.target.blur();
      return;
    }
    if (e.key === 'Escape') {
      if (els.modal.classList.contains('show')) return closeModal();
      if (EDIT.mode === 'select' || EDIT.sel) {
        setSelectMode(false);
        $('marquee').classList.remove('show');
        EDIT.sel = null;
        hideFillChip();
        return;
      }
      els.menuPop.classList.remove('show');
      els.panel.classList.add('collapsed');
      return;
    }
    if (e.ctrlKey || e.metaKey) {
      const k = e.key.toLowerCase();
      if (k === 'c') {
        const t = selectionText();
        if (t) { e.preventDefault(); copySelection(t); }
        return;
      }
      if (k === 'a' && els.textLayer) {
        e.preventDefault();
        const range = document.createRange();
        range.selectNodeContents(els.textLayer);
        const sel = window.getSelection();
        sel.removeAllRanges();
        sel.addRange(range);
        return;
      }
      if (k === 'o') { e.preventDefault(); if (e.shiftKey) openFolder(); else openFiles(); return; }
      if (k === 'e') { e.preventDefault(); els.panel.classList.remove('collapsed'); runOcr(); return; }
      if (k === 'z') { e.preventDefault(); undo(); return; }
      if (k === 's' && e.shiftKey) { e.preventDefault(); saveEdited(); return; }
      if (k === 's') { e.preventDefault(); $('btnSaveText').click(); return; }
      if (k === '=' || k === '+') { e.preventDefault(); setZoom(state.zoom * 1.25); return; }
      if (k === '-') { e.preventDefault(); setZoom(state.zoom / 1.25); return; }
      if (k === '0') { e.preventDefault(); fitToView(); return; }
      if (k === '1') { e.preventDefault(); state.panX = 0; state.panY = 0; setZoom(1); return; }
      return;
    }
    if (e.altKey) return;
    switch (e.key) {
      case 'ArrowLeft': e.preventDefault(); openAt(state.index - 1); break;
      case 'ArrowRight': e.preventDefault(); openAt(state.index + 1); break;
      case ' ':
      case 'PageDown': e.preventDefault(); openAt(state.index + 1); break;
      case 'PageUp': e.preventDefault(); openAt(state.index - 1); break;
      case 'Home': if (state.files.length) openAt(0); break;
      case 'End': if (state.files.length) openAt(state.files.length - 1); break;
      case 'r':
      case 'R': rotate(); break;
      case 'm':
      case 'M':
        if (state.natural.w) {
          if (state.rotation % 360 !== 0) { state.rotation = 0; applyTransform(); }
          setSelectMode(EDIT.mode !== 'select');
          if (EDIT.mode === 'select') toast('拖出一个方框，框里的东西会被抹掉并补成背景');
        }
        break;
      case '+':
      case '=': setZoom(state.zoom * 1.25); break;
      case '-':
      case '_': setZoom(state.zoom / 1.25); break;
      case '0': fitToView(); break;
      case '1': state.panX = 0; state.panY = 0; setZoom(1); break;
      default: break;
    }
  });

  // 文件拖放由主进程接管（WebView2 原生 OLE 拖放才能拿到真实路径）
  // 落点决定行为：拖到画面四边 = 拼接，拖到中间 = 打开
  api.onDrop((payload) => {
    hideStitchGuide();
    if (!payload) return;
    const paths = payload.paths || [];
    if (!paths.length) return;
    const dpr = window.devicePixelRatio || 1;
    const zone = state.natural.w
      ? stitchZoneAt((payload.x || 0) / dpr, (payload.y || 0) / dpr)
      : '';
    const imgs = paths.filter((p) => IMAGE_EXTS.has(extOf(p)));
    if (zone && zone !== 'open' && imgs.length === 1) return stitchWith(imgs[0], zone);
    return handleDroppedPaths(paths);
  });

  api.onDragOver((payload) => {
    if (!state.natural.w || !payload) return;
    const dpr = window.devicePixelRatio || 1;
    const zone = stitchZoneAt(payload.x / dpr, payload.y / dpr);
    if (!zone) return hideStitchGuide();
    $('stitchGuide').classList.add('show');
    highlightStitch(zone);
  });

  api.onDragLeave(() => hideStitchGuide());
  window.addEventListener('dragover', (e) => e.preventDefault());
  window.addEventListener('drop', (e) => e.preventDefault());

  // 窗口尺寸变化时，若处于「适应窗口」则重新适应
  const ro = new ResizeObserver(() => {
    if (!state.natural.w) return;
    if (Math.abs(state.zoom - state.fitZoom) < 0.002) fitToView();
  });
  ro.observe(els.stage);

  // 「更多」菜单
  els.btnMore.addEventListener('click', (e) => {
    e.stopPropagation();
    els.menuPop.classList.toggle('show');
  });
  els.menuPop.addEventListener('click', async (e) => {
    e.stopPropagation();
    const btn = e.target.closest('[data-menu]');
    if (!btn) return;
    els.menuPop.classList.remove('show');
    const act = btn.dataset.menu;
    if (act === 'makeDefault') makeDefault();
    else if (act === 'checkAssoc') showAssocDetail();
    else if (act === 'unregister') {
      if (!window.confirm('取消文件关联？只会移除图阅自己写入的登记，不会影响其他程序。')) return;
      const r = await api.assocUnregister();
      if (r && r.state) renderAssoc(r.state);
      toast('已取消文件关联');
    } else if (act === 'formats') showFormats();
    else if (act === 'about') showAbout();
    else if (act === 'quit') api.quit();
  });
  document.addEventListener('click', () => els.menuPop.classList.remove('show'));

  $('modalClose').addEventListener('click', closeModal);
  els.modal.addEventListener('click', (e) => {
    if (e.target === els.modal) closeModal();
  });

  // 菜单
  api.onMenu(({ action, payload }) => {
    switch (action) {
      case 'openFiles': openFiles(); break;
      case 'openFolder': openFolder(); break;
      case 'zoomIn': setZoom(state.zoom * 1.25); break;
      case 'zoomOut': setZoom(state.zoom / 1.25); break;
      case 'zoomFit': fitToView(); break;
      case 'zoomActual': state.panX = 0; state.panY = 0; setZoom(1); break;
      case 'rotate': rotate(); break;
      case 'prev': openAt(state.index - 1); break;
      case 'next': openAt(state.index + 1); break;
      case 'ocr': els.panel.classList.remove('collapsed'); runOcr(); break;
      case 'ocrBatch': els.panel.classList.remove('collapsed'); runBatch(); break;
      case 'copyText': $('btnCopyText').click(); break;
      case 'saveText': $('btnSaveText').click(); break;
      case 'devtools': break;
      case 'aboutFormats':
        toast('支持 JPG / PNG / GIF / WEBP / BMP / ICO / SVG / AVIF / TIFF / HEIC');
        break;
      case 'assocState':
        renderAssoc(payload);
        break;
      default: break;
    }
  });

  api.onOcrProgress((msg) => {
    if (!msg) return;
    if (msg.status === 'done') return;
    const ratio = typeof msg.progress === 'number' ? msg.progress : 0;
    setProgress(ratio, msg.label || '识别中…');
  });

  api.onOpenPath((p) => openPath(p));
}

bindEvents();
applyTransform();
refreshAssoc();

/* 自动化自检入口（界面不调用；--shot-fill 会走这里）。
   直接复用真实函数，测的是用户实际会走的那条链路。 */
window.__editTest = async (what, arg) => {
  window.__fillStage = 'enter';
  try {
    if (what === 'fill') {
      if (!ensureCanvas()) return 'no-canvas';
      window.__fillStage = 'canvas';
      const g = groupLines(state.ocr.words)[0];
      if (!g) return 'no-words';
      const sel = { x0: g.x0 - 6, y0: g.y0 - 6, x1: g.x1 + 6, y1: g.y1 + 6 };
      EDIT.sel = sel;
      setSelectMode(true);
      showMarquee(sel.x0, sel.y0, sel.x1, sel.y1);
      window.__fillStage = 'before-fill';
      const r = await inpaintRegion(sel, 26);
      window.__fillStage = 'after-fill';
      setSelectMode(false);
      $('marquee').classList.remove('show');
      return r ? `fill ok ${r.w}x${r.h} ${r.ms}ms ${r.mode || ''}` : 'fill null';
    }
    if (what === 'zonetest') {
      // 采样若干落点，验证「图片边界」而非「画面边界」的判定
      const img = visibleImageRect();
      if (!img) return 'no-image';
      const band = stitchBand(img);
      const cx = (img.left + img.right) / 2;
      const cy = (img.top + img.bottom) / 2;
      const near = Math.min(10, band / 2);
      const side = Math.min(20, band / 2);
      const r = [
        ['c', stitchZoneAt(cx, cy)],
        ['iR', stitchZoneAt(img.right - near, cy)],
        ['oR', stitchZoneAt(img.right + 20, cy)],
        ['iT', stitchZoneAt(cx, img.top + 5)],
        ['oB', stitchZoneAt(cx, img.bottom + 30)],
        ['iL', stitchZoneAt(img.left + side, cy)],
        ['oL', stitchZoneAt(img.left - 25, cy)]
      ];
      return `img=${Math.round(img.width)}x${Math.round(img.height)} band=${Math.round(band)} ` +
        r.map((p) => p[0] + ':' + (p[1] || '-')).join(' ');
    }
    if (what === 'stitch') {
      await stitchWith(arg, 'right');
      return `stitch ok ${state.natural.w}x${state.natural.h}`;
    }
    if (what === 'erase') {
      await eraseAllText();
      return `erase ok ${state.natural.w}x${state.natural.h}`;
    }
    return 'unknown';
  } catch (e) {
    return 'err ' + (e && e.message ? e.message : e);
  }
};
