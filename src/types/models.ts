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

export interface Document {
  id: number
  course_id: number
  filename: string
  file_type: string
  file_path: string
  file_size: number
  status: 'pending' | 'processing' | 'completed' | 'failed'
  created_at: string
}

export interface Note {
  id: number
  course_id: number
  title: string
  content: string
  document_id?: number
  page_reference?: string
  created_at: string
  updated_at: string
}

export interface ChatMessage {
  id: number
  course_id: number
  session_id?: number
  role: 'user' | 'assistant' | 'system'
  content: string
  created_at: string
}
