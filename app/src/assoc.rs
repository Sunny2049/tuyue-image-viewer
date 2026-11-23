//! 把「图阅」注册成系统看图软件（HKCU，无需管理员）
//!
//! 注意：Windows 8 之后 .png 等格式的最终归属记在带签名哈希的 UserChoice 键里，
//! 第三方程序既写不了也伪造不了。所以这里做的是正规三层注册：
//!   ProgID + Applications\图阅.exe + Capabilities/RegisteredApplications
//! 用户随后在系统「默认应用」里点一次即可。没有默认程序的格式会直接生效。

use std::collections::BTreeMap;
use std::path::Path;
use winreg::enums::*;
use winreg::RegKey;

pub const PROGID: &str = "ImgTxt.Image";
pub const LABEL: &str = "图阅";

pub const EXTS: &[&str] = &[
    ".jpg", ".jpeg", ".jpe", ".jfif", ".png", ".apng", ".gif", ".webp", ".bmp", ".dib",
    ".ico", ".svg", ".avif", ".tif", ".tiff", ".heic", ".heif", ".hif",
];

#[derive(serde::Serialize, Clone)]
pub struct Item {
    pub ext: String,
    pub prog_id: String,
    pub source: String,
    pub ours: bool,
}

#[derive(serde::Serialize, Clone)]
pub struct Status {
    pub total: usize,
    pub ours: usize,
    pub all: bool,
    pub items: Vec<Item>,
}

fn hkcu() -> RegKey {
    RegKey::predef(HKEY_CURRENT_USER)
}

fn hklm() -> RegKey {
    RegKey::predef(HKEY_LOCAL_MACHINE)
}

fn read_default(root: &RegKey, path: &str) -> Option<String> {
    root.open_subkey_with_flags(path, KEY_READ)
        .ok()
        .and_then(|k| k.get_value::<String, _>("").ok())
        .filter(|v| !v.is_empty())
}

fn read_str(root: &RegKey, path: &str, name: &str) -> Option<String> {
    root.open_subkey_with_flags(path, KEY_READ)
        .ok()
        .and_then(|k| k.get_value::<String, _>(name).ok())
}

pub fn status() -> Status {
    let cu = hkcu();
    let lm = hklm();
    let mut items = Vec::new();
    for ext in EXTS {
        let uc_path = format!(
            "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\FileExts\\{ext}\\UserChoice"
        );
        let (prog_id, source) = if let Some(v) = read_str(&cu, &uc_path, "ProgId") {
            (v, "userchoice")
        } else if let Some(v) = read_default(&cu, &format!("Software\\Classes\\{ext}")) {
            (v, "hkcu")
        } else if let Some(v) = read_default(&lm, &format!("SOFTWARE\\Classes\\{ext}")) {
            (v, "hklm")
        } else {
            (String::new(), "none")
        };
        let ours = prog_id.eq_ignore_ascii_case(PROGID);
        items.push(Item {
            ext: ext.to_string(),
            prog_id,
            source: source.to_string(),
            ours,
        });
    }
    let ours = items.iter().filter(|i| i.ours).count();
    Status {
        total: items.len(),
        ours,
        all: ours == items.len(),
        items,
    }
}

fn backup_path() -> std::path::PathBuf {
    crate::util::app_data_dir().join("assoc-backup.json")
}

