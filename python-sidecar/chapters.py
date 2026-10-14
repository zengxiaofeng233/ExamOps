"""Chapter / section recognition.

Sources, from most to least trustworthy:
  1. PDF bookmarks (outline)                    -> "outline"
  2. Word heading styles / outline levels       -> "styles"   (marked by parser.py)
  3. Markdown headings                          -> "markdown" (marked by parser.py)
  4. PPT sections, section-header layouts, slide titles -> "slides"
  5. Numbering patterns such as 第三章 / 3.2 / 一、 -> "patterns"
  6. PDF font-size layout                        -> "layout"
The first source that yields a usable structure wins; the app falls back to
page ranges when nothing is found ("none").

Design idea (labelled structure, data-not-instructions) inspired by the MIT
licensed Universal ExamPrep Skill by ZeKaiNie; the implementation is original.
"""
import re
from collections import Counter

from structure import Block

CN_DIGITS = {"零": 0, "〇": 0, "一": 1, "二": 2, "两": 2, "三": 3, "四": 4, "五": 5,
             "六": 6, "七": 7, "八": 8, "九": 9}
CN_UNITS = {"十": 10, "百": 100, "千": 1000}
NUM = r"[0-9０-９零〇一二三四五六七八九十百千两]+"
ROMAN = r"[ivxlcIVXLC]+"

# (rank, regex). Rank orders numbering families; levels are compressed later so
# the highest family present becomes level 1.
PATTERNS = [
    (1, re.compile(rf"^第\s*({NUM})\s*[章篇编讲](?=$|[\s:：、.．\-—])")),
    (1, re.compile(rf"^(?:chapter|chap\.|lecture|unit|part|module)\s+(\d+|{ROMAN})\b", re.I)),
    (2, re.compile(rf"^第\s*({NUM})\s*节(?=$|[\s:：、.．\-—])")),
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
    return re.sub(r"\W+", "", text, flags=re.UNICODE).casefold()


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
    text = line.strip()
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
    edge_keys = Counter()
    for unit in units:
        texts = [b for b in unit.blocks if b.kind == "text"]
        for block in texts[:2] + texts[-2:]:
            edge_keys[re.sub(r"\d+", "#", normalized(block.text))] += 1
    threshold = max(4, int(len(units) * 0.3))
    repeated = {key for key, count in edge_keys.items() if key and count >= threshold}
    page_number = re.compile(r"^(?:[-—–\s]*\d{1,4}[-—–\s]*|第\s*\d+\s*页(?:\s*共\s*\d+\s*页)?|page\s*\d+(?:\s*of\s*\d+)?|\d+\s*/\s*\d+)$", re.I)
    for unit in units:
        texts = [b for b in unit.blocks if b.kind == "text"]
        edges = {id(b) for b in texts[:2] + texts[-2:]}
        unit.blocks = [
            b for b in unit.blocks
            if id(b) not in edges or not (
                re.sub(r"\d+", "#", normalized(b.text)) in repeated or page_number.match(b.text.strip()))
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
                if block.heading is not None or block.kind == "ocr":
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
    return hits >= 4 or (hits >= 2 and any(normalized(b.text) in ("目录", "目次", "contents", "tableofcontents") for b in unit.blocks))


def apply_patterns(units, kinds=("text", "title", "synthetic")):
    """Numbering-pattern headings with TOC, duplicate and monotonic filtering."""
    candidates = []  # (unit, block, rank, number_tuple)
    for unit in units:
        if _is_toc_page(unit):
            continue
        for block in unit.blocks:
            if block.kind not in kinds or block.heading is not None:
                continue
            text = block.text.strip()
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
    for unit, block, rank, numbers in candidates:
        key = (rank, normalized(block.text))
        if key in seen_text:
            continue
        seen_text.add(key)
        if rank == 1 and numbers and numbers[0] is not None:
            if numbers[0] < last_number.get(1, 0):
                continue
            last_number[1] = numbers[0]
        kept.append((block, rank))
    if not kept:
        return 0
    levels = {rank: index + 1 for index, rank in enumerate(sorted({rank for _, rank in kept}))}
    for block, rank in kept:
        block.heading, block.source = levels[rank], "patterns"
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


def detect(units, kind, toc=None):
    """Marks headings in place and returns the structure source used."""
    if kind == "pdf":
        strip_furniture(units)
        if toc and apply_outline(units, toc) >= 2:
            return "outline"
        clear(units, "outline")
        if apply_patterns(units) >= 2:
            return "patterns"
        clear(units, "patterns")
        if apply_layout(units) >= 2:
            return "layout"
        clear(units, "layout")
        return "none"
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
