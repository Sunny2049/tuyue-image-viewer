"""给 windres 用的极简预处理器替身。

这台机器只有 MSYS2 的 binutils，没有 gcc，windres 默认调 gcc -E 就会失败。
我们的 .rc 里没有 #include / #define，直接原样透传即可
（windres 要么读 stdout，要么认 -o 输出文件，两种都兼容）。
"""
import sys

argv = sys.argv[1:]
out = None
inputs = []
i = 0
while i < len(argv):
    a = argv[i]
    if a == "-o" and i + 1 < len(argv):
        out = argv[i + 1]
        i += 2
        continue
    if a.startswith("-o") and len(a) > 2:
        out = a[2:]
        i += 1
        continue
    if a.startswith("-"):
        i += 1
        continue
    inputs.append(a)
    i += 1

if not inputs:
    sys.exit(1)

data = open(inputs[-1], "rb").read()
if out:
    with open(out, "wb") as f:
        f.write(data)
else:
    sys.stdout.buffer.write(data)
