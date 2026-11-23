//! 图阅 · 图片查看与文字提取（Rust + 系统 WebView2）
//!
//! 体积对比：Electron 版 397MB → 本版约 6MB。
//! 图片解码交给系统 WebView2（Chromium 内核），TIFF/HEIC 用内嵌的 wasm 解码库，
//! 文字识别交给 Windows 自带 OCR 引擎（无需打包模型）。

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod assoc;
mod assets;
mod inpaint;
mod ipc;
mod ocr;
mod shot;
mod single;
mod util;

use serde_json::{json, Value};
use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tao::platform::windows::WindowExtWindows;
use tao::window::WindowBuilder;
use wry::http::{Request, Response};
use wry::WebViewBuilder;

enum AppEvent {
    Ipc(Value),
    IpcDone(i64, Value),
    /// 拖放落下：路径 + 落点（相对窗口左上角，物理像素）
    Drop(Vec<String>, i32, i32),
    /// 拖拽经过：落点
    DragOver(i32, i32),
    DragLeave,
    OpenPath(String),
}

/// WinRT 的异步方法必须在线程内有 COM 公寓才能正常完成回调。
/// 主线程被消息循环占着，识别放到独立线程里跑，否则 .get() 会等自己。
fn com_init() {
    use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--register") {
        let st = assoc::register(&ipc::current_exe());
        write_cli_result("register", st);
        return;
    }
    if args.iter().any(|a| a == "--unregister") {
        let st = assoc::unregister();
        write_cli_result("unregister", st);
        return;
    }
    if args.iter().any(|a| a == "--selftest") {
        let img = args.get(2).cloned().unwrap_or_default();
        com_init();
        let out = ocr::run(&img, "chi_sim+eng");
        let path = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("selftest.json")))
            .unwrap_or_else(|| "selftest.json".into());
        let body = match out {
            Ok(o) => json!({
                "ok": true,
                "ms": o.ms,
                "lang": o.lang,
                "words": o.words.len(),
                "text": o.text,
                "chars": o.text.chars().filter(|c| !c.is_whitespace()).count(),
            }),
            Err(e) => json!({ "ok": false, "error": e }),
        };
        let _ = std::fs::write(&path, serde_json::to_string_pretty(&body).unwrap());
        return;
    }

    // 离线算法回归测试：不启动窗口，纯算。
    // 用法：图阅.exe --filltest in.raw out.raw W H x0 y0 x1 y1 [auto|region]
    if args.iter().any(|a| a == "--filltest") {
        let num = |i: usize| args.get(i).and_then(|v| v.parse::<usize>().ok()).unwrap_or(0);
        let inp = args.get(2).cloned().unwrap_or_default();
        let outp = args.get(3).cloned().unwrap_or_default();
        let (w, h) = (num(4), num(5));
        let (x0, y0, x1, y1) = (num(6), num(7), num(8), num(9));
        let mode = args.get(10).cloned().unwrap_or_else(|| "auto".into());
        let report = |body: Value| {
            let p = std::path::Path::new(&outp)
                .parent()
                .map(|d| d.join("filltest.json"))
                .unwrap_or_else(|| "filltest.json".into());
            let _ = std::fs::write(p, serde_json::to_string_pretty(&body).unwrap());
        };
        let mut px = match std::fs::read(&inp) {
            Ok(v) => v,
            Err(e) => {
                report(json!({ "ok": false, "error": format!("读入失败：{e}") }));
                return;
            }
        };
        if px.len() != w * h * 4 {
            report(json!({ "ok": false, "error": format!("尺寸不符 {} != {}", px.len(), w * h * 4) }));
            return;
        }
        let mut mask = vec![0u8; w * h];
        for y in y0..y1.min(h) {
            for x in x0..x1.min(w) {
                mask[y * w + x] = 1;
            }
        }
        let res = if mode == "region" {
            inpaint::inpaint(&mut px, w, h, &mask, 4)
        } else {
            inpaint::inpaint_auto(&mut px, w, h, &mask, 4)
        };
        match res {
            Ok(rep) => match std::fs::write(&outp, &px) {
                Ok(_) => report(json!({
                    "ok": true, "mode": rep.mode, "ms": rep.ms,
                    "hole": rep.hole_px, "levels": rep.levels,
                })),
                Err(e) => report(json!({ "ok": false, "error": format!("写出失败：{e}") })),
            },
            Err(e) => report(json!({ "ok": false, "error": e })),
        }
        return;
    }

    let pending: Option<String> = args
        .iter()
        .skip(1)
        .find(|a| !a.starts_with("--") && std::path::Path::new(a).exists())
        .cloned();

    let (is_first, rx) = if args.iter().any(|a| a == "--shot") {
        // 自检要用独立实例，不能被正在使用的窗口用单实例机制顶掉
        (true, None)
    } else {
        single::acquire()
    };
    if !is_first {
        if let Some(p) = pending {
            single::forward_to_first(&p);
        }
        return;
    }

    app_main(pending, rx);
}

