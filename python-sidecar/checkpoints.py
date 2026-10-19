"""Atomic, content-addressed checkpoints. Paths inside records are relative."""
from dataclasses import asdict
from pathlib import Path
import hashlib
import importlib.metadata
import json
import os
from functools import lru_cache

from structure import Block, Unit

SCHEMA = 1


@lru_cache(maxsize=1)
def model_fingerprint():
    distribution = importlib.metadata.distribution("rapidocr")
    digest = hashlib.sha256(distribution.version.encode())
    for name in ("PP-OCRv6_det_small.onnx", "ch_ppocr_mobile_v2.0_cls_mobile.onnx", "PP-OCRv6_rec_small.onnx"):
        with Path(distribution.locate_file(f"rapidocr/models/{name}")).open("rb") as stream:
            for chunk in iter(lambda: stream.read(1024 * 1024), b""):
                digest.update(chunk)
    return digest.hexdigest()


def atomic_json(path, value):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(f".{os.getpid()}.tmp")
    with temporary.open("w", encoding="utf-8") as stream:
        json.dump(value, stream, ensure_ascii=False)
        stream.flush()
        os.fsync(stream.fileno())
    os.replace(temporary, path)


class Checkpoints:
    def __init__(self, root, source, mode, figures=True):
        self.root = None
        if root is None:
            return
        digest = hashlib.sha256()
        with Path(source).open("rb") as stream:
            for chunk in iter(lambda: stream.read(1024 * 1024), b""):
                digest.update(chunk)
        version = model_fingerprint()
        digest.update(f"{SCHEMA}:{version}:PP-OCRv6:{mode}:adaptive-v1:{figures}".encode())
        self.root = Path(root) / digest.hexdigest()
        self.root.mkdir(parents=True, exist_ok=True)

    def read(self, number):
        if self.root is None:
            return None
        try:
            record = json.loads((self.root / f"{number}.json").read_text(encoding="utf-8"))
            unit = record["unit"]
            if unit["number"] != number:
                return None
            unit = Unit(**{**unit, "blocks": [Block(**b) for b in unit["blocks"]]})
            for item in record["images"]:
                path = (self.root / item["path"]).resolve()
                if not path.is_relative_to(self.root.resolve()) or not path.is_file():
                    return None
                item["path"] = str(path)
            return unit, record["images"], record["skipped"]
        except (OSError, ValueError, TypeError, KeyError):
            return None

    def write(self, unit, images, skipped):
        if self.root is not None:
            items = [{**item, "path": str(Path(item["path"]).relative_to(self.root))} for item in images]
            atomic_json(self.root / f"{unit.number}.json", {"unit": asdict(unit), "images": items, "skipped": skipped})
