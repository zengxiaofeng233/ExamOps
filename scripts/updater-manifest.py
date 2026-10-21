"""Build a Tauri updater manifest from the versioned, signed NSIS artifact."""
import json
import os
from pathlib import Path
from urllib.parse import quote


def build_manifest(root: Path, repository: str, tag: str) -> dict:
    version = json.loads((root / "src-tauri/tauri.conf.json").read_text(encoding="utf-8"))["version"]
    if tag != f"v{version}":
        raise ValueError(f"Release tag {tag!r} must match v{version}")
    name = f"ExamOps_{version}_x64-setup.exe"
    bundle = root / "src-tauri/target/release/bundle/nsis"
    if not (bundle / name).is_file() or (bundle / name).stat().st_size == 0:
        raise ValueError(f"Missing installer: {name}")
    signature = (bundle / f"{name}.sig").read_text(encoding="utf-8").strip()
    if not signature:
        raise ValueError("Updater signature is empty")
    return {
        "version": version,
        "platforms": {
            "windows-x86_64": {
                "signature": signature,
                "url": f"https://github.com/{repository}/releases/download/{quote(tag, safe='')}/{quote(name, safe='')}",
            }
        },
    }


if __name__ == "__main__":
    root = Path(__file__).resolve().parents[1]
    version = json.loads((root / "src-tauri/tauri.conf.json").read_text(encoding="utf-8"))["version"]
    tag = os.environ.get("GITHUB_REF_NAME", f"v{version}") if os.environ.get("GITHUB_REF_TYPE") == "tag" else f"v{version}"
    manifest = build_manifest(root, os.environ.get("GITHUB_REPOSITORY", "zengxiaofeng233/ExamOps"), tag)
    (root / "src-tauri/target/release/bundle/nsis/latest.json").write_text(
        json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
