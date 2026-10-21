import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { AppService } from '@/services/app'
import { useUpdateStore } from '../updates'
import { useToastStore } from '../toast'
import type { AvailableUpdate } from '@/types/app'

vi.mock('@/services/app', () => ({ AppService: { checkUpdates: vi.fn(), openExternal: vi.fn(), downloadUpdate: vi.fn(), installUpdate: vi.fn() } }))

const update: AvailableUpdate = {
  current_version: '0.1.0', version: '0.2.0',
  release_url: 'https://github.com/zengxiaofeng233/ExamOps/releases/tag/v0.2.0',
  asset_name: 'ExamOps_0.2.0_x64-setup.exe'
}

beforeEach(() => {
  setActivePinia(createPinia())
  vi.resetAllMocks()
})

describe('update discovery', () => {
  it('downloads once, preserves the selected version, and requires a separate install action', async () => {
    vi.mocked(AppService.checkUpdates).mockResolvedValue(update)
    let done!: () => void
    vi.mocked(AppService.downloadUpdate).mockImplementation((_version, progress) => {
      progress({ downloaded: 1048576, total: 2097152 })
      return new Promise(resolve => { done = resolve })
    })
    const store = useUpdateStore()
    await store.check()
    await store.install()
    expect(AppService.installUpdate).not.toHaveBeenCalled()
    const download = store.download()
    await store.download()
    await store.check()
    expect(AppService.downloadUpdate).toHaveBeenCalledTimes(1)
    expect(AppService.checkUpdates).toHaveBeenCalledTimes(1)
    expect(store.progressText).toBe('50% · 1.0 / 2.0 MB')
    store.dismiss()
    done()
    await download
    expect(store.phase).toBe('ready')
    expect(AppService.installUpdate).not.toHaveBeenCalled()
    await store.install()
    await store.install()
    expect(AppService.installUpdate).toHaveBeenCalledTimes(1)
  })

  it('never enables installation after a download or signature failure and allows retry', async () => {
    vi.mocked(AppService.checkUpdates).mockResolvedValue(update)
    vi.mocked(AppService.downloadUpdate).mockRejectedValueOnce('签名校验失败').mockResolvedValueOnce(undefined)
    const store = useUpdateStore()
    await store.check()
    await store.download()
    expect(store.phase).toBe('idle')
    expect(store.downloadError).toBe('签名校验失败')
    await store.install()
    expect(AppService.installUpdate).not.toHaveBeenCalled()
    await store.download()
    expect(store.phase).toBe('ready')
    expect(store.downloadError).toBe('')
  })

  it('retains the verified download when installation is blocked or fails', async () => {
    vi.mocked(AppService.checkUpdates).mockResolvedValue(update)
    vi.mocked(AppService.downloadUpdate).mockResolvedValue(undefined)
    vi.mocked(AppService.installUpdate).mockRejectedValueOnce('资料正在处理').mockResolvedValueOnce(undefined)
    const store = useUpdateStore()
    await store.check()
    await store.download()
    await store.install()
    expect(store.phase).toBe('ready')
    expect(store.downloadError).toBe('资料正在处理')
    await store.install()
    expect(AppService.downloadUpdate).toHaveBeenCalledTimes(1)
    expect(AppService.installUpdate).toHaveBeenCalledTimes(2)
  })

  it('handles unknown content length without showing an invalid percentage', async () => {
    vi.mocked(AppService.checkUpdates).mockResolvedValue(update)
    vi.mocked(AppService.downloadUpdate).mockImplementation(async (_version, progress) => {
      progress({ downloaded: 1048576, total: null })
    })
    const store = useUpdateStore()
    await store.check()
    await store.download()
    expect(store.progressText).toBe('1.0 MB')
  })

  it('shares an in-flight check and never opens the release automatically', async () => {
    let resolve!: (value: AvailableUpdate) => void
    vi.mocked(AppService.checkUpdates).mockReturnValue(new Promise(done => { resolve = done }))
    const store = useUpdateStore()
    const first = store.check()
    const second = store.check()
    expect(store.checking).toBe(true)
    expect(AppService.checkUpdates).toHaveBeenCalledTimes(1)
    resolve(update)
    await Promise.all([first, second])
    expect(store.available).toEqual(update)
    expect(store.checked).toBe(true)
    expect(store.checking).toBe(false)
    expect(AppService.openExternal).not.toHaveBeenCalled()
  })

  it('keeps the available release after dismissal and opens it only on request', async () => {
    vi.mocked(AppService.checkUpdates).mockResolvedValue(update)
    vi.mocked(AppService.openExternal).mockResolvedValue(undefined)
    const store = useUpdateStore()
    await store.check()
    store.dismiss()
    expect(store.dismissed).toBe(true)
    expect(store.available).toEqual(update)
    await store.openRelease()
    expect(AppService.openExternal).toHaveBeenCalledExactlyOnceWith(update.release_url)
    await store.check()
    expect(store.dismissed).toBe(false)
  })

  it('records a quiet failure and can retry successfully without an update', async () => {
    vi.mocked(AppService.checkUpdates).mockRejectedValueOnce('检查更新超时').mockResolvedValueOnce(null)
    const store = useUpdateStore()
    await store.check()
    expect(store.error).toBe('检查更新超时')
    expect(store.checking).toBe(false)
    expect(useToastStore().toasts).toHaveLength(0)
    await store.check()
    expect(store.error).toBe('')
    expect(store.available).toBeNull()
    expect(store.checked).toBe(true)
  })

  it('reports a failed browser launch only after the user requests it', async () => {
    vi.mocked(AppService.checkUpdates).mockResolvedValue(update)
    vi.mocked(AppService.openExternal).mockRejectedValue('无法打开链接')
    const store = useUpdateStore()
    await store.check()
    await store.openRelease()
    expect(useToastStore().toasts[0].text).toBe('无法打开链接')
  })
})
