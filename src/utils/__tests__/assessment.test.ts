import { describe, expect, it } from 'vitest'
import { actionTarget, answerText, describeDue, isAnswered, percent, scoreTone } from '../assessment'
import type { Question, StudyAction } from '@/types/assessment'

const question = (patch: Partial<Question>): Question => ({
  id: 1, quiz_id: 1, position: 0, kind: 'single_choice', kind_label: '单选题', knowledge_point_id: 1, knowledge_point_title: '导数',
  prompt: '?', options: ['切线斜率', '面积'], response: null, skipped: false, answer: null, explanation: null, rubric: [], sources: [],
  score: null, grading: '', feedback: '', ...patch
})

describe('assessment helpers', () => {
  it('describes answers for every question type', () => {
    expect(answerText(question({}), 'a')).toBe('A. 切线斜率')
    expect(answerText(question({ kind: 'true_false' }), 'false')).toBe('错误')
    expect(answerText(question({ kind: 'short_answer' }), '斜率')).toBe('斜率')
    expect(answerText(question({}), null)).toBe('未作答')
  })

  it('treats skipped questions as answered for navigation', () => {
    expect(isAnswered(question({}))).toBe(false)
    expect(isAnswered(question({ skipped: true }))).toBe(true)
    expect(isAnswered(question({ response: 'B' }))).toBe(true)
  })

  it('grades scores into tones and percentages', () => {
    expect([scoreTone(0.9), scoreTone(0.6), scoreTone(0.2), scoreTone(null)]).toEqual(['good', 'pass', 'weak', 'none'])
    expect(percent(0.456)).toBe('46%')
    expect(percent(null)).toBe('—')
  })

  it('describes review dates relative to today', () => {
    const today = new Date(2026, 9, 14)
    expect(describeDue('2026-10-14', today)).toBe('今天')
    expect(describeDue('2026-10-15', today)).toBe('明天')
    expect(describeDue('2026-10-20', today)).toBe('6 天后')
    expect(describeDue('2026-10-12', today)).toBe('已过期 2 天')
  })

  it('turns recommendations into quizzes or pages', () => {
    const action = (kind: StudyAction['kind'], patch: Partial<StudyAction> = {}): StudyAction =>
      ({ kind, title: '', detail: '', knowledge_point_ids: [3, 4], section_id: 9, step_id: null, ...patch })
    expect(actionTarget(action('review_quiz'), 1)).toEqual({ type: 'quiz', input: { course_id: 1, origin: 'review', knowledge_point_ids: [3, 4] } })
    expect(actionTarget(action('mistakes_quiz'), 1)).toEqual({ type: 'quiz', input: { course_id: 1, origin: 'mistakes' } })
    expect(actionTarget(action('review_session'), 1)).toEqual({ type: 'route', to: { name: 'learn', params: { id: 1 }, query: { mode: 'review', section: 9 } } })
    expect(actionTarget(action('sprint', { section_id: null }), 1)).toEqual({ type: 'route', to: { name: 'learn', params: { id: 1 }, query: { mode: 'sprint' } } })
  })
})
