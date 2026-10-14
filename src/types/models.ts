// Mirrors src-tauri/src/models/course.rs

export interface Course {
  id: number
  name: string
  description: string
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
}
