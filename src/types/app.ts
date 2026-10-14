// Mirrors src-tauri/src/settings.rs and src-tauri/src/commands/app.rs

export interface ContextSettings {
  mode: 'auto' | 'full' | 'rag'
  rag_top_k: number
  full_ratio: number
  history_turns: number
}

export interface AppSettings {
  restore_last_location: boolean
  context: ContextSettings
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
