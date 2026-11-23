"""检查 exe 是否真的带上了图标和版本信息（只读）"""
import ctypes
from ctypes import wintypes

exe = r"E:\Indie Maker\img2txt\app\target\x86_64-pc-windows-gnu\release\图阅.exe"

shell32 = ctypes.windll.shell32
n = shell32.ExtractIconExW(exe, -1, None, None, 0)
print("icons:", n)

ver = ctypes.windll.version
size = ver.GetFileVersionInfoSizeW(exe, None)
print("versioninfo_bytes:", size)
if size:
    buf = ctypes.create_string_buffer(size)
    if ver.GetFileVersionInfoW(exe, 0, size, buf):
        ptr = ctypes.c_void_p()
        length = wintypes.UINT()
        for key in ("FileDescription", "ProductName", "FileVersion", "OriginalFilename"):
            sub = "080404b0"  # zh-CN, 1200
            if ver.VerQueryValueW(buf, f"\\StringFileInfo\\{sub}\\{key}", ctypes.byref(ptr), ctypes.byref(length)):
                print(key, "=", ctypes.wstring_at(ptr, length.value).rstrip("\x00"))
            else:
                print(key, "= <missing>")
