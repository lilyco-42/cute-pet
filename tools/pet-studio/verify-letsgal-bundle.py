import json
import re
import sys
import zipfile
from pathlib import Path, PurePosixPath


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit("usage: verify-letsgal-bundle.py <bundle.zip>")
    with zipfile.ZipFile(Path(sys.argv[1])) as archive:
        assert archive.testzip() is None, "ZIP CRC check failed"
        names = archive.namelist()
        assert len(names) == len(set(names)), "duplicate ZIP paths"
        for name in names:
            path = PurePosixPath(name)
            assert not path.is_absolute() and ".." not in path.parts and "\\" not in name, f"unsafe path: {name}"
        fragment = json.loads(archive.read("characters.fragment.json"))
        assert fragment["format"] == "lain42.letsgal-character-fragment"
        assert fragment["version"] == 1
        character = fragment["character"]
        assert re.fullmatch(r"[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}", character["id"])
        assert character["name"] == "穗"
        assert character["expressions"] == [{"name": "微笑", "assetPath": f"characters/{character['id']}/portrait.png"}]
        asset_name = f"assets/{character['expressions'][0]['assetPath']}"
        assert set(names) == {asset_name, "characters.fragment.json", "IMPORT.md"}, "missing or extra bundle files"
        png = archive.read(asset_name)
        assert png.startswith(b"\x89PNG\r\n\x1a\n"), "portrait is not a PNG"
        assert png[12:16] == b"IHDR", "portrait PNG has no IHDR"
        import_guide = archive.read("IMPORT.md").decode("utf-8")
        assert "register-asset" in import_guide and "validate" in import_guide
    print("PASS: Let’sGal ZIP, safe paths, character fragment, portrait PNG, and CLI import guide")


if __name__ == "__main__":
    main()
