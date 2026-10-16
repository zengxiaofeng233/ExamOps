"""Conservative TF-IDF / TextTiling-style fallback, without a model or tokenizer.

Tiles end on existing block boundaries; lexical-cohesion valleys yield virtual
learning units, never asserted to be the author's chapter structure.
"""
import math
import re
from collections import Counter
from structure import Block

STOP = set('the and for with from that this into are was were not have has you your of to in on as is it an a'.split())
CN_STOP = set('我们 你们 他们 这个 那个 一个 可以 进行 以及 因此 但是 其中 对于 通过 所以'.split())


def tokens(text):
    words = []
    for part in re.findall(r'[A-Za-z]{3,}|[\u4e00-\u9fff]+', text.lower()):
        if part.isascii():
            if part not in STOP:
                words.append(part)
        else:
            words.extend(part[i:i + 2] for i in range(len(part) - 1) if part[i:i + 2] not in CN_STOP)
    return words


def apply(units):
    if any(b.heading is not None for u in units for b in u.blocks):
        return 0
    tiles, positions, pending = [], [], Counter()
    start = None
    for u, unit in enumerate(units):
        for b, block in enumerate(unit.blocks):
            if start is None:
                start = (u, b)
            if not block.issues and not block.formula:
                pending.update(tokens(block.text))
            if pending.total() >= 120:
                tiles.append(pending)
                positions.append(start)
                pending, start = Counter(), None
    if pending:
        tiles.append(pending)
        positions.append(start)
    if len(tiles) < 8 or sum(t.total() for t in tiles) < 900:
        return 0
    frequency = Counter(word for tile in tiles for word in tile)
    idf = {w: math.log((len(tiles) + 1) / (n + 1)) + 1 for w, n in frequency.items()}

    def vector(group):
        counts = sum(group, Counter())
        return {w: (1 + math.log(n)) * idf[w] for w, n in counts.items()}

    def cosine(a, b):
        norm = math.sqrt(sum(v*v for v in a.values()) * sum(v*v for v in b.values()))
        return sum(v*b.get(w, 0) for w, v in a.items()) / norm if norm else 1

    # Compare two tiles on each side; score valleys by their local peak depth.
    scores = {i: cosine(vector(tiles[i-2:i]), vector(tiles[i:i+2])) for i in range(2, len(tiles)-1)}
    depths = {i: max(scores[j] for j in range(max(2, i-3), i+1))
              + max(scores[j] for j in range(i, min(len(tiles)-2, i+3)+1)) - 2*s
              for i, s in scores.items()}
    mean = sum(depths.values()) / len(depths)
    deviation = math.sqrt(sum((d-mean)**2 for d in depths.values()) / len(depths))
    threshold = max(.25, mean + .5*deviation)
    candidates = [i for i, d in depths.items() if d >= threshold and scores[i] < .5
                  and scores[i] <= scores.get(i-1, 1) and scores[i] <= scores.get(i+1, 1)]
    boundaries = [0, len(tiles)]
    for i in sorted(candidates, key=lambda i: depths[i], reverse=True):
        left = max(b for b in boundaries if b < i)
        right = min(b for b in boundaries if b > i)
        if min(sum(t.total() for t in tiles[left:i]), sum(t.total() for t in tiles[i:right])) >= 300:
            boundaries.append(i)
    boundaries.sort()
    if len(boundaries) == 2:
        return 0
    insertions = []
    for number, (a, b) in enumerate(zip(boundaries, boundaries[1:]), 1):
        weights = vector(tiles[a:b])
        words = sorted(weights, key=weights.get, reverse=True)[:3]
        label = f"主题学习单元 {number}：{'、'.join(words)}"
        position = (0, 0) if a == 0 else positions[a]
        insertions.append((position, Block(label, heading=1, kind='synthetic', source='topic')))
    for (u, b), block in reversed(insertions):
        units[u].blocks.insert(b, block)
    return len(insertions)
