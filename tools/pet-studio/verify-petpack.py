import json
import struct
import sys
import zipfile
from pathlib import Path, PurePosixPath


EXPECTED = {"idle": 4, "walk": 6, "sit": 4, "sleep": 4, "reaction": 4}


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit("usage: verify-petpack.py <archive.petpack>")
    archive_path = Path(sys.argv[1])
    with zipfile.ZipFile(archive_path) as archive:
        assert archive.testzip() is None, "ZIP CRC check failed"
        names = archive.namelist()
        assert len(names) == len(set(names)), "duplicate ZIP paths"
        for name in names:
            path = PurePosixPath(name)
            assert not path.is_absolute() and ".." not in path.parts and "\\" not in name, f"unsafe path: {name}"
        manifest = json.loads(archive.read("pet.json"))
        assert manifest["schemaVersion"] == 1
        assert manifest["id"] == "test-pet"
        assert manifest["name"] == "测试桌宠"
        assert manifest["preview"] == "preview.png"
        assert manifest["normalizationMetric"] == "alpha-area-v1"
        expected_paths = {"pet.json", "preview.png"}
        for action, expected_count in EXPECTED.items():
            config = manifest["animations"][action]
            assert len(config["frames"]) == expected_count, f"unexpected {action} frame count"
            assert len(config["durations"]) == expected_count, f"unexpected {action} duration count"
            assert all(isinstance(duration, int) and 40 <= duration <= 10000 for duration in config["durations"])
            expected_paths.update(config["frames"])
        assert set(names) == expected_paths, "archive contains missing or unreferenced files"
        sizes = set()
        for name in expected_paths - {"pet.json"}:
            png = archive.read(name)
            assert png[:8] == b"\x89PNG\r\n\x1a\n", f"not a PNG: {name}"
            assert len(png) >= 26 and png[12:16] == b"IHDR", f"missing PNG IHDR: {name}"
            width, height = struct.unpack(">II", png[16:24])
            assert 16 <= width <= 512 and 16 <= height <= 512
            assert png[25] == 6, f"expected RGBA output with alpha channel: {name}"
            sizes.add((width, height))
        assert len(sizes) == 1, "all animation frames must share one canvas size"
    print("PASS: valid ZIP, safe paths, v1 manifest, all 22 RGBA frames, and no extra files")


if __name__ == "__main__":
    main()
