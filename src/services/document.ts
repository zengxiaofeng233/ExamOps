import { Channel, invoke } from '@tauri-apps/api/core'
import type { CourseSection, Document, ImportProgress, PageView, SearchResult, Section } from '@/types/document'

function progressChannel(onProgress?: (progress: ImportProgress) => void) {
  const channel = new Channel<ImportProgress>()
  channel.onmessage = (message) => onProgress?.(message)
  return channel
}

export const DocumentService = {
  list: (courseId: number) => invoke<Document[]>('get_documents', { courseId }),
  get: (id: number) => invoke<Document>('get_document', { id }),
  content: (id: number) => invoke<string>('get_document_content', { id }),
  import: (courseId: number, path: string, onProgress?: (p: ImportProgress) => void) =>
    invoke<Document>('import_document', { courseId, path, onProgress: progressChannel(onProgress) }),
  reprocess: (id: number, onProgress?: (p: ImportProgress) => void) =>
    invoke<Document>('reprocess_document', { id, onProgress: progressChannel(onProgress) }),
  remove: (id: number) => invoke<void>('delete_document', { id }),
  sections: (documentId: number) => invoke<Section[]>('get_document_sections', { documentId }),
  courseSections: (courseId: number) => invoke<CourseSection[]>('get_course_sections', { courseId }),
  pages: (documentId: number, start: number, end: number) =>
    invoke<PageView[]>('get_document_pages', { documentId, start, end }),
  preview: (documentId: number, page: number, width = 1200) =>
    invoke<string>('get_page_preview', { documentId, page, width }),
  image: (id: number) => invoke<string>('get_document_image', { id }),
  search: (courseId: number, query: string, documentId?: number) =>
    invoke<SearchResult>('search_course', { courseId, query, documentId: documentId ?? null, limit: 50 })
}
