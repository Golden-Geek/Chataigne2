"""Read only GCC's two root license files from the pinned compiler source bundle."""
from pathlib import Path, PurePosixPath
import re
import sys
import tarfile


def extract(archive, destination):
    wanted = {"COPYING.RUNTIME": "GCC-Runtime-Exception.txt", "COPYING3": "GCC-GPL-3.0.txt"}
    found = set()
    with tarfile.open(archive, mode="r|") as outer:
        for member in outer:
            if not re.fullmatch(r"source/gcc-[0-9.]+\.tar\.xz", member.name):
                continue
            with tarfile.open(fileobj=outer.extractfile(member), mode="r|xz") as gcc:
                for entry in gcc:
                    parts = PurePosixPath(entry.name).parts
                    if len(parts) != 2 or parts[1] not in wanted or not entry.isfile():
                        continue
                    assert 0 < entry.size < 1024 * 1024
                    (destination / wanted[parts[1]]).write_bytes(gcc.extractfile(entry).read())
                    found.add(parts[1])
                    if len(found) == len(wanted):
                        return
            break
    raise ValueError("Pinned compiler archive is missing root runtime license files")


if __name__ == "__main__":
    extract(Path(sys.argv[1]), Path(sys.argv[2]))
