import { invoke } from '@tauri-apps/api/core'
import type {
  Advice, AnswerInput, CreateQuizInput, Mistake, Question, Quiz, QuizDetail, ReviewQueue, StudyAction
} from '@/types/assessment'

export const QuizService = {
  /** The AI writes the questions (takes a few seconds to a minute). */
  create: (input: CreateQuizInput) => invoke<QuizDetail>('create_quiz', { input }),
  list: (courseId: number) => invoke<Quiz[]>('list_quizzes', { courseId }),
  get: (id: number) => invoke<QuizDetail>('get_quiz', { id }),
  remove: (id: number) => invoke<void>('delete_quiz', { id }),
  answer: (input: AnswerInput) => invoke<Question>('save_quiz_answer', { input }),
  setPosition: (quizId: number, index: number) => invoke<void>('set_quiz_position', { quizId, index }),
  submit: (id: number) => invoke<QuizDetail>('submit_quiz', { id }),
  regrade: (id: number) => invoke<QuizDetail>('regrade_quiz', { id }),
  selfGrade: (questionId: number, score: number) => invoke<QuizDetail>('self_grade_question', { questionId, score })
}

export const ReviewService = {
  queue: (courseId: number) => invoke<ReviewQueue>('get_review_queue', { courseId }),
  mistakes: (courseId: number, includeResolved = false) => invoke<Mistake[]>('list_mistakes', { courseId, includeResolved }),
  resolveMistake: (id: number, resolved: boolean) => invoke<void>('resolve_mistake', { id, resolved }),
  recommendations: (courseId: number) => invoke<StudyAction[]>('get_recommendations', { courseId }),
  advice: (courseId: number) => invoke<Advice>('get_ai_advice', { courseId })
}
