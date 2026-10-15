import { defineStore } from 'pinia'
import { reactive } from 'vue'
import { BilibiliService } from '@/services/bilibili'
import type { BilibiliText } from '@/types/bilibili'
import { errorMessage } from '@/utils/errors'

interface TextJob { pending: boolean; error: string; result: BilibiliText | null }

export function isBilibiliVideo(input: string) {
  try {
    const url = new URL(input.trim())
    if (!['http:', 'https:'].includes(url.protocol) || url.username || url.password || url.port) return false
    if (url.hostname === 'b23.tv') return url.pathname.length > 1
    return ['www.bilibili.com', 'bilibili.com', 'm.bilibili.com'].includes(url.hostname)
      && /^\/video\/(BV[a-zA-Z0-9]{10}|av\d+)(\/|$)/.test(url.pathname)
  } catch { return false }
}

// Keep in-flight results when a user leaves the page; never write to a resource implicitly.
export const useBilibiliStore = defineStore('bilibili', () => {
  const jobs = reactive<Record<string, TextJob>>({})
  const running = new Map<string, Promise<BilibiliText | null>>()

  function fetchText(input: string, retry = false): Promise<BilibiliText | null> {
    const url = input.trim()
    const pending = running.get(url)
    // A login may finish while an anonymous request is still running.
    if (pending) return retry ? pending.then(() => fetchText(url, true)) : pending
    if (!retry && jobs[url]) return Promise.resolve(jobs[url].result)
    const job = jobs[url] = reactive<TextJob>({ pending: true, error: '', result: null })
    const task = BilibiliService.fetchText(url).then(result => {
      job.result = result
      return result
    }).catch(error => {
      job.error = errorMessage(error)
      return null
    }).finally(() => {
      job.pending = false
      running.delete(url)
    })
    running.set(url, task)
    return task
  }

  return { jobs, fetchText }
})