fn write_cli_result(action: &str, st: Result<assoc::Status, String>) {
    let body = match st {
        Ok(s) => json!({ "action": action, "ok": true, "total": s.total, "ours": s.ours, "all": s.all, "items": s.items }),
        Err(e) => json!({ "action": action, "ok": false, "error": e }),
    };
    let p = util::app_data_dir().join("assoc-cli.json");
    let _ = std::fs::write(p, serde_json::to_string_pretty(&body).unwrap());
}

fn boot_log(msg: &str) {
    use std::io::Write;
    // 只有自检模式才写日志，正式使用时不往程序目录丢文件
    if !std::env::args().any(|a| a == "--shot") {
        return;
    }
    let dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| ".".into());
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("boot.log"))
    {
        let _ = writeln!(f, "{}", msg);
    }
}

fn app_main(pending: Option<String>, rx: Option<std::sync::mpsc::Receiver<String>>) {
    boot_log(&format!("app_main args={:?} pending={:?}", std::env::args().collect::<Vec<_>>(), pending));
    let event_loop = EventLoopBuilder::<AppEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    let window = WindowBuilder::new()
        .with_title("图阅")
        .with_inner_size(tao::dpi::LogicalSize::new(1280.0, 840.0))
        .with_min_inner_size(tao::dpi::LogicalSize::new(900.0, 560.0))
        .build(&event_loop)
        .expect("创建窗口失败");
    boot_log("window built");

    let ipc_proxy = proxy.clone();
    let drop_proxy = proxy.clone();
    let ready_proxy = proxy.clone();
    let task_proxy = proxy.clone();

    // WebView2 的缓存默认会堆在 exe 旁边；指到 %LOCALAPPDATA% 才不会污染程序目录，
    // 也避免以后装到 Program Files 时因权限不足起不来。
    // 自检跑独立数据目录，免得和正在用的窗口抢同一个 user data folder。
    let cache = if std::env::args().any(|a| a == "--shot") {
        util::local_dir().join("WebView2-selftest")
    } else {
        util::local_dir().join("WebView2")
    };
    let mut web_context = wry::WebContext::new(Some(cache));

    let webview = WebViewBuilder::new_with_web_context(&mut web_context)
        .with_url("app://localhost/index.html")
        .with_custom_protocol("app".into(), |_id, req| serve(req))
        .with_ipc_handler(move |req| {
            if let Ok(v) = serde_json::from_str::<Value>(req.body()) {
                let _ = ipc_proxy.send_event(AppEvent::Ipc(v));
            }
        })
        .with_drag_drop_handler(move |event| {
            // 拖到图片边缘 = 拼接，拖到中间 = 打开；位置来自原生拖放的落点
            match event {
                wry::DragDropEvent::Enter { position, .. } => {
                    let _ = drop_proxy.send_event(AppEvent::DragOver(position.0, position.1));
                }
                wry::DragDropEvent::Over { position } => {
                    let _ = drop_proxy.send_event(AppEvent::DragOver(position.0, position.1));
                }
                wry::DragDropEvent::Drop { paths, position } => {
                    let list: Vec<String> = paths
                        .into_iter()
                        .map(|p| p.to_string_lossy().to_string())
                        .collect();
                    let _ = drop_proxy.send_event(AppEvent::Drop(list, position.0, position.1));
                }
                wry::DragDropEvent::Leave => {
                    let _ = drop_proxy.send_event(AppEvent::DragLeave);
                }
                _ => {}
            }
            true
        })
        .build(&window)
        .expect("创建 WebView 失败");
    boot_log("webview built");

    let handle: Rc<RefCell<Option<wry::WebView>>> = Rc::new(RefCell::new(None));
    *handle.borrow_mut() = Some(webview);
    let wv = handle.clone();

    let selftest = std::env::args().any(|a| a == "--shot");
    let shot_ocr = std::env::args().any(|a| a == "--shot-ocr");
    let shot_png = std::env::args().any(|a| a == "--shot-png");
    let shot_copy = std::env::args().any(|a| a == "--shot-copy");
    let shot_fill = std::env::args().any(|a| a == "--shot-fill");
    let shot_erase = std::env::args().any(|a| a == "--shot-erase");
    let shot_stitch = std::env::args().any(|a| a == "--shot-stitch");
    let mut fill_fired = false;
    let test_img = pending.clone().unwrap_or_default();
    let hwnd = window.hwnd();
    let t0 = Instant::now();
    let mut ocr_fired = false;
    let mut pending_open = pending.clone();
    let _ = ready_proxy; // 保留：页面就绪事件由 IPC 触发

    let mut last_tick = Instant::now();
    let mut dump_at: Option<Instant> = None;
    let mut png_done = false;
    let mut copy_done = false;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(120));

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                *control_flow = ControlFlow::Exit;
            }

            Event::UserEvent(AppEvent::Ipc(msg)) => {
                let cmd = msg.get("cmd").and_then(|c| c.as_str()).unwrap_or("");
                let id = msg.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
                let args = msg.get("args").cloned().unwrap_or(Value::Null);

                if cmd == "__ready" {
                    boot_log("page ready");
                    if let Some(p) = pending_open.take() {
                        eval(&wv, "open", &json!(p));
                    }
                    if selftest {
                        // 交给下面的定时器做诊断
                    }
                    return;
                }

                // 识别要开线程：主线程被消息循环占着，WinRT 异步在这里会死等
                // 填充是纯 CPU 重活（几百毫秒到几秒），同样不能占着消息循环
                if cmd == "ocrRun" || cmd == "inpaint" {
                    let tp = task_proxy.clone();
                    let args2 = args.clone();
                    let c2 = cmd.to_string();
                    std::thread::spawn(move || {
                        com_init();
                        let res = ipc::dispatch(&c2, &args2);
                        let _ = tp.send_event(AppEvent::IpcDone(id, res));
                    });
                    return;
                }

                let res = ipc::dispatch(cmd, &args);
                let payload = serde_json::to_string(&res).unwrap_or_else(|_| "null".into());
                let js = format!("window.__ipcDone && window.__ipcDone({id}, {payload});");
                if let Some(w) = wv.borrow().as_ref() {
                    let _ = w.evaluate_script(&js);
                }
            }

            Event::UserEvent(AppEvent::IpcDone(id, res)) => {
                let payload = serde_json::to_string(&res).unwrap_or_else(|_| "null".into());
                let js = format!("window.__ipcDone && window.__ipcDone({id}, {payload});");
                if let Some(w) = wv.borrow().as_ref() {
                    let _ = w.evaluate_script(&js);
                }
            }

            Event::UserEvent(AppEvent::Drop(paths, x, y)) => {
                eval(&wv, "drop", &json!({ "paths": paths, "x": x, "y": y }));
            }

            Event::UserEvent(AppEvent::DragOver(x, y)) => {
                eval(&wv, "dragover", &json!({ "x": x, "y": y }));
            }

            Event::UserEvent(AppEvent::DragLeave) => {
                eval(&wv, "dragleave", &Value::Null);
            }

            Event::UserEvent(AppEvent::OpenPath(p)) => {
                eval(&wv, "open", &json!(p));
            }

            _ => {}
        }

        // 第二个实例转交过来的路径
        if let Some(rx) = &rx {
            while let Ok(p) = rx.try_recv() {
                if let Some(filename) = std::path::Path::new(&p).file_name() {
                    let _ = filename;
                }
                eval(&wv, "open", &json!(p));
            }
        }

        // --shot：启动后采集一次页面状态，用于自动化验证
        if selftest && last_tick.elapsed() > Duration::from_millis(40) {
            last_tick = Instant::now();
            if shot_ocr && !ocr_fired && t0.elapsed() > Duration::from_millis(900) {
                ocr_fired = true;
                boot_log("click ocr");
                if let Some(w) = wv.borrow().as_ref() {
                    let _ = w.evaluate_script(
                        "document.getElementById('btnRunOcr') && document.getElementById('btnRunOcr').click();",
                    );
                }
            }
            if shot_fill && !fill_fired && t0.elapsed() > Duration::from_millis(1500) {
                fill_fired = true;
                boot_log("run fill test");
                if let Some(w) = wv.borrow().as_ref() {
                    let _ = w.evaluate_script(
                        "window.__editTest && window.__editTest('fill').then(r => { window.__fillResult = r; });",
                    );
                }
            }
            if shot_erase && !fill_fired && t0.elapsed() > Duration::from_millis(1500) {
                fill_fired = true;
                boot_log("run erase test");
                if let Some(w) = wv.borrow().as_ref() {
                    let _ = w.evaluate_script(
                        "window.__editTest && window.__editTest('erase').then(r => { window.__fillResult = r; });",
                    );
                }
            }
            if shot_stitch && !fill_fired && t0.elapsed() > Duration::from_millis(1500) {
                fill_fired = true;
                boot_log("run stitch test");
                let js = format!(
                    "window.__editTest && (async () => {{ \
                       const z = await window.__editTest('zonetest'); \
                       const s = await window.__editTest('stitch', {img}); \
                       window.__fillResult = 'zone[' + z + '] ' + s; \
                     }})();",
                    img = serde_json::to_string(&test_img).unwrap()
                );
                if let Some(w) = wv.borrow().as_ref() {
                    let _ = w.evaluate_script(&js);
                }
            }
            let deadline = if shot_erase {
                5200
            } else if shot_stitch || shot_fill {
                4200
            } else if shot_ocr {
                2600
            } else {
                2000
            };
            if dump_at.is_none() && t0.elapsed() > Duration::from_millis(deadline) {
                boot_log("dump diagnostics");
                run_shot_diagnostics(&wv);
                dump_at = Some(Instant::now());
            }
            // 采集是异步回调，要等它落地再退出，否则脚本还没执行进程就没了
            if let Some(d) = dump_at {
                // --shot-copy：另起一次脚本触发真实 copy 事件，验证剪贴板
                if shot_copy && !copy_done && d.elapsed() > Duration::from_millis(300) {
                    copy_done = true;
                    if let Some(w) = wv.borrow().as_ref() {
                        let _ = w.evaluate_script(
                            "(() => { const L = document.querySelectorAll('.tl-line'); \
                             if (L.length < 2) return 'no-lines'; \
                             const r = document.createRange(); \
                             r.setStart(L[0].firstChild, 0); \
                             r.setEnd(L[1].firstChild, (L[1].firstChild.textContent || '').length); \
                             const s = window.getSelection(); s.removeAllRanges(); s.addRange(r); \
                             return document.execCommand('copy') ? 'copied' : 'exec-false'; })()",
                        );
                    }
                }
                if d.elapsed() > Duration::from_millis(500) && !png_done {
                    png_done = true;
                    if shot_png {
                        let dir = std::env::current_exe()
                            .ok()
                            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
                            .unwrap_or_else(|| ".".into());
                        match shot::capture(hwnd, &dir.join("shot.bmp")) {
                            Ok(_) => boot_log("captured shot.bmp"),
                            Err(e) => boot_log(&format!("capture failed: {e}")),
                        }
                    }
                }
                if d.elapsed() > Duration::from_millis(900) {
                    *control_flow = ControlFlow::Exit;
                }
            }
        }
    });
}

