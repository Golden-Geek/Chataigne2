"""Write a small DXV3 YG10 MOV with independently known color and alpha planes."""

import json
import struct
import subprocess
import sys
from pathlib import Path


WIDTH = 128
HEIGHT = 64
RATE = 8
FRAMES = 3 * RATE
ALPHA = (0, 64, 160, 255)
# Full-range Y, Cg and Co. These are intentionally colorful after compositing.
COLORS = ((80, 116, 190), (112, 164, 80), (192, 128, 144), (128, 146, 172))


def stripe(x: int, frame: int) -> int:
    return (x // (WIDTH // len(ALPHA)) + frame // RATE) % len(ALPHA)


def bc4_constant(value: int) -> bytes:
    # Equal BC4 endpoints and index zero for all 16 pixels.
    return bytes((value, value)) + bytes(6)


def literal_bc5_section(blocks: bytes) -> bytes:
    # The first BC5 block is literal. The two constant opcode streams select
    # opcode 3 (literal) for each following block's two BC4 halves.
    op_count = len(blocks) // 16 - 1
    header = struct.pack("<III", 12 + len(blocks), op_count, op_count)
    return header + blocks + b"\x01\x03\x01\x03"


def sample(frame: int) -> bytes:
    luma_alpha = bytearray()
    chroma = bytearray()
    for _ in range(0, HEIGHT, 4):
        for x in range(0, WIDTH, 4):
            index = stripe(x, frame)
            luma_alpha += bc4_constant(COLORS[index][0]) + bc4_constant(ALPHA[index])
    for _ in range(0, HEIGHT // 2, 4):
        for x in range(0, WIDTH // 2, 4):
            _, cg, co = COLORS[stripe(2 * x, frame)]
            chroma += bc4_constant(co) + bc4_constant(cg)
    body = literal_bc5_section(luma_alpha) + literal_bc5_section(chroma)
    # YG10 is stored as a little-endian integer. Version 4 uses the DXV3 header.
    return b"01GY" + bytes((4, 0, 0, 0)) + struct.pack("<I", len(body)) + body


def atom(kind: bytes, data: bytes) -> bytes:
    return struct.pack(">I4s", 8 + len(data), kind) + data


def full_atom(kind: bytes, data: bytes, flags: int = 0) -> bytes:
    return atom(kind, struct.pack(">I", flags) + data)


def movie(samples: list[bytes]) -> bytes:
    assert all(len(frame) == len(samples[0]) for frame in samples)
    count = len(samples)
    container = atom(b"ftyp", b"qt  " + bytes(4) + b"qt  ")
    payload = atom(b"mdat", b"".join(samples))
    matrix = struct.pack(">9I", 1 << 16, 0, 0, 0, 1 << 16, 0, 0, 0, 1 << 30)
    movie_header = full_atom(
        b"mvhd",
        struct.pack(">IIIIIH10x", 0, 0, RATE, count, 1 << 16, 0x0100)
        + matrix
        + struct.pack(">24xI", 2),
    )
    track_header = full_atom(
        b"tkhd",
        struct.pack(">IIIII16x", 0, 0, 1, 0, count)
        + matrix
        + struct.pack(">II", WIDTH << 16, HEIGHT << 16),
        flags=3,
    )
    media_header = full_atom(b"mdhd", struct.pack(">IIIIHH", 0, 0, RATE, count, 0x55C4, 0))
    handler = full_atom(b"hdlr", b"mhlrvide" + bytes(12) + b"\x00")
    video_header = full_atom(b"vmhd", bytes(8), flags=1)
    data_info = atom(
        b"dinf",
        full_atom(b"dref", struct.pack(">I", 1) + full_atom(b"url ", b"", flags=1)),
    )
    description = atom(
        b"DXD3",
        struct.pack(
            ">6xH16xHHIIIH32sHh",
            1, WIDTH, HEIGHT, 72 << 16, 72 << 16, 0, 1, b"\x09DXV3 YG10", 24, -1,
        ),
    )
    # One video chunk containing consecutive equal-sized, independently decodable frames.
    sample_table = atom(
        b"stbl",
        full_atom(b"stsd", struct.pack(">I", 1) + description)
        + full_atom(b"stts", struct.pack(">III", 1, count, 1))
        + full_atom(b"stsc", struct.pack(">IIII", 1, 1, count, 1))
        + full_atom(b"stsz", struct.pack(">II", len(samples[0]), count))
        + full_atom(b"stco", struct.pack(">II", 1, len(container) + 8)),
    )
    media = atom(b"mdia", media_header + handler + atom(b"minf", video_header + data_info + sample_table))
    return container + payload + atom(b"moov", movie_header + atom(b"trak", track_header + media))


def verify(path: Path) -> None:
    decoded = subprocess.run(
        [
            "ffmpeg", "-v", "error", "-i", str(path), "-map", "0:v:0",
            "-pix_fmt", "yuva420p", "-c:v", "rawvideo", "-threads", "1", "-f", "rawvideo", "-",
        ],
        check=True,
        stdout=subprocess.PIPE,
    ).stdout
    frame_bytes = WIDTH * HEIGHT * 5 // 2
    assert len(decoded) == FRAMES * frame_bytes
    for frame in range(FRAMES):
        full_width = [stripe(x, frame) for x in range(WIDTH)]
        half_width = full_width[::2]
        expected = (
            bytes(COLORS[i][0] for i in full_width) * HEIGHT
            + bytes(COLORS[i][1] for i in half_width) * (HEIGHT // 2)
            + bytes(COLORS[i][2] for i in half_width) * (HEIGHT // 2)
            + bytes(ALPHA[i] for i in full_width) * HEIGHT
        )
        assert decoded[frame * frame_bytes : (frame + 1) * frame_bytes] == expected, frame
    print(json.dumps({"matchedPlaneFrames": FRAMES, "authoredAlphaLevels": ALPHA}))


if __name__ == "__main__":
    path = Path(sys.argv[1])
    with path.open("xb") as output:
        output.write(movie([sample(frame) for frame in range(FRAMES)]))
    verify(path)
