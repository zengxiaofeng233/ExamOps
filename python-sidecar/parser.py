"""Plain-text extraction. OCR models are local and loaded only when needed."""
from io import BytesIO
from pathlib import Path
from contextlib import closing
import re


class Ocr:
    def __init__(self):
        self.engine = None

    def lines(self, image):
        if self.engine is None:
            import rapidocr
            from rapidocr import RapidOCR

            models = Path(rapidocr.__file__).parent / "models"
            self.engine = RapidOCR(params={
                "Global.log_level": "error",
                "Det.model_path": str(models / "PP-OCRv6_det_small.onnx"),
                "Cls.model_path": str(models / "ch_ppocr_mobile_v2.0_cls_mobile.onnx"),
                "Rec.model_path": str(models / "PP-OCRv6_rec_small.onnx"),
                "EngineConfig.onnxruntime.intra_op_num_threads": 2,
                "EngineConfig.onnxruntime.inter_op_num_threads": 1,
            })
        result = self.engine(image)
        if result.txts is None:
            return []
        return list(zip(result.boxes, result.txts))

    def image_text(self, blob):
        from PIL import Image
        with Image.open(BytesIO(blob)) as image:
            return "\n".join(text for _, text in self.lines(image.convert("RGB")))


def normalized(text):
    return re.sub(r"\W+", "", text, flags=re.UNICODE).casefold()


def parse_pdf(path, ocr):
    import pypdfium2 as pdfium
    import pypdfium2.raw as raw

    sections = []
    with pdfium.PdfDocument(path) as pdf:
        for number in range(len(pdf)):
            with closing(pdf[number]) as page, closing(page.get_textpage()) as textpage:
                native = textpage.get_text_bounded().strip()
                extra = []
                has_images = any(page.get_objects(filter=[raw.FPDF_PAGEOBJ_IMAGE]))
                if has_images or not native:
                    # Render one page at a time; coordinates account for rotation/crop boxes.
                    scale = min(2.5, 3000 / max(page.get_size()))
                    with closing(page.render(scale=scale)) as bitmap:
                        conversion = bitmap.get_posconv(page)
                        image = bitmap.to_pil()
                        try:
                            for box, text in ocr.lines(image):
                                points = [conversion.to_page(round(x), round(y)) for x, y in box]
                                xs, ys = zip(*points)
                                covered = textpage.get_text_bounded(
                                    min(xs) - 2, min(ys) - 2, max(xs) + 2, max(ys) + 2
                                )
                                # Only deduplicate text from the SAME location, not equal
                                # phrases elsewhere in the page or on another page.
                                recognized, existing = normalized(text), normalized(covered)
                                if existing and recognized and (recognized in existing or
                                        (existing in recognized and len(existing) / len(recognized) > 0.9)):
                                    continue
                                extra.append(text)
                        finally:
                            image.close()
                body = "\n".join(part for part in [native, *extra] if part)
                if body:
                    sections.append(f"[第 {number + 1} 页]\n{body}")
    return "\n\n".join(sections)


def parse_docx(path, ocr):
    from docx import Document
    from docx.oxml.ns import qn

    document = Document(path)
    parts = []
    # Walk body XML so text, tables and inline/floating images stay in document order.
    for paragraph in document.element.body.iter(qn("w:p")):
        line = []
        for node in paragraph.iter():
            if node.tag == qn("w:t") and node.text:
                line.append(node.text)
            elif node.tag in (qn("w:tab"), qn("w:br")):
                line.append("\t" if node.tag == qn("w:tab") else "\n")
            elif node.tag == qn("a:blip"):
                rel_id = node.get(qn("r:embed"))
                if rel_id:
                    line.append("\n" + ocr.image_text(document.part.related_parts[rel_id].blob) + "\n")
        if "".join(line).strip():
            parts.append("".join(line).strip())
    return "\n".join(parts)


def parse_pptx(path, ocr):
    from pptx import Presentation
    from pptx.enum.shapes import MSO_SHAPE_TYPE

    def shape_text(shapes):
        parts = []
        for shape in shapes:
            if shape.shape_type == MSO_SHAPE_TYPE.GROUP:
                parts.extend(shape_text(shape.shapes))
            elif shape.shape_type == MSO_SHAPE_TYPE.PICTURE:
                parts.append(ocr.image_text(shape.image.blob))
            elif shape.has_table:
                parts.extend("\t".join(cell.text for cell in row.cells if not cell.is_spanned)
                             for row in shape.table.rows)
            elif shape.has_text_frame:
                parts.append(shape.text)
        return parts

    sections = []
    for number, slide in enumerate(Presentation(path).slides, 1):
        body = "\n".join(part for part in shape_text(slide.shapes) if part.strip())
        if body:
            sections.append(f"[第 {number} 张幻灯片]\n{body}")
    return "\n\n".join(sections)


def parse_document(path):
    path = Path(path)
    ocr = Ocr()
    suffix = path.suffix.lower()
    if suffix == ".pdf":
        content = parse_pdf(path, ocr)
    elif suffix == ".docx":
        content = parse_docx(path, ocr)
    elif suffix == ".pptx":
        content = parse_pptx(path, ocr)
    elif suffix in (".txt", ".md", ".markdown"):
        import chardet
        data = path.read_bytes()
        try:
            content = data.decode("utf-8-sig")
        except UnicodeDecodeError:
            encoding = chardet.detect(data)["encoding"]
            if not encoding:
                raise ValueError("无法识别文本编码，请另存为 UTF-8 后导入")
            content = data.decode(encoding)
    else:
        raise ValueError("不支持的文件格式")
    content = content.strip()
    if not content:
        raise ValueError("未提取到正文（已对图片尝试 OCR），请检查资料内容或扫描清晰度")
    return content
