"""Chapter / section recognition.

Sources, from most to least trustworthy:
  1. PDF bookmarks (outline)                    -> "outline"
  2. Word heading styles / outline levels       -> "styles"   (marked by parser.py)
  3. Markdown headings                          -> "markdown" (marked by parser.py)
  4. PPT sections, section-header layouts, slide titles -> "slides"
  5. Numbering patterns such as 第三章 / 3.2 / 一、 -> "patterns"
  6. PDF font-size layout                        -> "layout"
Bookmarks and numbering patterns are combined; layout is a last resort.
No heading is invented when nothing is found ("none").

Design idea (labelled structure, data-not-instructions) inspired by the MIT
licensed Universal ExamPrep Skill by ZeKaiNie; the implementation is original.
"""
import re
import unicodedata
from collections import Counter
from dataclasses import replace

from structure import Block
import topics

CN_DIGITS = {"零": 0, "〇": 0, "一": 1, "二": 2, "两": 2, "三": 3, "四": 4, "五": 5,
             "六": 6, "七": 7, "八": 8, "九": 9}
CN_UNITS = {"十": 10, "百": 100, "千": 1000}
NUM = r"[0-9０-９零〇一二三四五六七八九十百千两]+"
ROMAN = r"[ivxlcIVXLC]+"

# (rank, regex). Preserve numbering depth; bookmark anchors can add a part level.
PATTERNS = [
    (1, re.compile(rf"^第\s*({NUM})\s*[章篇编讲]")),
    (1, re.compile(rf"^(?:chapter|chap\.|lecture|unit|part|module)\s+(\d+|{ROMAN})\b", re.I)),
    (2, re.compile(rf"^第\s*({NUM})\s*节")),
    (2, re.compile(r"^(?:section|§)\s*(\d+(?:\.\d+)?)", re.I)),
    (2, re.compile(r"^(\d{1,2})[.．](\d{1,2})(?![.．\d])\s*(?=[一-鿿A-Za-z（(《“\"])")),
    (3, re.compile(r"^(\d{1,2})[.．](\d{1,2})[.．](\d{1,2})(?![.．\d])\s*(?=[一-鿿A-Za-z（(《“\"])")),
    (3, re.compile(r"^([一二三四五六七八九十]{1,3})、\s*\S")),
    (4, re.compile(r"^[（(]([一二三四五六七八九十]{1,3})[)）]\s*\S")),
]
# "3 函数的极限" style top-level numbers only count when "3.x" subsections exist.
BARE_NUMBER = re.compile(r"^(\d{1,2})\s{1,3}(?=[一-鿿A-Z])\S")
TOC_LINE = re.compile(r"(?:\.{3,}|…{2,}|·{3,}|-{4,}|\s{3,}|\t)\s*[0-9ivxlcIVXLC]{1,4}\s*$")
TRAILING_PAGE = re.compile(r"\s+\d{1,4}$")
SENTENCE_END = tuple("。，；,;！？!?")
CONTINUED = re.compile(r"\s*[（(]?(?:续|續|cont\.?|continued)[)）]?\s*$|\s*\(\d+/\d+\)$", re.I)


def normalized(text):
    return re.sub(r"\W+", "", heading_text(text), flags=re.UNICODE).casefold()


def heading_text(text):
    # Comparison only: never apply NFKC to stored mathematical source text.
    return unicodedata.normalize('NFKC', text).strip()


def chinese_number(text):
    """'十二' -> 12, '３' -> 3, 'iv' -> 4. Returns None when not a number."""
    text = text.strip().translate(str.maketrans("０１２３４５６７８９", "0123456789"))
    if text.isdigit():
        return int(text)
    if re.fullmatch(ROMAN, text):
        values = {"i": 1, "v": 5, "x": 10, "l": 50, "c": 100}
        total, previous = 0, 0
        for char in reversed(text.lower()):
            value = values[char]
            total += -value if value < previous else value
            previous = max(previous, value)
        return total
    total, current = 0, 0
    for char in text:
        if char in CN_DIGITS:
            current = CN_DIGITS[char]
        elif char in CN_UNITS:
            total += (current or 1) * CN_UNITS[char]
            current = 0
        else:
            return None
    return total + current


def looks_like_heading(line):
    """Quick check used by the text parser to split a heading line from its paragraph."""
    text = heading_text(line)
    return heading_like(text) and any(pattern.match(text) for _, pattern in PATTERNS)


