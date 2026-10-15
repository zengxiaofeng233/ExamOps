import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { effectScope } from 'vue'
import { DocumentService } from '@/services/document'
import { useDocumentTasksStore } from '../documentTasks'
import { useToastStore } from '../toast'
import type { Document } from '@/types/document'

vi.mock('@/services/document', () => ({ DocumentService: { import: vi.fn(), reprocess: vi.fn() } }))

function document(id: number): Document {
  return {
    id, course_id: 1, filename: `${id}.pdf`, file_type: 'pdf', file_path: '', file_size: 1,
    status: 'completed', created_at: '', page_count: 2, loc_kind: 'page', char_count: 10,
    structure_source: 'none', warnings: [], indexed_at: null, section_count: 1, image_count: 0
  }
}

beforeEach(() => {
  setActivePinia(createPinia())
  vi.resetAllMocks()
  vi.useFakeTimers()
})

afterEach(() => {
  vi.clearAllTimers()
  vi.useRealTimers()
})

describe('background document tasks', () => {
  it('keeps progress and the sequential queue after the originating view is disposed', async () => {
    let finish!: (value: Document) => void
    vi.mocked(DocumentService.import)
      .mockReturnValueOnce(new Promise(resolve => { finish = resolve }))
      .mockResolvedValueOnce(document(2))
    const view = effectScope()
    const store = view.run(() => useDocumentTasksStore())!
    const pending = store.importFiles(1, ['first.pdf', 'second.pdf'])
    expect(DocumentService.import).toHaveBeenCalledTimes(1)
    view.stop()

    const returned = useDocumentTasksStore()
    const progress = { document_id: 1, stage: 'page', current: 1, total: 2 }
    vi.mocked(DocumentService.import).mock.calls[0][2]!(progress)
    expect(returned.forCourse(1).importing).toBe(true)
    expect(returned.progress[1]).toEqual(progress)
    expect(returned.forCourse(2).importing).toBe(false)
    await returned.importFiles(1, ['duplicate.pdf'])
    expect(DocumentService.import).toHaveBeenCalledTimes(1)

    finish(document(1))
    await pending
    expect(DocumentService.import).toHaveBeenNthCalledWith(2, 1, 'second.pdf', expect.any(Function))
    expect(returned.forCourse(1)).toMatchObject({ importing: false, importStatus: '', lastImportedId: 2, revision: 2 })
    expect(returned.forCourse(2).revision).toBe(0)
    expect(useToastStore().toasts.slice(-1)[0]?.text).toBe('已导入 2 份资料')
  })

  it('continues after a failed import and clears reprocessing state after an error', async () => {
    vi.mocked(DocumentService.import).mockRejectedValueOnce('无法读取文件').mockResolvedValueOnce(document(2))
    const store = useDocumentTasksStore()
    await store.importFiles(1, ['bad.pdf', 'good.pdf'])
    expect(DocumentService.import).toHaveBeenCalledTimes(2)
    expect(store.forCourse(1).importing).toBe(false)
    expect(useToastStore().toasts.slice(-1)[0]?.text).toContain('1 个成功，1 个失败')

    vi.mocked(DocumentService.reprocess).mockRejectedValueOnce('解析失败')
    await store.reprocess(1, 2)
    expect(store.forCourse(1).reprocessing).toBe(false)
    expect(store.forCourse(1).revision).toBe(3)
    expect(useToastStore().toasts.slice(-1)[0]?.text).toBe('解析失败')
  })
})
