"""Page-scoped OCR, durable checkpoints and memory-bounded process workers."""
from concurrent.futures import ProcessPoolExecutor, wait, FIRST_COMPLETED
from contextlib import closing
import multiprocessing
import os
from pathlib import Path

from checkpoints import Checkpoints
from figures import Figures
from ocr_runtime import available_worker_slots, preferred_mode, worker_limit
from structure import Block, Unit, unicode_issues


def regions_for(page, native, objects):
    """Return PDF-canvas rectangles; None requests the full visible page."""
    if not native.strip() or unicode_issues(native):
        return [None]
    left, bottom, right, top = page.get_bbox()
    area = max(1, (right - left) * (top - bottom))
    regions = []
    for obj in objects:
        try:
            x0, y0, x1, y1 = obj.get_bounds()
        except Exception:
            return [None]  # unknown image geometry must not silently lose text
        box = [max(left, x0 - 2), max(bottom, y0 - 2), min(right, x1 + 2), min(top, y1 + 2)]
        if box[2] <= box[0] or box[3] <= box[1]:
            continue
        if (box[2] - box[0]) * (box[3] - box[1]) >= area * .8:
            return [None]
        regions.append(box)
    # Merge overlaps (including tiled images) to avoid duplicated recognition.
    merged = []
    while regions:
        box = regions.pop()
        changed = True
        while changed:
            changed = False
            for index, other in enumerate(regions):
                if box[0] <= other[2] and other[0] <= box[2] and box[1] <= other[3] and other[1] <= box[3]:
                    box = [min(box[0], other[0]), min(box[1], other[1]), max(box[2], other[2]), max(box[3], other[3])]
                    regions.pop(index)
                    changed = True
                    break
        merged.append(box)
    return merged


def render_region(page, region, scale):
    import pypdfium2 as pdfium
    width, height = page.get_size()
    crop = (0, 0, 0, 0)
    if region is not None:
        conv = pdfium.PdfPosConv(page, (0, 0, round(width * 4), round(height * 4), 0))
        l, b, r, t = region
        points = [conv.to_bitmap(x, y) for x in (l, r) for y in (b, t)]
        xs, ys = zip(*points)
        crop = (max(0, min(xs) / 4), max(0, height - max(ys) / 4),
                max(0, width - max(xs) / 4), max(0, min(ys) / 4))
    return page.render(scale=scale, crop=crop)


