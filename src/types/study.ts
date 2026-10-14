// Mirrors src-tauri/src/knowledge, paths, resources, notes, study and tutor/teach.rs

import type { Conversation } from './ai'

/** new = 未学 · learned = 学过 · mastered = 已掌握 (only granted by quiz results) */
export type PointStatus = 'new' | 'learned' | 'mastered'

export interface KnowledgePoint {
  id: number
  course_id: number
  document_id: number | null
  filename: string | null
  section_id: number | null
  section_title: string | null
  title: string
  summary: string
  page_start: number | null
  page_end: number | null
  loc_label: string | null
  /** 1 一般 · 2 重要 · 3 核心考点 */
  importance: number
  /** manual | heading | ai */
  origin: string
  position: number
  status: PointStatus
  learned_at: string | null
  difficult: boolean
  created_at: string
  updated_at: string
  /** Quiz-based mastery 0..1; null until tested. */
  mastery: number | null
  attempts: number
  /** Next spaced-review date (YYYY-MM-DD). */
  due_date: string | null
}

export interface KnowledgeInput {
  id: number | null
  course_id: number
  section_id: number | null
  title: string
  summary: string
  importance: number | null
}

export interface ExtractResult {
  created: KnowledgePoint[]
  skipped: number
  truncated: boolean
}

export type PathMode = 'learn' | 'review' | 'sprint'
export type StepKind = 'read_text' | 'read_slides' | 'video' | 'ai_explain' | 'quiz' | 'recall' | 'review' | 'custom'
export type StepStatus = 'todo' | 'in_progress' | 'done' | 'skipped'

export interface LearningPath {
  id: number
  course_id: number
  name: string
  description: string
  mode: PathMode
  /** manual | template | ai */
  origin: string
  archived: boolean
  created_at: string
  updated_at: string
  step_count: number
  done_count: number
  skipped_count: number
}

export interface PathStep {
  id: number
  path_id: number
  position: number
  kind: StepKind
  kind_label: string
  title: string
  instructions: string
  document_id: number | null
  document_name: string | null
  section_id: number | null
  section_title: string | null
  page_start: number | null
  resource_id: number | null
  resource_title: string | null
  knowledge_point_ids: number[]
  estimated_minutes: number | null
  status: StepStatus
  started_at: string | null
  completed_at: string | null
  note: string
}

export interface PathDetail {
  path: LearningPath
  steps: PathStep[]
}

export interface PathInput {
  id: number | null
  course_id: number
  name: string
  description: string
  mode: PathMode
  archived?: boolean
}

export interface StepInput {
  id: number | null
  path_id: number
  kind: StepKind
  title: string
  instructions: string
  document_id: number | null
  section_id: number | null
  resource_id: number | null
  knowledge_point_ids: number[]
  estimated_minutes: number | null
  after_step_id?: number | null
}

export interface TemplateInput {
  course_id: number
  name?: string | null
  mode: PathMode
  document_ids: number[]
  include_quiz: boolean
}

export interface RecommendInput {
  course_id: number
  mode: PathMode
  preferences: string
}

export interface ResourceMark {
  id: number
  resource_id: number
  seconds: number
  label: string
  note: string
  knowledge_point_id: number | null
}

export interface Resource {
  id: number
  course_id: number
  /** video | link */
  kind: string
  title: string
  url: string
  section_id: number | null
  section_title: string | null
  transcript: string
  notes: string
  marks: ResourceMark[]
  created_at: string
  updated_at: string
}

export interface ResourceInput {
  id: number | null
  course_id: number
  kind: string
  title: string
  url: string
  section_id: number | null
  transcript: string
  notes: string
}

export interface MarkInput {
  id: number | null
  resource_id: number
  seconds: number
  label: string
  note: string
  knowledge_point_id: number | null
}

export interface Note {
  id: number
  course_id: number
  title: string
  content: string
  document_id: number | null
  document_name: string | null
  page_number: number | null
  page_reference: string | null
  section_id: number | null
  section_title: string | null
  knowledge_point_id: number | null
  knowledge_point_title: string | null
  created_at: string
  updated_at: string
}

export interface NoteInput {
  id: number | null
  course_id: number
  title: string
  content: string
  document_id: number | null
  page_number: number | null
  section_id: number | null
  knowledge_point_id: number | null
}

export interface StudyCounts {
  documents: number
  sections: number
  points: number
  points_learned: number
  points_mastered: number
  points_difficult: number
  steps: number
  steps_done: number
}

export interface DayActivity {
  date: string
  events: number
}

export interface StudyState {
  course_id: number
  active_path: LearningPath | null
  current_step: PathStep | null
  current_section_id: number | null
  current_section_title: string | null
  last_document_id: number | null
  last_document_name: string | null
  last_page: number | null
  last_conversation: Conversation | null
  counts: StudyCounts
  activity: DayActivity[]
  updated_at: string | null
}

export type TeachAction = 'start' | 'continue' | 'skip' | 'goto' | 'example' | 'rephrase' | 'summary' | 'ask'

export interface StartTeachingInput {
  course_id: number
  mode: PathMode
  section_id?: number | null
  resource_id?: number | null
  step_id?: number | null
}

export interface LessonPart {
  index: number
  title: string
  loc_label: string
  chars: number
  status: 'taught' | 'skipped' | 'next' | 'todo'
}

export interface CourseDigest {
  course_id: number
  name: string
  exam_date: string | null
  days_to_exam: number | null
  points: number
  learned: number
  mastered: number
  due: number
  weak: number
  open_mistakes: number
  next_step: string | null
  updated_at: string
}
