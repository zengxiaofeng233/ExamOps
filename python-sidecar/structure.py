"""Shared data structures for structured extraction.

A document is a list of *units*: PDF pages, PPT slides, Word paragraphs or
blocks of text lines. Each unit holds ordered *blocks*; a block may be marked
as a heading (1 = chapter, 2 = section, ...). The desktop app builds chapters,
retrieval chunks and citations from this structure, so every block keeps the
unit (page / slide / paragraph / line range) it came from.
"""
from dataclasses import dataclass, field, asdict
from typing import Optional
import re
import unicodedata


def unicode_issues(text):
    """Report suspect mappings, never rewrite mathematical Unicode or PUA glyphs."""
    return sorted({f"U+{ord(c):04X}" for c in text if c == '\ufffd'
                   or unicodedata.category(c) in ('Co', 'Cs', 'Cn')
                   or (unicodedata.category(c) == 'Cc' and c not in '\n\r\t')})


def formula_like(text):
    return (any(unicodedata.category(c) == 'Sm' for c in text)
            or bool(re.search(r'\$|\\\(|\\\[|[⁰¹²³⁴⁵⁶⁷⁸⁹₀₁₂₃₄₅₆₇₈₉]|[∫∑∏√∞]', text)))


@dataclass
class Block:
    text: str
    heading: Optional[int] = None
    # text | ocr | table | title | section | synthetic
    kind: str = "text"
    # Approximate font size (PDF line height in points) used by layout heuristics.
    size: Optional[float] = None
    source: Optional[str] = None
    raw_text: Optional[str] = None
    # PDF canvas points: left, bottom, right, top (same space as PDFium).
    bbox: Optional[list] = None
    recognition: str = "native"
    issues: list = field(default_factory=list)
    formula: bool = False

    def __post_init__(self):
        if self.raw_text is None:
            self.raw_text = self.text
        self.issues = unicode_issues(self.raw_text)
        self.formula = self.formula or formula_like(self.text)
        if self.issues:
            self.recognition = "suspect"
        elif self.kind == "ocr":
            self.recognition = "unverified"


@dataclass
class Unit:
    number: int
    label: str
    blocks: list = field(default_factory=list)
    width: Optional[float] = None
    height: Optional[float] = None
    ocr: bool = False
    # Human-checkable location range: page/slide/paragraph number or line numbers.
    loc_start: int = 0
    loc_end: int = 0
    meta: dict = field(default_factory=dict)

    @property
    def text(self):
        return "\n".join(block.text for block in self.blocks if block.text)

    def to_json(self):
        return {
            "number": self.number,
            "label": self.label,
            "width": self.width,
            "height": self.height,
            "ocr": self.ocr,
            "loc_start": self.loc_start or self.number,
            "loc_end": self.loc_end or self.number,
            "blocks": [
                asdict(b)
                for b in self.blocks if b.text
            ],
        }
