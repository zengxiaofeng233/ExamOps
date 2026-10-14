"""Figure extraction: saves meaningful embedded images as PNG files.

Decorative images (tiny, blank, repeated logos) and full-page scans are skipped;
scanned pages are shown through page previews instead.
"""
from contextlib import closing
from io import BytesIO
from pathlib import Path
import hashlib

MIN_SIDE = 48
MIN_AREA = 96 * 96
MAX_PIXELS = 40_000_000
MAX_WIDTH = 1600


class Figures:
    def __init__(self, out_dir=None, max_total=400, max_per_unit=12):
        self.out_dir = Path(out_dir) if out_dir else None
        self.max_total = max_total
        self.max_per_unit = max_per_unit
        self.items = []
        self.skipped = 0
        self._hashes = set()
        self._per_unit = {}
        if self.out_dir:
            self.out_dir.mkdir(parents=True, exist_ok=True)

    @property
    def enabled(self):
        return self.out_dir is not None

    def _fingerprint(self, image):
        small = image.convert("L").resize((24, 24))
        return hashlib.sha1(small.tobytes()).hexdigest()

    def save_image(self, image, unit, ocr_text=""):
        """Saves a PIL image for a unit; returns the record or None when filtered."""
        if not self.enabled or len(self.items) >= self.max_total:
            return None
        width, height = image.size
        if min(width, height) < MIN_SIDE or width * height < MIN_AREA or width * height > MAX_PIXELS:
            return None
        if self._per_unit.get(unit, 0) >= self.max_per_unit:
            self.skipped += 1
            return None
        from PIL import ImageStat
        rgb = image.convert("RGB")
        if max(ImageStat.Stat(rgb).stddev) < 4:  # blank / single colour
            return None
        fingerprint = self._fingerprint(rgb)
        if fingerprint in self._hashes:  # logo or background repeated on every page
            return None
        self._hashes.add(fingerprint)
        if width > MAX_WIDTH:
            rgb = rgb.resize((MAX_WIDTH, round(height * MAX_WIDTH / width)))
        ordinal = self._per_unit.get(unit, 0) + 1
        self._per_unit[unit] = ordinal
        path = self.out_dir / f"{unit:05d}_{ordinal:02d}.png"
        rgb.save(path, format="PNG")
        record = {"page": unit, "ordinal": ordinal, "path": str(path),
                  "width": rgb.width, "height": rgb.height, "ocr_text": ocr_text.strip()}
        self.items.append(record)
        return record

    def save_blob(self, blob, unit, ocr_text=""):
        if not self.enabled:
            return None
        from PIL import Image
        try:
            with Image.open(BytesIO(blob)) as image:
                image.load()
                return self.save_image(image, unit, ocr_text)
        except Exception:
            # EMF/WMF and other vector formats cannot be rasterised offline.
            self.skipped += 1
            return None

    def extract_pdf_page(self, page, unit):
        """Saves image objects from one PDF page (not full-page scans)."""
        if not self.enabled:
            return
        import pypdfium2.raw as raw
        page_width, page_height = page.get_size()
        page_area = max(page_width * page_height, 1)
        for obj in page.get_objects(filter=[raw.FPDF_PAGEOBJ_IMAGE], max_depth=4):
            try:
                left, bottom, right, top = obj.get_bounds()
                if (right - left) * (top - bottom) >= page_area * 0.8:
                    continue
                try:
                    bitmap = obj.get_bitmap(render=True)
                except Exception:
                    bitmap = obj.get_bitmap(render=False)
                with closing(bitmap):
                    image = bitmap.to_pil()
                    try:
                        self.save_image(image, unit)
                    finally:
                        image.close()
            except Exception:
                self.skipped += 1
