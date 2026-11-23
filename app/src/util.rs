//! 文件系统相关的小工具

use std::path::{Path, PathBuf};

pub const IMAGE_EXTS: &[&str] = &[
    "jpg", "jpeg", "jpe", "jfif", "png", "apng", "gif", "webp", "bmp", "dib", "ico", "svg",
    "avif", "tif", "tiff", "heic", "heif", "hif",
];

pub fn ext_of(p: &str) -> String {
    Path::new(p)
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
}

pub fn is_image(p: &str) -> bool {
    IMAGE_EXTS.contains(&ext_of(p).as_str())
}

pub fn base_name(p: &str) -> String {
    Path::new(p)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| p.to_string())
}

pub fn dir_name(p: &str) -> String {
    Path::new(p)
        .parent()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default()
}

/// 自然排序：a2 排在 a10 前面
fn natural_key(name: &str) -> Vec<(u8, u64, String)> {
    let mut out = Vec::new();
    let mut num = String::new();
    let mut txt = String::new();
    for ch in name.chars() {
        if ch.is_ascii_digit() {
            if !txt.is_empty() {
                out.push((1u8, 0u64, txt.to_lowercase()));
                txt.clear();
            }
            num.push(ch);
        } else {
            if !num.is_empty() {
                out.push((0u8, num.parse::<u64>().unwrap_or(0), String::new()));
                num.clear();
            }
            txt.push(ch);
        }
    }
    if !num.is_empty() {
        out.push((0u8, num.parse::<u64>().unwrap_or(0), String::new()));
    }
    if !txt.is_empty() {
        out.push((1u8, 0u64, txt.to_lowercase()));
    }
    out
}

#[derive(serde::Serialize, Clone)]
pub struct FileEntry {
    pub path: String,
    pub name: String,
    pub size: u64,
}

pub fn list_images(dir: &str) -> Result<Vec<FileEntry>, String> {
    let rd = std::fs::read_dir(dir).map_err(|e| format!("读取目录失败：{e}"))?;
    let mut files: Vec<FileEntry> = Vec::new();
    for ent in rd.flatten() {
        let path = ent.path();
        let p = path.to_string_lossy().to_string();
        if !is_image(&p) {
            continue;
        }
        let meta = ent.metadata().ok();
        if let Some(m) = &meta {
            if !m.is_file() {
                continue;
            }
        }
        files.push(FileEntry {
            path: p.clone(),
            name: base_name(&p),
            size: meta.map(|m| m.len()).unwrap_or(0),
        });
    }
    files.sort_by(|a, b| natural_key(&a.name).cmp(&natural_key(&b.name)));
    Ok(files)
}

/// %APPDATA%\图阅 —— 关联备份等配置
pub fn app_data_dir() -> PathBuf {
    let base = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
    let d = PathBuf::from(base).join("图阅");
    let _ = std::fs::create_dir_all(&d);
    d
}

/// %LOCALAPPDATA%\Tuyue —— WebView2 数据、单实例中转
pub fn local_dir() -> PathBuf {
    let base = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".to_string());
    let d = PathBuf::from(base).join("Tuyue");
    let _ = std::fs::create_dir_all(&d);
    d
}
