// Mirrors src-tauri/src/models/course.rs

export interface Course {
  id: number
  name: string
  description: string
  /** YYYY-MM-DD */
  exam_date: string | null
  created_at: string
  updated_at: string
}

export interface CreateCourseInput {
  name: string
  description: string
}

export interface UpdateCourseInput {
  id: number
  name?: string
  description?: string
  /** '' clears the date */
  exam_date?: string
}
