import { describe, expect, it } from 'vitest'
import { citedLabels, renderMarkdown } from '../markdown'

describe('renderMarkdown', () => {
  it('escapes raw HTML from model output', () => {
    const html = renderMarkdown('<img src=x onerror="alert(1)"> <script>alert(2)</script>')
    expect(html).not.toContain('<img')
    expect(html).not.toContain('<script')
    expect(html).toContain('&lt;script&gt;')
  })

  it('turns citations into buttons and provenance tags into badges', () => {
    const html = renderMarkdown('【教材】导数是变化率 [S1]，见 [S2, S3]。\n\n【补充】例子【S4】')
    expect(html).toContain('data-cite="S1"')
    expect(html).toContain('data-cite="S3"')
    expect(html).toContain('data-cite="S4"')
    expect(html).toContain('source-tag-textbook')
    expect(html).toContain('source-tag-supplement')
  })

  it('renders math and marks links as external', () => {
    const html = renderMarkdown("$f'(x)$ 与 [链接](https://example.com)")
    expect(html).toContain('katex')
    expect(html).toContain('data-external="1"')
  })

  it('leaves ordinary brackets alone', () => {
    const html = renderMarkdown('区间 [a, b] 与 [Section 2]')
    expect(html).not.toContain('data-cite')
  })
})

describe('citedLabels', () => {
  it('collects labels in order without duplicates', () => {
    expect(citedLabels('见 [S2] 和 【S1】 以及 [S2, S5]')).toEqual(['S2', 'S1', 'S5'])
  })
})
