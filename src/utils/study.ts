import type { RouteLocationRaw } from 'vue-router'
import type { TutorMode } from '@/types/ai'
import type { PathMode, PathStep, PointStatus, StepKind, StepStatus } from '@/types/study'

export const POINT_STATUS_LABELS: Record<PointStatus, string> = { new: '未学', learned: '学过', mastered: '已掌握' }

export const POINT_STATUS_CLASSES: Record<PointStatus, string> = {
  new: 'bg-slate-100 text-slate-600',
  learned: 'bg-sky-50 text-sky-700',
  mastered: 'bg-emerald-50 text-emerald-700'
}

export const IMPORTANCE_LABELS: Record<number, string> = { 1: '一般', 2: '重要', 3: '核心考点' }

export const MODE_LABELS: Record<TutorMode, string> = { qa: 'AI 问答', learn: '从零学习', review: '正常复习', sprint: '考前冲刺' }

export const MODE_DESCRIPTIONS: Record<PathMode, string> = {
  learn: 'AI 按章节分段讲解，每段讲完停下来等你，可以追问、要例子、换种讲法或跳过。',
  review: 'AI 先出回忆题让你凭记忆作答，再逐题纠正和补充，适合学过之后巩固。',
  sprint: '临近考试时用：精炼列出必考点、必背公式和典型题思路，优先补薄弱知识点。'
}

export const MODE_CLASSES: Record<TutorMode, string> = {
  qa: 'bg-slate-100 text-slate-600',
  learn: 'bg-indigo-50 text-indigo-700',
  review: 'bg-amber-50 text-amber-700',
  sprint: 'bg-rose-50 text-rose-700'
}

export const STEP_KIND_LABELS: Record<StepKind, string> = {
  read_text: '阅读教材',
  read_slides: '阅读 PPT',
  video: '观看网课',
  ai_explain: 'AI 讲解',
  quiz: '知识点小测',
  recall: '主动回忆',
  review: '复习旧知识',
  custom: '自定义任务'
}

export const STEP_STATUS_LABELS: Record<StepStatus, string> = { todo: '待完成', in_progress: '进行中', done: '已完成', skipped: '已跳过' }

export const STEP_STATUS_CLASSES: Record<StepStatus, string> = {
  todo: 'bg-slate-100 text-slate-600',
  in_progress: 'bg-indigo-50 text-indigo-700',
  done: 'bg-emerald-50 text-emerald-700',
  skipped: 'bg-slate-100 text-slate-400 line-through'
}

/** Kinds that need a chapter / video to be useful. */
export const STEP_NEEDS: Record<StepKind, 'section' | 'document' | 'resource' | null> = {
  read_text: 'document',
  read_slides: 'document',
  video: 'resource',
  ai_explain: 'section',
  quiz: null,
  recall: 'section',
  review: null,
  custom: null
}

/** "12:34", "1:02:05" or plain seconds → seconds; null when invalid. */
export function parseTimestamp(value: string): number | null {
  const text = value.trim().replace('：', ':')
  if (!text) return null
  if (/^\d+$/.test(text)) return Number(text)
  const parts = text.split(':')
  if (parts.length < 2 || parts.length > 3 || parts.some(p => !/^\d{1,2}$/.test(p))) return null
  const numbers = parts.map(Number)
  if (numbers.slice(1).some(n => n >= 60)) return null
  return numbers.reduce((total, n) => total * 60 + n, 0)
}

export function formatSeconds(seconds: number): string {
  const h = Math.floor(seconds / 3600)
  const m = Math.floor((seconds % 3600) / 60)
  const s = seconds % 60
  const pad = (n: number) => String(n).padStart(2, '0')
  return h > 0 ? `${h}:${pad(m)}:${pad(s)}` : `${pad(m)}:${pad(s)}`
}

/** Video link that starts at a timestamp on sites that support it (Bilibili, YouTube). */
export function urlAt(url: string, seconds: number): string {
  try {
    const parsed = new URL(url)
    if (/(^|\.)bilibili\.com$/.test(parsed.hostname)) parsed.searchParams.set('t', String(seconds))
    else if (/(^|\.)(youtube\.com|youtu\.be)$/.test(parsed.hostname)) parsed.searchParams.set('t', `${seconds}s`)
    else return url
    return parsed.toString()
  } catch {
    return url
  }
}

/** Whole days from `today` until a YYYY-MM-DD date (negative when past). */
export function daysUntil(date: string | null | undefined, today = new Date()): number | null {
  if (!date || !/^\d{4}-\d{2}-\d{2}$/.test(date)) return null
  const [y, m, d] = date.split('-').map(Number)
  const target = Date.UTC(y, m - 1, d)
  const base = Date.UTC(today.getFullYear(), today.getMonth(), today.getDate())
  return Math.round((target - base) / 86400000)
}

export function examCountdown(date: string | null | undefined, today = new Date()): string {
  const days = daysUntil(date, today)
  if (days === null) return ''
  if (days > 0) return `距离考试还有 ${days} 天`
  if (days === 0) return '今天考试'
  return '考试日期已过'
}

/** Where "开始" on a learning step takes the student; null = done by hand (custom tasks). */
export function stepRoute(step: PathStep, courseId: number, pathMode: PathMode): RouteLocationRaw | null {
  const params = { id: courseId }
  switch (step.kind) {
    case 'read_text':
    case 'read_slides':
      return step.document_id
        ? { name: 'documents', params, query: { doc: step.document_id, ...(step.page_start ? { page: step.page_start } : {}) } }
        : null
    case 'video':
      return step.resource_id ? { name: 'videos', params, query: { r: step.resource_id } } : null
    case 'ai_explain':
      return { name: 'learn', params, query: { mode: pathMode, ...(step.section_id ? { section: step.section_id } : {}), step: step.id } }
    case 'recall':
    case 'review':
      return { name: 'learn', params, query: { mode: 'review', ...(step.section_id ? { section: step.section_id } : {}), step: step.id } }
    case 'quiz':
      return { name: 'quiz', params, query: { step: step.id, ...(step.section_id ? { section: step.section_id } : {}) } }
    default:
      return null
  }
}
