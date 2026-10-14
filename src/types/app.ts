// Mirrors src-tauri/src/settings.rs and src-tauri/src/commands/app.rs

export interface AppSettings {
  restore_last_location: boolean
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
