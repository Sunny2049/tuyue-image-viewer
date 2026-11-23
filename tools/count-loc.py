# -*- coding: utf-8 -*-
"""统计项目代码行数（跳过第三方 vendor 与二进制）"""
import os, json, re

ROOT = r"E:\Indie Maker\img2txt"

GROUPS = {
    "Rust 后端": [("app/src", ".rs"), ("app", "build.rs")],
    "前端页面": [("app/ui", ".js"), ("app/ui", ".html"), ("app/ui", ".css")],
    "编译资源": [("app/res", ".rc")],
    "构建工具": [("tools", ".py"), ("tools", ".ps1"), ("tools", ".js"), ("tools", ".html")],
    "文档": [("", ".md")],
}
VENDOR = os.path.join(ROOT, "app", "ui", "vendor")


def classify(path):
    if os.path.abspath(path).startswith(os.path.abspath(VENDOR)):
        return "vendor"
    return "own"


def count_lines(path, ext):
    with open(path, "r", encoding="utf-8", errors="replace") as f:
        raw = f.read()
    lines = raw.splitlines()
    total = len(lines)
    blank = sum(1 for l in lines if not l.strip())
    code = 0
    comment = 0
    in_block = False
    for l in lines:
        s = l.strip()
        if not s:
            continue
        if ext in (".rs", ".js", ".css"):
            if in_block:
                comment += 1
                if "*/" in s:
                    in_block = False
                continue
            if s.startswith("/*"):
                comment += 1
                if "*/" not in s:
                    in_block = True
                continue
            if s.startswith("//"):
                comment += 1
                continue
            code += 1
        elif ext in (".py", ".ps1"):
            if s.startswith("#") or s.startswith("<#"):
                comment += 1
                continue
            code += 1
        elif ext == ".html":
            if s.startswith("<!--"):
                comment += 1
                continue
            code += 1
        else:
            code += 1
    return {"total": total, "blank": blank, "comment": comment, "code": code, "bytes": len(raw.encode("utf-8"))}


result = {}
for group, patterns in GROUPS.items():
    rows = []
    for sub, ext in patterns:
        d = os.path.join(ROOT, sub) if sub else ROOT
        if not os.path.isdir(d):
            continue
        for name in sorted(os.listdir(d)):
            p = os.path.join(d, name)
            if not os.path.isfile(p) or not name.endswith(ext):
                continue
            if sub == "" and name == "README.md":
                pass
            rows.append((name, count_lines(p, ext)))
    if rows:
        result[group] = rows

vend = []
if os.path.isdir(VENDOR):
    for name in sorted(os.listdir(VENDOR)):
        p = os.path.join(VENDOR, name)
        if os.path.isfile(p):
            vend.append((name, count_lines(p, ".js")))
if vend:
    result["第三方库(未计入)"] = vend

out = r"E:\Indie Maker\img2txt\.smoke\loc.json"
with open(out, "w", encoding="utf-8") as f:
    json.dump(result, f, ensure_ascii=False, indent=1)

for g, rows in result.items():
    t = sum(r[1]["total"] for r in rows)
    c = sum(r[1]["code"] for r in rows)
    print("%-16s files=%2d total=%5d code=%5d" % (g, len(rows), t, c))