fn eval(wv: &Rc<RefCell<Option<wry::WebView>>>, name: &str, payload: &Value) {
    let p = serde_json::to_string(payload).unwrap_or_else(|_| "null".into());
    let js = format!(
        "window.__ipcEvent && window.__ipcEvent({}, {});",
        serde_json::to_string(name).unwrap(),
        p
    );
    if let Some(w) = wv.borrow().as_ref() {
        let _ = w.evaluate_script(&js);
    }
}

fn run_shot_diagnostics(wv: &Rc<RefCell<Option<wry::WebView>>>) {
    let out_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| ".".into());
    let script = r#"
    (() => {
      const id = (x) => document.getElementById(x);
      const el = id('stageInner');
      const img = id('viewer');
      const boxes = document.querySelectorAll('.word-box').length;
      const cs = getComputedStyle(el);
      const tlLines = document.querySelectorAll('.tl-line');
      let pick = null;
      try {
        const first = tlLines[0];
        const last = tlLines[Math.min(1, tlLines.length - 1)];
        if (first && last) {
          const r = document.createRange();
          r.setStart(first.firstChild, 0);
          r.setEnd(last.firstChild, (last.firstChild.textContent || '').length);
          const sel = window.getSelection();
          sel.removeAllRanges();
          sel.addRange(r);
          pick = {
            text: sel.toString().slice(0, 60),
            chars: sel.toString().replace(/\s/g, '').length,
            rects: r.getClientRects().length,
            chip: id('selChip') ? id('selChip').classList.contains('show') : null
          };
        }
      } catch (e) { pick = { error: String(e) }; }
      return JSON.stringify({
        url: location.href,
        ready: el ? el.classList.contains('ready') : null,
        transform: cs.transform,
        imgNatural: img ? [img.naturalWidth, img.naturalHeight] : null,
        imgRect: img ? [Math.round(img.getBoundingClientRect().width), Math.round(img.getBoundingClientRect().height)] : null,
        stageRect: id('stage') ? [Math.round(id('stage').getBoundingClientRect().width), Math.round(id('stage').getBoundingClientRect().height)] : null,
        thumbs: document.querySelectorAll('.thumb').length,
        boxes,
        ocrText: id('ocrText') ? id('ocrText').value.slice(0, 200) : '',
        ocrStat: id('ocrStat') ? id('ocrStat').textContent : '',
        tlLines: tlLines.length,
        tlFirst: tlLines[0] ? tlLines[0].textContent.slice(0, 40) : '',
        tlBox: tlLines[0] ? (() => { const b = tlLines[0].getBoundingClientRect(); return [Math.round(b.left), Math.round(b.top), Math.round(b.width), Math.round(b.height)]; })() : null,
        pick,
        fill: window.__fillResult || null,
        fillStage: window.__fillStage || null,
        editCanvas: id('editCanvas') ? [id('editCanvas').width, id('editCanvas').height, id('editCanvas').classList.contains('show')] : null,
        saveEnabled: id('btnSaveImg') ? !id('btnSaveImg').disabled : null,
        statusMeta: id('stMeta') ? id('stMeta').textContent : '',
        err: id('stageError') ? id('stageError').textContent : '',
        toast: id('toast') ? id('toast').textContent : '',
        assoc: id('emptyAssocText') ? id('emptyAssocText').textContent : '',
        fontFamily: getComputedStyle(document.body).fontFamily,
        brandIcon: (() => { const b = document.querySelector('.brand img'); return b ? [b.complete, b.naturalWidth, b.naturalHeight] : null; })()
      });
    })()
    "#;

    let target = out_dir.join("shot.json");
    if let Some(w) = wv.borrow().as_ref() {
        let path = target.clone();
        let _ = w.evaluate_script_with_callback(script, move |raw| {
            let text = serde_json::from_str::<String>(&raw).unwrap_or(raw);
            boot_log(&format!("callback len={} head={}", text.len(), &text[..text.len().min(80)]));
            let _ = std::fs::write(&path, text);
        });
    }
}