def heading_like(text, limit=50):
    text = text.strip()
    return (1 < len(text) <= limit and not text.endswith(SENTENCE_END)
            and not TOC_LINE.search(text) and sum(c.isdigit() for c in text) <= len(text) * 0.6)


# --------------------------------------------------------------------------- PDF furniture

def strip_furniture(units):
    """Removes running headers/footers and bare page numbers repeated across pages."""
    if len(units) < 4:
        return
    def edge_blocks(unit):
        texts = [b for b in unit.blocks if b.kind in ("text", "ocr") and not b.formula and not b.issues]
        if unit.meta.get('rotation', 0):
            return []  # do not infer horizontal margins from rotated canvas boxes
        if unit.height and any(b.bbox for b in texts):
            bottom, top = (unit.meta['canvas'][1], unit.meta['canvas'][3]) if 'canvas' in unit.meta else (0, unit.height)
            height = top - bottom
            return [b for b in texts if b.bbox and
                    (b.bbox[1] >= top - height * .10 or b.bbox[3] <= bottom + height * .10)]
        return texts[:2] + texts[-2:]
    edge_keys, page_offsets = Counter(), Counter()
    for unit in units:
        for key in {normalized(b.text) for b in edge_blocks(unit)}:
            edge_keys[key] += 1
        for offset in {unit.number - int(b.text.strip()) for b in edge_blocks(unit)
                       if re.fullmatch(r'\d{1,4}', b.text.strip())}:
            page_offsets[offset] += 1
    # Chapter-specific headers repeat over only a small part of a long textbook.
    # Geometry limits deletion to the margin, so a whole-book majority is wrong.
    threshold = 3 if any(u.height for u in units) else max(4, int(len(units) * 0.3))
    repeated = {key for key, count in edge_keys.items() if key and count >= threshold}
    numbered = {offset for offset, count in page_offsets.items() if count >= 3}
    page_number = re.compile(r"^(?:[-—–\s]*\d{1,4}[-—–\s]*|第\s*\d+\s*页(?:\s*共\s*\d+\s*页)?|page\s*\d+(?:\s*of\s*\d+)?|\d+\s*/\s*\d+)$", re.I)
    for unit in units:
        edges = {id(b) for b in edge_blocks(unit)}
        def furniture(block):
            text = block.text.strip()
            if re.fullmatch(r'\d{1,4}', text):
                # A repeated bare constant can be part of a formula. Page numbers
                # must advance with the PDF page number (allow front-matter offsets).
                return unit.number - int(text) in numbered
            return normalized(text) in repeated or page_number.match(text)
        unit.blocks = [
            b for b in unit.blocks
            if id(b) not in edges or not furniture(b)
        ]


# --------------------------------------------------------------------------- sources

def apply_outline(units, toc):
    """Marks bookmark titles as headings. toc: [(level0, title, page_index)]."""
    applied = 0
    by_page = {}
    for level, title, page_index in toc:
        title = re.sub(r"\s+", " ", title or "").strip()
        if not title or page_index is None or not (0 <= page_index < len(units)):
            continue
        by_page.setdefault(page_index, []).append((level + 1, title))
    for page_index, entries in by_page.items():
        unit = units[page_index]
        inserted = 0
        for level, title in entries:
            key = normalized(title)
            match = None
            for block in unit.blocks:
                if block.heading is not None:
                    continue
                text_key = normalized(block.text)
                if text_key and (text_key == key or (len(key) >= 2 and key in text_key and len(text_key) <= len(key) * 2 + 8)
                                 or (len(text_key) >= 4 and text_key in key)):
                    match = block
                    break
            if match:
                match.heading, match.source = level, "outline"
                if len(normalized(match.text)) < len(key):
                    # The title wraps over several lines; show the full bookmark title.
                    match.text = title
            else:
                unit.blocks.insert(inserted, Block(title, heading=level, kind="synthetic", source="outline"))
                inserted += 1
            applied += 1
    return applied


