import hashlib
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from zipfile import ZIP_DEFLATED, ZipFile


ROOT = Path(__file__).resolve().parents[1]


def digest(data):
    return hashlib.sha256(data).hexdigest()


def package(version, images, output):
    subprocess.run(["cargo", "xtask", "verify-release", images], cwd=ROOT, check=True)
    images = Path(images)
    output = Path(output)
    output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=output) as tmp:
        stage = Path(tmp)
        archives = []
        included = []
        for board in ("paintbrush", "toucan"):
            kit = stage / f"kb.wlt-{board}-{version}"
            subprocess.run(["cargo", "xtask", "install-kit", board, images, kit], cwd=ROOT, check=True)
            files = sorted(kit.iterdir())
            firmware = sorted(kit.glob("*.uf2"))
            if len(firmware) != 3 or {p.name for p in files if p not in firmware} != {"INSTALL.txt", "BRAILLE.txt", "KEYMAP.txt", "flash-uf2.sh"}:
                raise ValueError(f"incomplete install kit: {board}")
            included.extend(p.name for p in firmware)
            sums = "".join(f"{digest(p.read_bytes())}  {p.name}\n" for p in files)
            (kit / "SHA256SUMS").write_text(sums, encoding="ascii")
            archive = stage / (kit.name + ".zip")
            with ZipFile(archive, "w", compression=ZIP_DEFLATED) as bundle:
                for path in sorted(kit.iterdir()):
                    bundle.write(path, f"{kit.name}/{path.name}")
            with ZipFile(archive) as bundle:
                if bundle.testzip() is not None:
                    raise ValueError(f"invalid archive: {archive.name}")
            archives.append(archive)
        if sorted(included) != sorted(p.name for p in images.glob("*.uf2")):
            raise ValueError("kits do not partition the production images")
        (stage / "SHA256SUMS").write_text("".join(f"{digest(p.read_bytes())}  {p.name}\n" for p in archives), encoding="ascii")
        for path in [*archives, stage / "SHA256SUMS"]:
            shutil.move(path, output / path.name)


if __name__ == "__main__":
    package(sys.argv[1], (ROOT / "dist/release").resolve(), (ROOT / "dist/packages").resolve())
