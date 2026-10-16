// Mirrors src-tauri/src/documents/mod.rs, search/mod.rs and commands/document.rs

/** What one "unit" of a document is: PDF page, PPT slide, Word paragraph or text lines. */
export type LocKind = 'page' | 'slide' | 'paragraph' | 'line'

export type DocumentStatus = 'pending' | 'processing' | 'completed' | 'failed'

export interface Document {
  id: number
  course_id: number
  filename: string
  file_type: string
  file_path: string
  file_size: number
  status: DocumentStatus
  created_at: string
  error?: string | null
  page_count: number
  loc_kind: LocKind
  char_count: number
  /** outline | styles | markdown | slides | patterns | layout | none | legacy */
  structure_source: string
  warnings: string[]
  indexed_at: string | null
  section_count: number
  image_count: number
}

export interface Section {
  id: number
  document_id: number
  parent_id: number | null
  ordinal: number
  level: number
  title: string
  page_start: number
  page_end: number
  char_count: number
  loc_label: string
  block_start?: number
  block_end?: number
  source?: string
  user_edited?: boolean
  locked?: boolean
}

export interface CourseSection extends Section {
  filename: string
  file_type: string
}

export interface Block {
  text: string
  heading: number | null
  kind: string
  raw_text?: string | null
  bbox?: [number, number, number, number] | null
  source?: string | null
  recognition?: string
  issues?: string[]
  formula?: boolean
}

export interface ImageInfo {
  id: number
  page_number: number
  ordinal: number
  width: number
  height: number
  ocr_text: string
}

export interface PageView {
  page_number: number
  label: string
  loc_start: number
  loc_end: number
  blocks: Block[]
  has_ocr: boolean
  width: number | null
  height: number | null
  images: ImageInfo[]
}

export interface ImportProgress {
  document_id: number
  stage: string
  current: number
  total: number
}

export interface Segment {
  text: string
  hit: boolean
}

export interface SearchHit {
  chunk_id: number
  document_id: number
  filename: string
  file_type: string
  section_id: number | null
  section_title: string | null
  page_start: number
  page_end: number
  loc_label: string
  snippet: Segment[]
  score: number
}

export interface SearchResult {
  query: string
  terms: string[]
  hits: SearchHit[]
}
export type SectionEdit =
  | { action: 'rename'; title: string }
  | { action: 'lock'; locked: boolean }
  | { action: 'merge'; next_id: number }
  | { action: 'split'; page: number; block: number; title: string }
