//! 前端 → 主进程的命令分发

use base64::Engine as _;
use serde_json::{json, Value};

fn arg_str(args: &Value, key: &str) -> String {
    args.get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

pub fn dispatch(cmd: &str, args: &Value) -> Value {
    match cmd {
        "info" => {
            let p = arg_str(args, "path");
            match std::fs::metadata(&p) {
                Ok(m) => json!({
                    "size": m.len(),
                    "name": crate::util::base_name(&p),
                    "dir": crate::util::dir_name(&p),
                }),
                Err(_) => Value::Null,
            }
        }

        "readFile" => {
            let p = arg_str(args, "path");
            match std::fs::read(&p) {
                Ok(bytes) => json!(base64::engine::general_purpose::STANDARD.encode(bytes)),
                Err(e) => json!({ "error": format!("读取失败：{e}") }),
            }
        }

        "pickFiles" => {
            let mut dlg = rfd::FileDialog::new().set_title("打开图片");
            let exts: Vec<&str> = crate::util::IMAGE_EXTS.to_vec();
            dlg = dlg.add_filter("图片", &exts);
            let picked = dlg.pick_files();
            match picked {
                Some(paths) => json!(paths
                    .into_iter()
                    .map(|p| p.to_string_lossy().to_string())
                    .collect::<Vec<_>>()),
                None => json!([]),
            }
        }

        "pickFolder" => {
            match rfd::FileDialog::new().set_title("打开图片文件夹").pick_folder() {
                Some(dir) => {
                    let d = dir.to_string_lossy().to_string();
                    match crate::util::list_images(&d) {
                        Ok(files) => json!({ "dir": d, "files": files }),
                        Err(e) => json!({ "dir": d, "files": [], "error": e }),
                    }
                }
                None => Value::Null,
            }
        }

        "listFolder" => {
            let d = arg_str(args, "dir");
            match crate::util::list_images(&d) {
                Ok(files) => json!({ "dir": d, "files": files }),
                Err(e) => json!({ "dir": d, "files": [], "error": e }),
            }
        }

        "ocrRun" => {
            let p = arg_str(args, "path");
            let lang = arg_str(args, "langs");
            match crate::ocr::run(&p, if lang.is_empty() { "chi_sim+eng" } else { &lang }) {
                Ok(out) => json!({
                    "ok": true,
                    "text": out.text,
                    "words": out.words,
                    "ms": out.ms,
                    "lang": out.lang,
                    "scale": 1.0,
                }),
                Err(e) => json!({ "ok": false, "error": e }),
            }
        }

        "copyText" => {
            let t = arg_str(args, "text");
            match set_clipboard(&t) {
                Ok(_) => json!(true),
                Err(e) => json!({ "ok": false, "error": e }),
            }
        }

        "saveText" => {
            let content = arg_str(args, "content");
            let default_name = arg_str(args, "defaultName");
            let name = if default_name.is_empty() {
                "识别结果.txt".to_string()
            } else {
                default_name
            };
            match rfd::FileDialog::new()
                .set_title("保存文字")
                .set_file_name(&name)
                .add_filter("文本文件", &["txt"])
                .save_file()
            {
                Some(p) => match std::fs::write(&p, content) {
                    Ok(_) => json!(p.to_string_lossy().to_string()),
                    Err(e) => json!({ "ok": false, "error": format!("保存失败：{e}") }),
                },
                None => Value::Null,
            }
        }

        "inpaint" => {
            use base64::engine::general_purpose::STANDARD as B64;
            let w = args.get("w").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let h = args.get("h").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let iters = args.get("iters").and_then(|v| v.as_u64()).unwrap_or(4) as usize;
            let mut px = match B64.decode(arg_str(args, "px").as_bytes()) {
                Ok(v) => v,
                Err(e) => return json!({ "ok": false, "error": format!("像素解码失败：{e}") }),
            };
            let mask = match B64.decode(arg_str(args, "mask").as_bytes()) {
                Ok(v) => v,
                Err(e) => return json!({ "ok": false, "error": format!("选区解码失败：{e}") }),
            };
            // mode = "auto"：只擦内容、保留背景（淡黄底上的黑字 → 只掉黑字）
            // mode = "region"：整块选区重建（PS 里框选一整块时的行为）
            let mode = arg_str(args, "mode");
            let out = if mode == "region" {
                crate::inpaint::inpaint(&mut px, w, h, &mask, iters)
            } else {
                crate::inpaint::inpaint_auto(&mut px, w, h, &mask, iters)
            };
            match out {
                Ok(rep) => json!({
                    "ok": true,
                    "px": B64.encode(&px),
                    "ms": rep.ms,
                    "hole": rep.hole_px,
                    "levels": rep.levels,
                    "mode": rep.mode,
                }),
                Err(e) => json!({ "ok": false, "error": e }),
            }
        }

        "saveImage" => {
            use base64::engine::general_purpose::STANDARD as B64;
            let data = match B64.decode(arg_str(args, "data").as_bytes()) {
                Ok(v) => v,
                Err(e) => return json!({ "ok": false, "error": format!("图片解码失败：{e}") }),
            };
            let name = {
                let n = arg_str(args, "name");
                if n.is_empty() { "图阅-编辑.png".to_string() } else { n }
            };
            match rfd::FileDialog::new()
                .set_title("保存图片")
                .set_file_name(&name)
                .add_filter("PNG 图片", &["png"])
                .save_file()
            {
                Some(p) => match std::fs::write(&p, &data) {
                    Ok(_) => json!(p.to_string_lossy().to_string()),
                    Err(e) => json!({ "ok": false, "error": format!("保存失败：{e}") }),
                },
                None => Value::Null,
            }
        }

        "assocStatus" => json!(crate::assoc::status()),

        "assocMakeDefault" => match crate::assoc::register(&current_exe()) {
            Ok(st) => {
                if !st.all {
                    crate::assoc::open_default_apps_settings();
                }
                json!({ "ok": true, "state": st })
            }
            Err(e) => json!({ "ok": false, "error": e }),
        },

        "assocUnregister" => match crate::assoc::unregister() {
            Ok(st) => json!({ "ok": true, "state": st }),
            Err(e) => json!({ "ok": false, "error": e }),
        },

        "openDefaultApps" => {
            crate::assoc::open_default_apps_settings();
            json!(true)
        }

        "quit" => {
            std::process::exit(0);
        }

        _ => json!({ "ok": false, "error": format!("未知命令 {cmd}") }),
    }
}

pub fn current_exe() -> String {
    std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default()
}

fn set_clipboard(text: &str) -> Result<(), String> {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
    use windows::Win32::System::Ole::CF_UNICODETEXT;

    unsafe {
        OpenClipboard(None).map_err(|e| format!("打开剪贴板失败：{e}"))?;
        let _ = EmptyClipboard();

        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let bytes = wide.len() * 2;
        let h = GlobalAlloc(GMEM_MOVEABLE, bytes).map_err(|e| format!("分配内存失败：{e}"))?;
        let ptr = GlobalLock(h);
        if ptr.is_null() {
            return Err("锁定内存失败".into());
        }
        std::ptr::copy_nonoverlapping(wide.as_ptr() as *const u8, ptr as *mut u8, bytes);
        let _ = GlobalUnlock(h);

        let r = SetClipboardData(CF_UNICODETEXT.0 as u32, HANDLE(h.0));
        let _ = CloseClipboard();
        r.map(|_| ()).map_err(|e| format!("写入剪贴板失败：{e}"))?;
    }
    Ok(())
}
