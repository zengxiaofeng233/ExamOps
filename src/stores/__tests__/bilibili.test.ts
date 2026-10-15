import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { effectScope } from 'vue'
import { BilibiliService } from '@/services/bilibili'
import { isBilibiliVideo, useBilibiliStore } from '../bilibili'
import type { BilibiliText } from '@/types/bilibili'

vi.mock('@/services/bilibili', () => ({ BilibiliService: { fetchText: vi.fn() } }))
const url = 'https://www.bilibili.com/video/BV1L94y1H7CV?p=2'
const result: BilibiliText = { title: '高数 P2', url, kind: 'summary', transcript: '【B站 AI 总结（非完整字幕）】\n导数', notice: '只有摘要' }

beforeEach(() => { setActivePinia(createPinia()); vi.resetAllMocks() })

describe('Bilibili text tasks', () => {
  it('recognizes video and short links without accepting lookalike hosts', () => {
    expect(isBilibiliVideo(url)).toBe(true)
    expect(isBilibiliVideo('https://b23.tv/abc')).toBe(true)
    expect(isBilibiliVideo('https://www.bilibili.com.evil.test/video/BV1L94y1H7CV')).toBe(false)
    expect(isBilibiliVideo('https://www.bilibili.com/')).toBe(false)
  })

  it('shares in-flight work and keeps the result when the originating page is gone', async () => {
    let finish!: (value: BilibiliText) => void
    vi.mocked(BilibiliService.fetchText).mockReturnValueOnce(new Promise(resolve => { finish = resolve }))
    const view = effectScope()
    const store = view.run(() => useBilibiliStore())!
    const first = store.fetchText(url)
    view.stop()
    const next = useBilibiliStore()
    const second = next.fetchText(url)
    expect(BilibiliService.fetchText).toHaveBeenCalledTimes(1)
    finish(result)
    expect(await first).toEqual(result)
    expect(await second).toEqual(result)
    expect(next.jobs[url]).toEqual({ pending: false, error: '', result })
  })

  it('does not repeatedly retry a failure until explicitly requested (e.g. after login)', async () => {
    vi.mocked(BilibiliService.fetchText).mockRejectedValueOnce('需要登录').mockResolvedValueOnce(result)
    const store = useBilibiliStore()
    expect(await store.fetchText(url)).toBeNull()
    await store.fetchText(url)
    expect(BilibiliService.fetchText).toHaveBeenCalledTimes(1)
    expect(store.jobs[url].error).toBe('需要登录')
    expect(await store.fetchText(url, true)).toEqual(result)
    expect(store.jobs[url].error).toBe('')
  })

  it('retries with the new login even if an anonymous request was still running', async () => {
    let fail!: (error: string) => void
    vi.mocked(BilibiliService.fetchText)
      .mockReturnValueOnce(new Promise((_, reject) => { fail = reject }))
      .mockResolvedValueOnce(result)
    const store = useBilibiliStore()
    const anonymous = store.fetchText(url)
    const afterLogin = store.fetchText(url, true)
    fail('需要登录')
    expect(await anonymous).toBeNull()
    expect(await afterLogin).toEqual(result)
    expect(BilibiliService.fetchText).toHaveBeenCalledTimes(2)
  })
})
