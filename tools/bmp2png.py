"""极简 BMP(24/32位) -> PNG 转换（只用标准库，写完立刻可看）"""
import struct
import sys
import zlib


def read_bmp(path):
    with open(path, "rb") as f:
        data = f.read()
    assert data[:2] == b"BM", "不是 BMP"
    pix_off = struct.unpack_from("<I", data, 10)[0]
    hdr = struct.unpack_from("<IiiHH", data, 14)
    width, height, _, bpp = hdr[1], hdr[2], hdr[3], hdr[4]
    bottom_up = height > 0
    height = abs(height)
    stride = ((width * bpp // 8) + 3) // 4 * 4
    rows = []
    for y in range(height):
        off = pix_off + y * stride
        row = data[off:off + width * bpp // 8]
        px = bytearray()
        for x in range(width):
            b, g, r = row[x * bpp // 8], row[x * bpp // 8 + 1], row[x * bpp // 8 + 2]
            px += bytes((r, g, b))
        rows.append(px)
    if bottom_up:
        rows.reverse()
    return width, height, rows


def write_png(path, width, height, rows):
    raw = b"".join(b"\x00" + bytes(r) for r in rows)

    def chunk(tag, payload):
        return (struct.pack(">I", len(payload)) + tag + payload
                + struct.pack(">I", zlib.crc32(tag + payload) & 0xFFFFFFFF))

    png = b"\x89PNG\r\n\x1a\n"
    png += chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(raw, 6))
    png += chunk(b"IEND", b"")
    with open(path, "wb") as f:
        f.write(png)


if __name__ == "__main__":
    src, dst = sys.argv[1], sys.argv[2]
    w, h, rows = read_bmp(src)
    write_png(dst, w, h, rows)
    print(f"{dst} {w}x{h}")