fn load_backup() -> BTreeMap<String, String> {
    std::fs::read_to_string(backup_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_backup(map: &BTreeMap<String, String>) {
    if let Ok(s) = serde_json::to_string_pretty(map) {
        let _ = std::fs::write(backup_path(), s);
    }
}

pub fn register(exe: &str) -> Result<Status, String> {
    let cu = hkcu();
    let exe_name = Path::new(exe)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| format!("{LABEL}.exe"));
    let icon = format!("\"{exe}\",0");
    let cmd = format!("\"{exe}\" \"%1\"");

    let mut errs: Vec<String> = Vec::new();

    let prog = format!("Software\\Classes\\{PROGID}");
    let mut set = |path: &str, name: &str, val: &str| {
        if let Err(e) = cu
            .create_subkey(path)
            .and_then(|(k, _)| k.set_value(name, &val.to_string()))
        {
            errs.push(format!("{path}[{name}]：{e}"));
        }
    };

    set(&prog, "", &format!("{LABEL} 图片"));
    set(&prog, "FriendlyTypeName", "图片文件");
    set(&format!("{prog}\\DefaultIcon"), "", &icon);
    set(&format!("{prog}\\shell\\open"), "FriendlyAppName", LABEL);
    set(&format!("{prog}\\shell\\open\\command"), "", &cmd);

    let app = format!("Software\\Classes\\Applications\\{exe_name}");
    set(&app, "FriendlyAppName", LABEL);
    set(&app, "ApplicationCompany", LABEL);
    set(&format!("{app}\\DefaultIcon"), "", &icon);
    set(&format!("{app}\\shell\\open\\command"), "", &cmd);

    let cap = format!("Software\\{LABEL}\\Capabilities");
    set(&cap, "ApplicationName", LABEL);
    set(&cap, "ApplicationDescription", "看图 + 提取图片文字");
    set(&cap, "ApplicationIcon", &icon);
    set("Software\\RegisteredApplications", LABEL, &format!("Software\\{LABEL}\\Capabilities"));

    let mut backup = load_backup();
    for ext in EXTS {
        let cls = format!("Software\\Classes\\{ext}");
        let cur = read_default(&cu, &cls).unwrap_or_default();
        if !cur.eq_ignore_ascii_case(PROGID) {
            backup.insert(ext.to_string(), cur);
        }
        set(&cls, "", PROGID);
        set(&format!("{cls}\\OpenWithProgids"), PROGID, "");
        set(&format!("{app}\\SupportedTypes"), ext, "");
        set(&format!("{cap}\\FileAssociations"), ext, PROGID);
    }
    save_backup(&backup);

    // 让资源管理器与「默认应用」页面刷新
    notify_shell();

    if !errs.is_empty() {
        eprintln!("register warnings: {}", errs.join(" | "));
    }
    Ok(status())
}

pub fn unregister() -> Result<Status, String> {
    let cu = hkcu();
    let backup = load_backup();

    for ext in EXTS {
        let cls = format!("Software\\Classes\\{ext}");
        let prev = backup.get(*ext).cloned().unwrap_or_default();
        if let Ok(key) = cu.open_subkey_with_flags(&cls, KEY_SET_VALUE) {
            let cur = key.get_value::<String, _>("").unwrap_or_default();
            if cur.eq_ignore_ascii_case(PROGID) || cur.is_empty() {
                if prev.is_empty() {
                    let _ = key.delete_value("");
                } else {
                    let _ = key.set_value("", &prev);
                }
            }
        }
        if let Ok(key) = cu.open_subkey_with_flags(format!("{cls}\\OpenWithProgids"), KEY_SET_VALUE) {
            let _ = key.delete_value(PROGID);
        }
    }

    let exe_name = format!("{LABEL}.exe");
    for path in [
        format!("Software\\Classes\\{PROGID}"),
        format!("Software\\Classes\\Applications\\{exe_name}"),
        format!("Software\\{LABEL}"),
    ] {
        let _ = cu.delete_subkey_all(&path);
    }
    if let Ok(key) = cu.open_subkey_with_flags("Software\\RegisteredApplications", KEY_SET_VALUE) {
        let _ = key.delete_value(LABEL);
    }
    let _ = std::fs::remove_file(backup_path());

    notify_shell();
    Ok(status())
}

pub fn open_default_apps_settings() {
    use windows::core::HSTRING;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let target = HSTRING::from(format!("ms-settings:defaultapps?registeredAppUser={LABEL}"));
    let verb = HSTRING::from("open");
    unsafe {
        let r = ShellExecuteW(None, &verb, &target, None, None, SW_SHOWNORMAL);
        if (r.0 as isize) <= 32 {
            let fallback = HSTRING::from("ms-settings:defaultapps");
            ShellExecuteW(None, &verb, &fallback, None, None, SW_SHOWNORMAL);
        }
    }
}

fn notify_shell() {
    use windows::Win32::UI::Shell::{SHChangeNotify, SHCNE_ASSOCCHANGED, SHCNF_IDLIST};
    unsafe {
        SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None);
    }
}
