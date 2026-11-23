//! 单实例：第二次双击图片时，把路径转交给已在运行的窗口

use std::sync::mpsc::{channel, Receiver};

const MUTEX: windows::core::PCWSTR = windows::core::w!("Local\\TuyueViewerMutex_v1");
const EVENT: windows::core::PCWSTR = windows::core::w!("Local\\TuyueViewerWake_v1");

fn pending_file() -> std::path::PathBuf {
    crate::util::local_dir().join("pending-open.txt")
}

/// 返回 (是否第一个实例, 转交路径的接收端)
pub fn acquire() -> (bool, Option<Receiver<String>>) {
    use windows::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
    use windows::Win32::System::Threading::CreateMutexW;

    let handle = unsafe { CreateMutexW(None, false, MUTEX) };
    let h = match handle {
        Ok(h) => h,
        Err(_) => return (true, None),
    };
    let already = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;

    if already {
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(h);
        }
        return (false, None);
    }

    // 第一个实例：句柄要留到进程结束，故意不关闭
    std::mem::forget(h);

    let (tx, rx) = channel::<String>();
    std::thread::spawn(move || {
        use windows::Win32::Foundation::WAIT_OBJECT_0;
        use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject};
        let ev = match unsafe { CreateEventW(None, false, false, EVENT) } {
            Ok(e) => e,
            Err(_) => return,
        };
        loop {
            let r = unsafe { WaitForSingleObject(ev, 800) };
            if r == WAIT_OBJECT_0 {
                if let Ok(s) = std::fs::read_to_string(pending_file()) {
                    let _ = std::fs::remove_file(pending_file());
                    let p = s.trim().to_string();
                    if !p.is_empty() && tx.send(p).is_err() {
                        break;
                    }
                }
            }
        }
    });

    (true, Some(rx))
}

/// 第二个实例：把路径写入中转文件并唤醒第一个实例
pub fn forward_to_first(path: &str) {
    use windows::Win32::System::Threading::{OpenEventW, SetEvent, EVENT_MODIFY_STATE};
    if !path.is_empty() {
        let _ = std::fs::write(pending_file(), path);
    }
    unsafe {
        if let Ok(ev) = OpenEventW(EVENT_MODIFY_STATE, false, EVENT) {
            let _ = SetEvent(ev);
            let _ = windows::Win32::Foundation::CloseHandle(ev);
        }
    }
}