fn serve(req: Request<Vec<u8>>) -> Response<Cow<'static, [u8]>> {
    let uri = req.uri();
    let path = uri.path().to_string();

    if path == "/file" {
        let q = uri.query().unwrap_or("");
        let p = query_param(q, "p");
        return match std::fs::read(&p) {
            Ok(bytes) => build(200, assets::mime_of_path(&p), Cow::Owned(bytes)),
            Err(_) => build(404, "text/plain; charset=utf-8", Cow::Borrowed(&b"not found"[..])),
        };
    }

    match assets::lookup(&path) {
        Some(a) => build(200, a.mime, Cow::Borrowed(a.bytes)),
        None => build(404, "text/plain; charset=utf-8", Cow::Borrowed(&b"not found"[..])),
    }
}

fn build(status: u16, mime: &str, body: Cow<'static, [u8]>) -> Response<Cow<'static, [u8]>> {
    Response::builder()
        .status(status)
        .header("Content-Type", mime)
        .header("Cache-Control", "no-store")
        .header("Access-Control-Allow-Origin", "*")
        .body(body)
        .unwrap()
}

fn query_param(q: &str, key: &str) -> String {
    for pair in q.split('&') {
        let mut it = pair.splitn(2, '=');
        let k = it.next().unwrap_or("");
        if k == key {
            return percent_decode(it.next().unwrap_or(""));
        }
    }
    String::new()
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let h = (bytes[i + 1] as char).to_digit(16);
                let l = (bytes[i + 2] as char).to_digit(16);
                if let (Some(h), Some(l)) = (h, l) {
                    out.push((h * 16 + l) as u8);
                    i += 3;
                    continue;
                }
                out.push(bytes[i]);
                i += 1;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).to_string()
}
