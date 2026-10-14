import { describe, expect, it } from 'vitest'
import { daysUntil, examCountdown, formatSeconds, parseTimestamp, stepRoute, urlAt } from '../study'
import type { PathStep } from '@/types/study'

describe('timestamps', () => {
  it('parses mm:ss, h:mm:ss, full-width colons and plain seconds', () => {
    expect(parseTimestamp('12:34')).toBe(754)
    expect(parseTimestamp('1:02:05')).toBe(3725)
    expect(parseTimestamp('3：05')).toBe(185)
    expect(parseTimestamp('90')).toBe(90)
  })

  it('rejects malformed values', () => {
    for (const bad of ['', '1:2:3:4', '12:75', 'ab:cd', '1:']) expect(parseTimestamp(bad)).toBeNull()
  })

  it('links to a timestamp on sites that support it', () => {
    expect(urlAt('https://www.bilibili.com/video/BV1xx?p=2', 754)).toBe('https://www.bilibili.com/video/BV1xx?p=2&t=754')
    expect(urlAt('https://youtu.be/abc', 90)).toBe('https://youtu.be/abc?t=90s')
    expect(urlAt('https://example.com/v', 90)).toBe('https://example.com/v')
    expect(urlAt('not a url', 90)).toBe('not a url')
  })

  it('formats seconds like the video players do', () => {
    expect(formatSeconds(754)).toBe('12:34')
    expect(formatSeconds(3725)).toBe('1:02:05')
    expect(formatSeconds(5)).toBe('00:05')
  })
})

describe('exam countdown', () => {
  const today = new Date(2026, 9, 14, 23, 30)
  it('counts calendar days regardless of the time of day', () => {
    expect(daysUntil('2026-10-20', today)).toBe(6)
    expect(daysUntil('2026-10-14', today)).toBe(0)
    expect(daysUntil('2026-10-01', today)).toBe(-13)
    expect(daysUntil('not a date', today)).toBeNull()
  })

  it('describes the countdown', () => {
    expect(examCountdown('2026-10-20', today)).toBe('距离考试还有 6 天')
    expect(examCountdown('2026-10-14', today)).toBe('今天考试')
    expect(examCountdown(null, today)).toBe('')
  })
})

describe('step routes', () => {
  const step = (patch: Partial<PathStep>): PathStep => ({
    id: 7, path_id: 1, position: 0, kind: 'custom', kind_label: '', title: '', instructions: '', document_id: null, document_name: null,
    section_id: null, section_title: null, page_start: null, resource_id: null, resource_title: null, knowledge_point_ids: [],
    estimated_minutes: null, status: 'todo', started_at: null, completed_at: null, note: '', ...patch
  })

  it('opens the right page for each kind of step', () => {
    expect(stepRoute(step({ kind: 'read_text', document_id: 3, page_start: 12 }), 1, 'learn'))
      .toEqual({ name: 'documents', params: { id: 1 }, query: { doc: 3, page: 12 } })
    expect(stepRoute(step({ kind: 'ai_explain', section_id: 5 }), 1, 'sprint'))
      .toEqual({ name: 'learn', params: { id: 1 }, query: { mode: 'sprint', section: 5, step: 7 } })
    expect(stepRoute(step({ kind: 'recall', section_id: 5 }), 1, 'learn'))
      .toEqual({ name: 'learn', params: { id: 1 }, query: { mode: 'review', section: 5, step: 7 } })
    expect(stepRoute(step({ kind: 'video', resource_id: 9 }), 1, 'learn')).toEqual({ name: 'videos', params: { id: 1 }, query: { r: 9 } })
    expect(stepRoute(step({ kind: 'custom' }), 1, 'learn')).toBeNull()
    expect(stepRoute(step({ kind: 'read_text' }), 1, 'learn')).toBeNull()
  })
})