def extract_page(pdf, number, ocr, figure_dir):
    import pypdfium2 as pdfium
    import pypdfium2.raw as raw
    from parser import _native_blocks, same_text
    figures = Figures(figure_dir)
    with closing(pdf[number - 1]) as page, closing(page.get_textpage()) as textpage:
        blocks = _native_blocks(textpage)
        native = "\n".join(b.text for b in blocks)
        objects = list(page.get_objects(filter=[raw.FPDF_PAGEOBJ_IMAGE]))
        regions = regions_for(page, native, objects)
        extra = []
        sizes = sorted(b.size for b in blocks if b.size and b.size > 0)
        # Aim for legible glyphs; dense small print gets more pixels than large headings.
        scale = min(3.0, max(1.5, 32 / sizes[len(sizes) // 2])) if sizes else 2.5
        width, height = page.get_size()
        for region in regions:
            extent = max(width, height) if region is None else max(region[2] - region[0], region[3] - region[1])
            base = min(scale, 3000 / max(1, extent))
            attempts = [base]
            higher = min(3.5, 3000 / max(1, extent))
            if higher > base * 1.15:
                attempts.append(higher)
            recognized = []
            mapped = []
            for attempt in attempts:
                with closing(render_region(page, region, attempt)) as bitmap:
                    conversion = bitmap.get_posconv(page)
                    image = bitmap.to_pil()
                    try:
                        recognized = [(list(box), text) for box, text in ocr.lines(image)]
                        if recognized:
                            mapped = [([conversion.to_page(round(x), round(y)) for x, y in box], text)
                                      for box, text in recognized]
                    finally:
                        image.close()
                if recognized and getattr(ocr, "confidence", 1) >= .8:
                    break
            for points, text in mapped:
                xs, ys = zip(*points)
                covered = textpage.get_text_bounded(min(xs) - 2, min(ys) - 2, max(xs) + 2, max(ys) + 2)
                if same_text(text, covered):
                    continue
                extra.append(Block(text, kind="ocr", size=max(ys) - min(ys),
                                   bbox=[min(xs), min(ys), max(xs), max(ys)]))
        if objects:
            figures.extract_pdf_page(page, number)
        blocks.extend(b for b in extra if b.text.strip())
        display = pdfium.PdfPosConv(page, (0, 0, round(width * 4), round(height * 4), 0))
        def reading_key(block):
            if not block.bbox:
                return (float("inf"), 0)
            l, b, r, t = block.bbox
            points = [display.to_bitmap(x, y) for x in (l, r) for y in (b, t)]
            return min(y for _, y in points), min(x for x, _ in points)
        blocks.sort(key=reading_key)
        unit = Unit(number, f"第 {number} 页", blocks, round(width, 1), round(height, 1),
                    ocr=bool(extra), meta={"rotation": page.get_rotation(), "canvas": page.get_bbox()})
    return unit, figures.items, figures.skipped


_worker_pdf = None
_worker_ocr = None


def initialize_worker(path, mode, parent_pid):
    global _worker_pdf, _worker_ocr
    import pypdfium2 as pdfium
    from parser import Ocr
    from ocr_runtime import watch_parent
    watch_parent(parent_pid)
    _worker_pdf = pdfium.PdfDocument(path)
    _worker_ocr = Ocr(mode)


def worker_page(number, directory):
    return extract_page(_worker_pdf, number, _worker_ocr, directory)


def parse_pdf(path, ocr, figures, progress, cache_dir=None):
    import pypdfium2 as pdfium
    import chapters
    from parser import DocumentError, _read_toc, merge_lines
    mode = preferred_mode(ocr.mode)
    cache = Checkpoints(cache_dir, path, mode, figures.enabled)
    try:
        pdf = pdfium.PdfDocument(path)
    except pdfium.PdfiumError as error:
        if "password" in str(error).lower():
            raise DocumentError("PDF 已加密，请先在其他软件中解除密码保护后再导入") from error
        raise DocumentError(f"无法打开 PDF（文件可能已损坏）：{error}") from error
    with pdf:
        total, toc = len(pdf), _read_toc(pdf)
        units = {}
        records = {}
        missing = []
        def accept(record, cached=False):
            unit, images, skipped = record
            records[unit.number] = (images, skipped)
            if not cached:
                cache.write(unit, images, skipped)
            # Reapply global figure limits/deduplication to cached and fresh pages alike.
            before = len(figures.items)
            if cache.root is not None:
                for item in images:
                    figures.save_blob(Path(item["path"]).read_bytes(), unit.number, item["ocr_text"])
                figures.skipped += skipped
            units[unit.number] = unit
            progress(len(units), total)
            if hasattr(progress, "page"):
                progress.page(unit, figures.items[before:], total)
        for number in range(1, total + 1):
            cached = cache.read(number)
            if cached:
                accept(cached, True)
            else:
                missing.append(number)
        workers = worker_limit() if cache.root is not None and mode == "cpu" else 1
        def directory(number):
            return str(cache.root / f"images-{number}") if cache.root is not None and figures.enabled else None
        if workers == 1:
            for number in missing:
                if cache.root is None:
                    # Preserve global figure filtering when no checkpoint output was requested.
                    record = extract_page(pdf, number, ocr, None)
                    before = len(figures.items)
                    with closing(pdf[number - 1]) as page:
                        figures.extract_pdf_page(page, number)
                    units[number] = record[0]
                    progress(len(units), total)
                    if hasattr(progress, "page"):
                        progress.page(record[0], figures.items[before:], total)
                else:
                    accept(extract_page(pdf, number, ocr, directory(number)))
        else:
            with ProcessPoolExecutor(max_workers=workers, mp_context=multiprocessing.get_context("spawn"),
                    initializer=initialize_worker, initargs=(str(path), "cpu", os.getpid())) as pool:
                pending = {}
                remaining = iter(missing)
                exhausted = False
                while pending or not exhausted:
                    # Recheck memory between pages; never queue an entire PDF as bitmaps.
                    limit = min(workers, max(1, len(pending) + available_worker_slots()))
                    while not exhausted and len(pending) < limit:
                        number = next(remaining, None)
                        if number is None:
                            exhausted = True
                            break
                        pending[pool.submit(worker_page, number, directory(number))] = number
                    if pending:
                        done, _ = wait(pending, return_when=FIRST_COMPLETED)
                        for future in done:
                            pending.pop(future)
                            accept(future.result())
        if cache.root is not None:
            # Final image ordering/limits are source-order deterministic, including resumed jobs.
            ordered_figures = Figures(figures.out_dir, figures.max_total, figures.max_per_unit)
            for number in range(1, total + 1):
                images, skipped = records[number]
                for item in images:
                    ordered_figures.save_blob(Path(item["path"]).read_bytes(), number, item["ocr_text"])
                ordered_figures.skipped += skipped
            figures.items = ordered_figures.items
            figures.skipped = ordered_figures.skipped
        ordered = [units[n] for n in range(1, total + 1)]
        source = chapters.detect(ordered, "pdf", toc)
        for unit in ordered:
            merge_lines(unit)
        return ordered, source, "page"
