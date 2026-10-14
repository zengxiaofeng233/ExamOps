"""Shared data structures for structured extraction.

A document is a list of *units*: PDF pages, PPT slides, Word paragraphs or
blocks of text lines. Each unit holds ordered *blocks*; a block may be marked
as a heading (1 = chapter, 2 = section, ...). The desktop app builds chapters,
retrieval chunks and citations from this structure, so every block keeps the
unit (page / slide / paragraph / line range) it came from.
"""
from dataclasses import dataclass, field
from typing import Optional


@dataclass
class Block:
    text: str
    heading: Optional[int] = None
    # text | ocr | table | title | section | synthetic
    kind: str = "text"
    # Approximate font size (PDF line height in points) used by layout heuristics.
    size: Optional[float] = None
    source: Optional[str] = None


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
                {"text": b.text, "heading": b.heading, "kind": b.kind}
                for b in self.blocks if b.text
            ],
        }
