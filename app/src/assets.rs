//! 内嵌静态资源（编译进 exe，单文件分发）

pub struct Asset {
    pub bytes: &'static [u8],
    pub mime: &'static str,
}

macro_rules! asset {
    ($path:literal, $mime:literal) => {
        Asset {
            bytes: include_bytes!(concat!("../ui/", $path)),
            mime: $mime,
        }
    };
}

pub fn lookup(path: &str) -> Option<Asset> {
    let p = path.trim_start_matches('/');
    let p = if p.is_empty() { "index.html" } else { p };
    Some(match p {
        "index.html" => asset!("index.html", "text/html; charset=utf-8"),
        "styles.css" => asset!("styles.css", "text/css; charset=utf-8"),
        "app.js" => asset!("app.js", "text/javascript; charset=utf-8"),
        "bridge.js" => asset!("bridge.js", "text/javascript; charset=utf-8"),
        "decode.js" => asset!("decode.js", "text/javascript; charset=utf-8"),
        "icon.png" => asset!("icon.png", "image/png"),
        "vendor/UTIF.js" => asset!("vendor/UTIF.js", "text/javascript; charset=utf-8"),
        "vendor/libheif-bundle.js" => {
            asset!("vendor/libheif-bundle.js", "text/javascript; charset=utf-8")
        }
        _ => return None,
    })
}

pub fn mime_of_path(p: &str) -> &'static str {
    let ext = p
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "jpg" | "jpeg" | "jpe" | "jfif" => "image/jpeg",
        "png" => "image/png",
        "apng" => "image/apng",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" | "dib" => "image/bmp",
        "ico" => "image/x-icon",
        "svg" => "image/svg+xml",
        "avif" => "image/avif",
        "tif" | "tiff" => "image/tiff",
        "heic" | "heif" | "hif" => "image/heic",
        _ => "application/octet-stream",
    }
}
