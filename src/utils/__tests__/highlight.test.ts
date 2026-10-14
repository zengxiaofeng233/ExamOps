import { describe, expect, it } from 'vitest'
import { highlightSegments } from '../highlight'

describe('highlightSegments', () => {
  it('highlights Chinese terms and ignores case and full-width letters', () => {
    const segments = highlightSegments('函数的导数与ＬＩＭＩＴ', ['导数', 'limit'])
    expect(segments.filter(s => s.hit).map(s => s.text)).toEqual(['导数', 'ＬＩＭＩＴ'])
  })

  it('merges overlapping matches', () => {
    const segments = highlightSegments('几何意义', ['几何', '何意'])
    expect(segments).toEqual([{ text: '几何意', hit: true }, { text: '义', hit: false }])
  })

  it('returns the text unchanged without matches', () => {
    expect(highlightSegments('没有匹配', ['导数'])).toEqual([{ text: '没有匹配', hit: false }])
  })
})
