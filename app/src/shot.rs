//! 自检用：把窗口内容截成 BMP（不依赖任何外部工具）
//!
//! WebView2 是合成渲染，普通 BitBlt 拿到的常是黑块，
//! 所以优先用 PrintWindow(PW_RENDERFULLCONTENT)，失败再退回屏幕抓取。

use std::path::Path;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits,
    ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HGDIOBJ,
    SRCCOPY,
};
use windows::Win32::Storage::Xps::{PrintWindow, PRINT_WINDOW_FLAGS};
use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;

pub fn capture(hwnd: isize, out: &Path) -> Result<(), String> {
    unsafe {
        let h = HWND(hwnd as *mut core::ffi::c_void);
        let mut rc = RECT::default();
        GetWindowRect(h, &mut rc).map_err(|e| format!("GetWindowRect: {e}"))?;
        let w = rc.right - rc.left;
        let ht = rc.bottom - rc.top;
        if w <= 0 || ht <= 0 {
            return Err(format!("窗口尺寸异常 {w}x{ht}"));
        }

        let screen = GetDC(None);
        let mem = CreateCompatibleDC(screen);
        let bmp = CreateCompatibleBitmap(screen, w, ht);
        let old = SelectObject(mem, HGDIOBJ(bmp.0));

        let mut painted = PrintWindow(h, mem, PRINT_WINDOW_FLAGS(2)).as_bool();
        if !painted {
            painted = BitBlt(mem, 0, 0, w, ht, screen, rc.left, rc.top, SRCCOPY).is_ok();
        }

        let stride = ((w * 3 + 3) / 4) * 4;
        let mut buf = vec![0u8; (stride * ht) as usize];
        let mut bi = BITMAPINFO::default();
        bi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bi.bmiHeader.biWidth = w;
        bi.bmiHeader.biHeight = ht;
        bi.bmiHeader.biPlanes = 1;
        bi.bmiHeader.biBitCount = 24;
        bi.bmiHeader.biCompression = BI_RGB.0;
        let lines = GetDIBits(
            mem,
            bmp,
            0,
            ht as u32,
            Some(buf.as_mut_ptr() as *mut core::ffi::c_void),
            &mut bi,
            DIB_RGB_COLORS,
        );

        SelectObject(mem, old);
        let _ = DeleteObject(HGDIOBJ(bmp.0));
        let _ = DeleteDC(mem);
        ReleaseDC(None, screen);

        if lines == 0 {
            return Err("GetDIBits 取像素失败".into());
        }

        // 24 位 BMP：14 字节文件头 + 40 字节信息头 + 自底向上的像素
        let mut file = Vec::with_capacity(54 + buf.len());
        file.extend_from_slice(b"BM");
        file.extend_from_slice(&((54 + buf.len()) as u32).to_le_bytes());
        file.extend_from_slice(&0u32.to_le_bytes());
        file.extend_from_slice(&54u32.to_le_bytes());
        file.extend_from_slice(&40u32.to_le_bytes());
        file.extend_from_slice(&w.to_le_bytes());
        file.extend_from_slice(&ht.to_le_bytes());
        file.extend_from_slice(&1u16.to_le_bytes());
        file.extend_from_slice(&24u16.to_le_bytes());
        file.extend_from_slice(&0u32.to_le_bytes());
        file.extend_from_slice(&(buf.len() as u32).to_le_bytes());
        file.extend_from_slice(&2835i32.to_le_bytes());
        file.extend_from_slice(&2835i32.to_le_bytes());
        file.extend_from_slice(&0u32.to_le_bytes());
        file.extend_from_slice(&0u32.to_le_bytes());
        file.extend_from_slice(&buf);

        std::fs::write(out, &file).map_err(|e| format!("写入截图失败：{e}"))?;
        Ok(())
    }
}
