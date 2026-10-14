import type { RouteLocationRaw } from 'vue-router'
import type { QuestionKind, QuizTrigger } from '@/types/app'
import type { CreateQuizInput, Question, StudyAction } from '@/types/assessment'
import { daysUntil } from './study'

export const QUESTION_KIND_LABELS: Record<QuestionKind, string> = {
  single_choice: '单选题',
  true_false: '判断题',
  short_answer: '简答题',
  recall: '自由复述',
  scenario: '情景应用题'
}

export const QUESTION_KIND_HINTS: Record<QuestionKind, string> = {
  single_choice: '四选一，自动判分',
  true_false: '判断对错，自动判分',
  short_answer: '简要作答，AI 批改并给出反馈',
  recall: '用自己的话复述概念，AI 对照要点批改',
  scenario: '在具体情景中运用知识，AI 批改'
}

export const TRIGGER_LABELS: Record<QuizTrigger, string> = {
  point: '每学完一个知识点后',
  chapter: '每学完一章后',
  manual: '仅手动开始'
}

export const LETTERS = ['A', 'B', 'C', 'D', 'E', 'F']

export function isObjective(kind: QuestionKind) {
  return kind === 'single_choice' || kind === 'true_false'
}

export function isAnswered(question: Question) {
  return question.response !== null || question.skipped
}

/** Colour of a 0..1 score: green when mastered, amber when passing, red when weak. */
export function scoreTone(score: number | null | undefined): 'good' | 'pass' | 'weak' | 'none' {
  if (score === null || score === undefined) return 'none'
  if (score >= 0.8) return 'good'
  if (score >= 0.6) return 'pass'
  return 'weak'
}

export const TONE_TEXT: Record<ReturnType<typeof scoreTone>, string> = {
  good: 'text-emerald-700',
  pass: 'text-amber-700',
  weak: 'text-rose-700',
  none: 'text-slate-400'
}

export const TONE_BAR: Record<ReturnType<typeof scoreTone>, string> = {
  good: 'bg-emerald-500',
  pass: 'bg-amber-400',
  weak: 'bg-rose-500',
  none: 'bg-slate-300'
}

export function percent(score: number | null | undefined): string {
  return score === null || score === undefined ? '—' : `${Math.round(score * 100)}%`
}

/** "今天" / "明天" / "3 天后" / "已过期 2 天". */
export function describeDue(date: string | null | undefined, today = new Date()): string {
  const days = daysUntil(date, today)
  if (days === null) return ''
  if (days < 0) return `已过期 ${-days} 天`
  if (days === 0) return '今天'
  if (days === 1) return '明天'
  return `${days} 天后`
}

/** Readable form of a stored answer. */
export function answerText(question: Pick<Question, 'kind' | 'options'>, value: string | null | undefined): string {
  if (value === null || value === undefined || value === '') return '未作答'
  if (question.kind === 'true_false') return value === 'true' ? '正确' : '错误'
  if (question.kind === 'single_choice') {
    const index = LETTERS.indexOf(value.toUpperCase())
    return index >= 0 && question.options[index] !== undefined ? `${value.toUpperCase()}. ${question.options[index]}` : value
  }
  return value
}

/** What a recommendation does: open a page, or create a quiz first. */
export function actionTarget(action: StudyAction, courseId: number):
  | { type: 'route'; to: RouteLocationRaw }
  | { type: 'quiz'; input: CreateQuizInput } {
  const params = { id: courseId }
  switch (action.kind) {
    case 'review_quiz':
      return { type: 'quiz', input: { course_id: courseId, origin: 'review', knowledge_point_ids: action.knowledge_point_ids } }
    case 'mistakes_quiz':
      return { type: 'quiz', input: { course_id: courseId, origin: 'mistakes' } }
    case 'point_quiz':
      return { type: 'quiz', input: { course_id: courseId, origin: 'manual', knowledge_point_ids: action.knowledge_point_ids } }
    case 'review_session':
      return { type: 'route', to: { name: 'learn', params, query: { mode: 'review', ...(action.section_id ? { section: action.section_id } : {}) } } }
    case 'sprint':
      return { type: 'route', to: { name: 'learn', params, query: { mode: 'sprint', ...(action.section_id ? { section: action.section_id } : {}) } } }
    case 'learn':
      return { type: 'route', to: { name: 'learn', params, query: { mode: 'learn', ...(action.section_id ? { section: action.section_id } : {}) } } }
    case 'path_step':
      return { type: 'route', to: { name: 'paths', params } }
    default:
      return { type: 'route', to: { name: 'knowledge', params } }
  }
}
