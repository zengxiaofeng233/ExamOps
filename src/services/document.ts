import { invoke } from '@tauri-apps/api/core'
import type { Document } from '@/types/models'

export const DocumentService = {
  list: (courseId: number) => invoke<Document[]>('get_documents', { courseId }),
  import: (courseId: number, path: string) => invoke<Document>('import_document', { courseId, path }),
  content: (id: number) => invoke<string>('get_document_content', { id })
}
