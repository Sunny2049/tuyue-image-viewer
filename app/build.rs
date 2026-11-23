//! 把图标和版本信息写进 exe（用 MSYS2 的 windres 编译 .rc）
//!
//! 这台机器上没有 gcc/rc.exe，windres 默认调 gcc -E 会失败，
//! 所以配一个「原样透传」的替身预处理器（tools/fakecpp.py）。
//! 中间文件都放 %TEMP%，避免项目路径里的空格被 windres 拼命令时拆开。
//! 任何一步失败都只警告、不让构建失败——图标属于润色，不该挡住编译。

use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=res/app.rc");
    println!("cargo:rerun-if-changed=res/icon.ico");
    println!("cargo:rerun-if-changed=../tools/fakecpp.py");

    let target = std::env::var("TARGET").unwrap_or_default();
    if !target.contains("windows") {
        return;
    }

    let tmp = PathBuf::from(std::env::var("TEMP").unwrap_or_else(|_| "C:/Windows/Temp".into()));
    let obj = tmp.join("tuyue_app_res.o");
    let cpp = tmp.join("tuyue_fakecpp.py");
    let fake = Path::new("../tools/fakecpp.py");

    if let Err(e) = std::fs::copy(fake, &cpp) {
        println!("cargo:warning=复制预处理器替身失败（{e}），跳过图标");
        return;
    }

    let windres = std::env::var("WINDRES").unwrap_or_else(|_| "windres".to_string());
    // 换成自己的解释器：设环境变量 PYTHON 即可
    let python = std::env::var("PYTHON").unwrap_or_else(|_| "python".to_string());

    let out = Command::new(&windres)
        .arg(format!("--preprocessor={python}"))
        .arg(format!("--preprocessor-arg={}", cpp.display()))
        .args(["-O", "coff", "--codepage=65001", "-I", "res", "res/app.rc", "-o"])
        .arg(&obj)
        .output();

    match out {
        Ok(o) if o.status.success() && obj.exists() => {
            println!("cargo:rustc-link-arg={}", obj.display());
        }
        Ok(o) => println!(
            "cargo:warning=windres 失败，跳过图标：{}",
            String::from_utf8_lossy(&o.stderr).trim()
        ),
        Err(e) => println!("cargo:warning=找不到 windres（{e}），跳过图标"),
    }
}
