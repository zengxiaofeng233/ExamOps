// Mirrors src-tauri/src/settings.rs and src-tauri/src/commands/app.rs

export interface ContextSettings {
  mode: 'auto' | 'full' | 'rag'
  rag_top_k: number
  full_ratio: number
  history_turns: number
}

/** When the tutor suggests a quiz: after each knowledge point, after each chapter, or only manually. */
export type QuizTrigger = 'point' | 'chapter' | 'manual'

export type QuestionKind = 'single_choice' | 'true_false' | 'short_answer' | 'recall' | 'scenario'

export interface QuizSettings {
  trigger: QuizTrigger
  question_count: number
  allow_skip: boolean
  kinds: QuestionKind[]
}

export interface AppSettings {
  restore_last_location: boolean
  context: ContextSettings
  quiz: QuizSettings
}

export interface AppInfo {
  version: string
  data_dir: string
  database_bytes: number
  documents_bytes: number
  schema_version: number
  platform: string
}

export interface LastLocation {
  path: string
  course_id: number | null
  saved_at?: string
}