def _is_toc_page(unit):
    hits = sum(1 for b in unit.blocks if TOC_LINE.search(b.text.strip()))
    if hits >= 4 or (hits >= 2 and any(normalized(b.text) in ("目录", "目次", "contents", "tableofcontents") for b in unit.blocks)):
        return True
    # Scanned contents often have a separately detected page-number column.
    titles = [b for b in unit.blocks if looks_like_heading(b.text)]
    numbers = [b for b in unit.blocks if re.fullmatch(r'\d{1,4}', b.text.strip())]
    pairs = 0
    for title in titles:
        if not title.bbox:
            continue
        l, bottom, r, top = title.bbox
        if any(n.bbox and n.bbox[0] >= r - 5 and
               abs((n.bbox[1] + n.bbox[3]) / 2 - (bottom + top) / 2) <= max(top - bottom, 5)
               for n in numbers):
            pairs += 1
    return pairs >= 4 or (not any(b.bbox for b in unit.blocks) and len(titles) >= 4 and len(numbers) >= 4)


def apply_patterns(units, kinds=("text", "ocr", "title", "synthetic")):
    """Numbering-pattern headings with TOC, duplicate and monotonic filtering."""
    candidates = []  # (unit, block, rank, number_tuple)
    anchors = set()
    for unit in units:
        for block in unit.blocks:
            if block.heading is not None:
                for rank, pattern in PATTERNS:
                    if pattern.match(heading_text(block.text)):
                        anchors.add(block.heading - rank)
                        break
    # Honour a consistent bookmark hierarchy (e.g. parts above chapters).
    offset = next(iter(anchors)) if len(anchors) == 1 else 0
    for unit in units:
        if _is_toc_page(unit):
            continue
        for block in unit.blocks:
            if block.heading is not None:
                rank = max(1, block.heading - offset)
                numbers = ()
                for _, pattern in PATTERNS:
                    match = pattern.match(heading_text(block.text))
                    if match:
                        numbers = tuple(chinese_number(g) for g in match.groups() if g)
                        break
                candidates.append((unit, block, rank, numbers))
                continue
            if block.kind not in kinds or block.issues:
                continue
            text = heading_text(block.text)
            if not heading_like(text):
                continue
            for rank, pattern in PATTERNS:
                match = pattern.match(text)
                if match:
                    numbers = tuple(chinese_number(g) for g in match.groups() if g)
                    candidates.append((unit, block, rank, numbers))
                    break
            else:
                match = BARE_NUMBER.match(text)
                if match and len(text) <= 30:
                    candidates.append((unit, block, 0, (int(match.group(1)),)))

    # Bare "3 Title" counts only when a matching "3.x" section exists.
    sub_parents = {numbers[0] for _, _, rank, numbers in candidates if rank == 2 and len(numbers) >= 2}
    candidates = [c for c in candidates if c[2] != 0 or c[3][0] in sub_parents]
    candidates = [(u, b, 1 if r == 0 else r, n) for u, b, r, n in candidates]

    # Drop repeats (running headers) and numbers going backwards within a family.
    seen_text, last_number, kept = set(), {}, []
    chapter = None
    parents = {}
    for unit, block, rank, numbers in candidates:
        scope = tuple(parents.get(level) for level in range(1, rank))
        family = (scope, rank, len(numbers))
        if numbers and all(n is not None for n in numbers):
            if numbers <= last_number.get(family, ()) and block.heading is None:
                continue
            last_number[family] = numbers
        parents[rank] = normalized(block.text)
        for level in list(parents):
            if level > rank:
                del parents[level]
        if rank == 1:
            chapter = normalized(block.text)
        key = (chapter if rank > 1 else None, rank, normalized(block.text))
        if key in seen_text:
            continue
        seen_text.add(key)
        if block.heading is None:
            kept.append((block, rank))
    if not kept:
        return 0
    for block, rank in kept:
        block.heading, block.source = min(6, max(1, rank + offset)), "patterns"
    return len(kept)


