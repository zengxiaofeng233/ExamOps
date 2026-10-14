"""PDF page previews rendered on demand (cached by the desktop app)."""
from contextlib import closing
from pathlib import Path

MAX_WIDTH = 2400


def render_pages(path, pages, out_dir, width=1200):
    import pypdfium2 as pdfium

    width = max(200, min(int(width), MAX_WIDTH))
    out_dir = Path(out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    results = []
    with pdfium.PdfDocument(path) as pdf:
        total = len(pdf)
        for number in pages:
            number = int(number)
            if not 1 <= number <= total:
                continue
            with closing(pdf[number - 1]) as page:
                page_width, _ = page.get_size()
                scale = width / max(page_width, 1)
                with closing(page.render(scale=scale)) as bitmap:
                    image = bitmap.to_pil()
                    target = out_dir / f"p{number}_w{width}.png"
                    try:
                        image.save(target, format="PNG")
                        results.append({"page": number, "path": str(target),
                                        "width": image.width, "height": image.height})
                    finally:
                        image.close()
    return results
