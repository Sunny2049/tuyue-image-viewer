//! 文字识别：直接调用 Windows 自带的 OCR 引擎（Windows.Media.Ocr）
//!
//! 好处：不用打包任何识别模型（省 30MB），识别只需约 90ms（tesseract 约 1900ms），
//! 且随系统语言包更新。代价：引擎不返回逐词置信度。

use std::time::Instant;
use windows::core::HSTRING;
use windows::Globalization::Language;
use windows::Graphics::Imaging::BitmapDecoder;
use windows::Media::Ocr::OcrEngine;
use windows::Storage::Streams::{DataWriter, InMemoryRandomAccessStream};

#[derive(serde::Serialize, Clone)]
pub struct Word {
    pub text: String,
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
    /// 同一行共享同一个行号，前端据此还原成可选取的文字层
    pub line: u32,
}

#[derive(serde::Serialize)]
pub struct OcrOut {
    pub text: String,
    pub words: Vec<Word>,
    pub ms: u64,
    pub lang: String,
}

/// 中文（含全角标点）视为连续文本，词间不加空格
fn is_cjk(ch: char) -> bool {
    matches!(ch as u32,
        0x3000..=0x303F |   // CJK 标点
        0x3400..=0x4DBF |   // 扩展 A
        0x4E00..=0x9FFF |   // 基本区
        0xF900..=0xFAFF |   // 兼容
        0xFF00..=0xFFEF     // 全角
    )
}

fn join_words(parts: &[String]) -> String {
    let mut out = String::new();
    for part in parts {
        if part.is_empty() {
            continue;
        }
        if let (Some(prev), Some(next)) = (out.chars().last(), part.chars().next()) {
            if !(is_cjk(prev) || is_cjk(next)) {
                out.push(' ');
            }
        }
        out.push_str(part);
    }
    out
}

fn engine_for(lang: &str) -> windows::core::Result<(OcrEngine, String)> {
    let candidates: Vec<&str> = match lang {
        "eng" => vec!["en-US", "zh-Hans-CN"],
        "chi_sim" => vec!["zh-Hans-CN", "en-US"],
        _ => vec!["zh-Hans-CN", "en-US"],
    };
    let mut last_err = None;
    for tag in &candidates {
        let l = Language::CreateLanguage(&HSTRING::from(*tag))?;
        match OcrEngine::TryCreateFromLanguage(&l) {
            Ok(engine) => {
                let actual = engine
                    .RecognizerLanguage()
                    .and_then(|l| l.LanguageTag())
                    .map(|h| h.to_string())
                    .unwrap_or_else(|_| tag.to_string());
                return Ok((engine, actual));
            }
            Err(e) => last_err = Some(e),
        }
    }
    Err(last_err.unwrap_or_else(|| windows::core::Error::from(windows::Win32::Foundation::E_FAIL)))
}

pub fn run(path: &str, lang: &str) -> Result<OcrOut, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("读取图片失败：{e}"))?;

    let stream = InMemoryRandomAccessStream::new().map_err(err_str)?;
    let writer = DataWriter::CreateDataWriter(&stream).map_err(err_str)?;
    writer.WriteBytes(&bytes).map_err(err_str)?;
    writer.StoreAsync().map_err(err_str)?.get().map_err(err_str)?;
    writer.FlushAsync().map_err(err_str)?.get().map_err(err_str)?;
    let _ = writer.DetachStream();
    stream.Seek(0).map_err(err_str)?;

    let decoder = BitmapDecoder::CreateAsync(&stream)
        .map_err(err_str)?
        .get()
        .map_err(err_str)?;
    let bitmap = decoder
        .GetSoftwareBitmapAsync()
        .map_err(err_str)?
        .get()
        .map_err(err_str)?;

    let (engine, actual_lang) = engine_for(lang).map_err(err_str)?;

    let t0 = Instant::now();
    let result = engine
        .RecognizeAsync(&bitmap)
        .map_err(err_str)?
        .get()
        .map_err(err_str)?;
    let ms = t0.elapsed().as_millis() as u64;

    let mut words = Vec::new();
    let mut lines_text: Vec<String> = Vec::new();

    let lines = result.Lines().map_err(err_str)?;
    let line_count = lines.Size().map_err(err_str)?;
    for i in 0..line_count {
        let line = lines.GetAt(i).map_err(err_str)?;
        let ws = line.Words().map_err(err_str)?;
        let wc = ws.Size().map_err(err_str)?;
        let mut parts: Vec<String> = Vec::new();
        let mut entries: Vec<(String, f32, f32, f32, f32)> = Vec::new();
        for j in 0..wc {
            let w = ws.GetAt(j).map_err(err_str)?;
            let text = w.Text().map_err(err_str)?.to_string();
            let r = w.BoundingRect().map_err(err_str)?;
            parts.push(text.clone());
            entries.push((text, r.X, r.Y, r.X + r.Width, r.Y + r.Height));
        }
        let joined = join_words(&parts);
        if joined.trim().is_empty() {
            continue;
        }
        let line_no = lines_text.len() as u32;
        for (text, x0, y0, x1, y1) in entries {
            words.push(Word { text, x0, y0, x1, y1, line: line_no });
        }
        lines_text.push(joined);
    }

    Ok(OcrOut {
        text: lines_text.join("\n"),
        words,
        ms,
        lang: actual_lang,
    })
}

fn err_str<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}
