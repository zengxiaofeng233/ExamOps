import { afterAll, afterEach, beforeAll, describe, expect, it, vi } from 'vitest'
import { createApp, nextTick, type App } from 'vue'
import DocumentReader from '../DocumentReader.vue'
import { DocumentService } from '@/services/document'

vi.mock('@/services/document', () => ({ DocumentService: {
  sections: vi.fn(), pages: vi.fn(), preview: vi.fn(), editSection: vi.fn()
} }))

let app: App | undefined
const originalScroll = HTMLElement.prototype.scrollTo
beforeAll(() => { HTMLElement.prototype.scrollTo = vi.fn() })
afterAll(() => { HTMLElement.prototype.scrollTo = originalScroll })
afterEach(() => { app?.unmount(); document.body.innerHTML = ''; vi.resetAllMocks() })

describe('reader source fidelity and section boundaries', () => {
  it('shows only the selected section on a shared page and exposes unmodified formula text', async () => {
    vi.mocked(DocumentService.sections).mockResolvedValue([
      { id: 1, document_id: 1, parent_id: null, ordinal: 0, level: 1, title: '甲章', page_start: 1, page_end: 1, block_start: 0, block_end: 1, char_count: 20, loc_label: '第 1 页' },
      { id: 2, document_id: 1, parent_id: null, ordinal: 1, level: 1, title: '乙章', page_start: 1, page_end: 1, block_start: 2, block_end: 3, char_count: 20, loc_label: '第 1 页' }
    ])
    const formula = '𝑥² + ∫₀¹ f(x) dx ≠ α'
    vi.mocked(DocumentService.pages).mockResolvedValue([{
      page_number: 1, label: '第 1 页', loc_start: 1, loc_end: 1, width: 600, height: 800, has_ocr: true, images: [],
      blocks: [{ text: '甲章', heading: 1, kind: 'text' },
        { text: formula, raw_text: formula, heading: null, kind: 'ocr', recognition: 'unverified', formula: true },
        { text: '乙章', heading: 1, kind: 'text' }, { text: '乙章专属正文', heading: null, kind: 'text' }]
    }])
    const host = document.createElement('div')
    document.body.append(host)
    app = createApp(DocumentReader, { document: {
      id: 1, course_id: 1, filename: '样例.pdf', file_type: 'pdf', file_path: '', file_size: 1,
      status: 'completed', created_at: '', page_count: 1, loc_kind: 'page', char_count: 40,
      structure_source: 'patterns', warnings: [], indexed_at: null, section_count: 2, image_count: 0
    } })
    app.component('RouterLink', { template: '<a><slot /></a>' })
    app.mount(host)
    await vi.waitFor(() => expect(host.querySelector('pre')?.textContent).toBe(formula))
    expect(host.textContent).toContain('公式或字符识别待核验')
    expect(host.textContent).not.toContain('乙章专属正文')
    const button = [...host.querySelectorAll<HTMLButtonElement>('nav button')].find(b => b.textContent?.includes('乙章'))!
    button.click()
    await nextTick()
    await vi.waitFor(() => expect(host.textContent).toContain('乙章专属正文'))
    expect(host.querySelector('pre')).toBeNull()
    expect(host.querySelector('article')?.textContent).not.toContain('甲章')
    const sections = await DocumentService.sections(1)
    vi.mocked(DocumentService.editSection).mockResolvedValue(sections.map(s => s.id === 2 ? { ...s, title: '新名称', user_edited: true } : s))
    const input = host.querySelector<HTMLInputElement>('#section-name')!
    input.value = '新名称'
    input.dispatchEvent(new Event('input', { bubbles: true }))
    await nextTick()
    input.form!.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }))
    await vi.waitFor(() => expect(DocumentService.editSection).toHaveBeenCalledWith(1, 2, { action: 'rename', title: '新名称' }))
    await vi.waitFor(() => expect(host.querySelector('#section-name')?.getAttribute('disabled')).toBeNull())
    const splitButton = [...host.querySelectorAll<HTMLButtonElement>('nav button')].find(b => b.textContent === '选择拆分位置')!
    splitButton.click()
    await nextTick()
    const splitPoints = [...host.querySelectorAll<HTMLButtonElement>('article button')].filter(b => b.textContent === '从此块拆分')
    expect(splitPoints).toHaveLength(2)
    splitPoints[1].click()
    await vi.waitFor(() => expect(DocumentService.editSection).toHaveBeenLastCalledWith(1, 2, { action: 'split', page: 1, block: 3, title: '新名称（续）' }))
    await nextTick()
    vi.mocked(DocumentService.editSection).mockResolvedValue(sections.map(s => s.id === 2 ? { ...s, locked: true } : s))
    await vi.waitFor(() => expect([...host.querySelectorAll<HTMLButtonElement>('nav button')].find(b => b.textContent === '锁定')?.disabled).toBe(false))
    ;[...host.querySelectorAll<HTMLButtonElement>('nav button')].find(b => b.textContent === '锁定')!.click()
    await vi.waitFor(() => expect(DocumentService.editSection).toHaveBeenLastCalledWith(1, 2, { action: 'lock', locked: true }))
    await vi.waitFor(() => expect(host.textContent).toContain('已锁定'))
    expect([...host.querySelectorAll<HTMLButtonElement>('nav button')].find(b => b.textContent === '重命名')?.disabled).toBe(true)
  })
})
