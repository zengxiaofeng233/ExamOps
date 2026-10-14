// Mirrors src-tauri/src/ai/*, context/mod.rs and tutor/*

export interface ModelProfile {
  id: number
  name: string
  base_url: string
  model: string
  temperature: number
  top_p: number | null
  max_output_tokens: number
  context_window: number
  timeout_secs: number
  json_mode: boolean
  is_default: boolean
  has_key: boolean
  /** keyring | local | none */
  key_storage: string
  key_hint: string | null
  created_at: string
  updated_at: string
}

export interface ProfileInput {
  id: number | null
  name: string
  base_url: string
  model: string
  temperature: number
  top_p: number | null
  max_output_tokens: number
  context_window: number
  timeout_secs: number
  json_mode: boolean
  /** null keeps the stored key, '' removes it */
  api_key: string | null
}

export interface SaveProfileResult {
  profile: ModelProfile
  warning: string | null
}

export interface TestReport {
  ok: boolean
  latency_ms: number
  reply: string
  model: string
}

export interface UsageSummary {
  today_prompt: number
  today_completion: number
  total_prompt: number
  total_completion: number
  requests: number
}

export type ContextMode = 'auto' | 'full' | 'rag'
export type TutorMode = 'qa' | 'learn' | 'review' | 'sprint'

export interface ConversationScope {
  document_ids: number[]
  section_ids: number[]
}

export interface SourceRef {
  label: string
  chunk_id: number
  document_id: number
  filename: string
  section_id: number | null
  section_title: string | null
  page_start: number
  page_end: number
  loc_label: string
  excerpt: string
  cited: boolean
}

/** 从零学习 progress over the lesson parts of a chapter or video transcript. */
export interface TeachProgress {
  /** Index of the next part to teach. */
  next: number
  parts: number
  last: number | null
  taught: number[]
  skipped: number[]
  finished: boolean
}

export interface Conversation {
  id: number
  course_id: number
  mode: TutorMode
  title: string
  context_mode: ContextMode
  scope: ConversationScope
  profile_id: number | null
  created_at: string
  updated_at: string
  message_count: number
  last_message: string | null
  section_id: number | null
  section_title: string | null
  step_id: number | null
  resource_id: number | null
  resource_title: string | null
  progress: TeachProgress
}

export type MessageStatus = 'done' | 'streaming' | 'cancelled' | 'error'

/** What a teaching turn did (stored on both the student's and the AI's message). */
export interface PlanMeta {
  action?: string
  target?: number | null
  /** Lesson part taught in this turn. */
  part?: number | null
  parts?: number
  /** Knowledge points fully taught in this turn (quiz suggestions). */
  covered_points?: number[]
}

export interface MessageMeta {
  profile?: string
  model?: string
  context_mode?: 'full' | 'rag' | 'lesson' | 'none'
  plan?: PlanMeta
  context_tokens?: number
  material_tokens?: number
  context_truncated?: boolean
  notice?: string | null
  elapsed_ms?: number
  finish_reason?: string | null
  output_truncated?: boolean
  reasoning?: string
  partial?: boolean
  error_kind?: string
  usage?: { prompt: number; completion: number; estimated: boolean }
  input?: string
  [key: string]: unknown
}

export interface Message {
  id: number
  conversation_id: number
  role: 'user' | 'assistant'
  content: string
  status: MessageStatus
  error: string | null
  sources: SourceRef[]
  meta: MessageMeta
  created_at: string
}

export interface ConversationDetail {
  conversation: Conversation
  messages: Message[]
}

export type ChatEvent =
  | { type: 'started'; request_id: string; user_message: Message | null; assistant_message: Message }
  | { type: 'delta'; text: string }
  | { type: 'reasoning'; text: string }
  | { type: 'done'; message: Message }
