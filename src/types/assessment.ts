// Mirrors src-tauri/src/assessment, mastery and review

import type { SourceRef } from './ai'
import type { QuestionKind } from './app'
import type { KnowledgePoint } from './study'

export type QuizOrigin = 'manual' | 'point' | 'chapter' | 'review' | 'mistakes' | 'step'
/** ready = answering · grading = submitted, written answers waiting for a score · graded */
export type QuizStatus = 'ready' | 'grading' | 'graded'

export interface Quiz {
  id: number
  course_id: number
  title: string
  origin: QuizOrigin
  section_id: number | null
  section_title: string | null
  step_id: number | null
  status: QuizStatus
  allow_skip: boolean
  /** Percentage once graded. */
  score: number | null
  pending: number
  current_index: number
  question_count: number
  answered_count: number
  grading_error: string | null
  model: string | null
  created_at: string
  submitted_at: string | null
  graded_at: string | null
}

export interface Question {
  id: number
  quiz_id: number
  position: number
  kind: QuestionKind
  kind_label: string
  knowledge_point_id: number | null
  knowledge_point_title: string | null
  prompt: string
  options: string[]
  /** Option letter, "true"/"false" or free text. */
  response: string | null
  skipped: boolean
  /** Revealed after submission. */
  answer: string | null
  explanation: string | null
  rubric: string[]
  sources: SourceRef[]
  /** 0..1 once graded. */
  score: number | null
  /** auto | ai | self | pending | '' */
  grading: string
  feedback: string
}

export interface PointOutcome {
  knowledge_point_id: number
  title: string
  result: number
  before: number | null
  after: number
  status: string
  due_date: string | null
}

export interface QuizDetail {
  quiz: Quiz
  questions: Question[]
  sources: SourceRef[]
  outcome: PointOutcome[]
}

export interface CreateQuizInput {
  course_id: number
  origin?: QuizOrigin
  section_id?: number | null
  knowledge_point_ids?: number[]
  step_id?: number | null
  question_count?: number
  kinds?: QuestionKind[]
  allow_skip?: boolean
}

export interface AnswerInput {
  question_id: number
  response: string | null
  skipped: boolean
}

export interface Mistake {
  id: number
  course_id: number
  question_id: number
  quiz_id: number
  quiz_title: string
  knowledge_point_id: number | null
  knowledge_point_title: string | null
  kind: QuestionKind
  kind_label: string
  prompt: string
  options: string[]
  response: string | null
  answer: string
  explanation: string
  feedback: string
  score: number | null
  sources: SourceRef[]
  resolved: boolean
  created_at: string
}

export interface Mastery {
  knowledge_point_id: number
  score: number
  attempts: number
  correct: number
  streak: number
  ease: number
  interval_days: number
  due_date: string | null
  last_result: number | null
  last_quiz_at: string | null
}

export interface ReviewItem {
  point: KnowledgePoint
  mastery: Mastery | null
  /** due | weak | difficult | untested */
  reason: 'due' | 'weak' | 'difficult' | 'untested'
  reason_label: string
  priority: number
}

export interface ReviewSummary {
  due: number
  overdue: number
  weak: number
  untested: number
  open_mistakes: number
  mastered: number
  tested: number
  average: number | null
  next_due: string | null
  exam_date: string | null
  days_to_exam: number | null
}

export interface ReviewQueue {
  summary: ReviewSummary
  items: ReviewItem[]
}

export type ActionKind = 'review_quiz' | 'mistakes_quiz' | 'point_quiz' | 'review_session' | 'path_step' | 'sprint' | 'learn' | 'extract_points'

export interface StudyAction {
  kind: ActionKind
  title: string
  detail: string
  knowledge_point_ids: number[]
  section_id: number | null
  step_id: number | null
}

export interface Advice {
  text: string
  focus: { id: number; title: string }[]
  model: string
}
