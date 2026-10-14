export interface TextSegment {
  text: string
  hit: boolean
}

function normalizeChar(c: string): string {
  const code = c.charCodeAt(0)
  if (code >= 0xff01 && code <= 0xff5e) return String.fromCharCode(code - 0xfee0).toLowerCase()
  return c.toLowerCase()
}

/** Splits text into highlighted / plain segments (case- and width-insensitive). */
export function highlightSegments(text: string, terms: string[]): TextSegment[] {
  const chars = Array.from(text)
  const lowered = chars.map(c => (c.length === 1 ? normalizeChar(c) : c))
  const ranges: Array<[number, number]> = []
  for (const term of terms) {
    const needle = Array.from(term).map(c => (c.length === 1 ? normalizeChar(c) : c))
    if (!needle.length) continue
    for (let i = 0; i + needle.length <= lowered.length; i++) {
      let match = true
      for (let j = 0; j < needle.length; j++) {
        if (lowered[i + j] !== needle[j]) { match = false; break }
      }
      if (match) {
        ranges.push([i, i + needle.length])
        i += needle.length - 1
      }
    }
  }
  if (!ranges.length) return [{ text, hit: false }]
  ranges.sort((a, b) => a[0] - b[0])
  const merged: Array<[number, number]> = []
  for (const range of ranges) {
    const last = merged[merged.length - 1]
    if (last && range[0] <= last[1]) last[1] = Math.max(last[1], range[1])
    else merged.push([...range])
  }
  const segments: TextSegment[] = []
  let cursor = 0
  for (const [start, end] of merged) {
    if (start > cursor) segments.push({ text: chars.slice(cursor, start).join(''), hit: false })
    segments.push({ text: chars.slice(start, end).join(''), hit: true })
    cursor = end
  }
  if (cursor < chars.length) segments.push({ text: chars.slice(cursor).join(''), hit: false })
  return segments
}