def apply_layout(units):
    """PDF fallback: short lines set noticeably larger than body text."""
    sizes = Counter()
    for unit in units:
        for block in unit.blocks:
            if block.size and block.kind == "text":
                sizes[round(block.size)] += len(block.text)
    if not sizes:
        return 0
    body = sizes.most_common(1)[0][0]
    candidates = []
    for unit in units:
        page_hits = [b for b in unit.blocks
                     if b.kind == "text" and b.heading is None and b.size and b.size >= body * 1.25
                     and heading_like(b.text, 40) and not b.text.strip().isdigit()]
        if 0 < len(page_hits) <= 3:
            candidates.extend((unit.number, b) for b in page_hits)
    pages_by_size = {}
    for number, block in candidates:
        pages_by_size.setdefault(round(block.size), set()).add(number)
    usable = sorted((s for s, pages in pages_by_size.items() if len(pages) >= 2), reverse=True)[:3]
    if not usable:
        return 0
    level_of = {size: index + 1 for index, size in enumerate(usable)}
    seen, applied = set(), 0
    for _, block in candidates:
        size = round(block.size)
        key = normalized(block.text)
        if size in level_of and key not in seen:
            seen.add(key)
            block.heading, block.source = level_of[size], "layout"
            applied += 1
    return applied


def apply_slides(units, has_sections):
    """PPT: sections (already level-1 blocks) > section-header layouts / chapter titles > slide titles."""
    titles = [(u, b) for u in units for b in u.blocks if b.kind == "title"]
    chapter_titles = set()
    if not has_sections:
        for unit, block in titles:
            text = block.text.strip()
            layout = (unit.meta.get("layout") or "").lower()
            if PATTERNS[0][1].match(text) or PATTERNS[1][1].match(text) or "section header" in layout or "节标题" in layout:
                chapter_titles.add(id(block))
    previous = None
    for unit, block in titles:
        key = normalized(CONTINUED.sub("", block.text))
        if not key or not heading_like(block.text, 80):
            continue
        if id(block) in chapter_titles:
            block.heading, block.source = 1, "slides"
        elif key != previous:
            block.heading, block.source = 2, "slides"
        previous = key
    return sum(1 for _, b in titles if b.heading)


def clear(units, source):
    for unit in units:
        for block in unit.blocks:
            if block.source == source:
                block.heading = None
        unit.blocks = [b for b in unit.blocks if not (b.kind == "synthetic" and b.source == source)]


def join_heading_fragments(units):
    """Join a split chapter number/title on the same physical line only."""
    for unit in units:
        for block in list(unit.blocks):
            if not block.bbox or not re.fullmatch(rf'第\s*{NUM}\s*[章节篇编讲]', heading_text(block.text)):
                continue
            left, bottom, right, top = block.bbox
            height = top - bottom
            matches = [b for b in unit.blocks if b is not block and b.bbox
                       and right - height * .65 <= b.bbox[0] <= right + height
                       and abs((b.bbox[1] + b.bbox[3] - bottom - top) / 2) < height * .3
                       and .7 * height <= b.bbox[3] - b.bbox[1] <= 1.4 * height
                       and heading_like(b.text) and not b.formula and not b.issues
                       and not looks_like_heading(b.text)]
            if len(matches) == 1:
                tail = matches[0]
                joined = replace(block, text=block.text + ' ' + tail.text,
                                 raw_text=(block.raw_text or block.text) + ' ' + (tail.raw_text or tail.text),
                                 bbox=[left, min(bottom, tail.bbox[1]), tail.bbox[2], max(top, tail.bbox[3])])
                position = min(unit.blocks.index(block), unit.blocks.index(tail))
                unit.blocks.remove(block)
                unit.blocks.remove(tail)
                unit.blocks.insert(position, joined)


def detect(units, kind, toc=None):
    """Marks headings in place and returns the structure source used."""
    if kind == "pdf":
        join_heading_fragments(units)
        strip_furniture(units)
        outline = apply_outline(units, toc) if toc else 0
        patterns = apply_patterns(units)
        if outline or patterns:
            return "mixed" if outline and patterns else ("outline" if outline else "patterns")
        if apply_layout(units) >= 2:
            return "layout"
        clear(units, "layout")
        return "topic" if topics.apply(units) else "none"
    if kind == "pptx":
        has_sections = any(b.kind == "section" for u in units for b in u.blocks)
        found = apply_slides(units, has_sections)
        return "slides" if found or has_sections else "none"
    if kind in ("docx", "md"):
        marked = sum(1 for u in units for b in u.blocks if b.heading)
        if marked >= 2:
            return "styles" if kind == "docx" else "markdown"
        found = apply_patterns(units)
        if found + marked >= 2:
            return "patterns"
        return "styles" if marked and kind == "docx" else ("markdown" if marked else "none")
    if apply_patterns(units) >= 2:
        return "patterns"
    clear(units, "patterns")
    return "none"
