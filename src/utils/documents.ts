import type { Document, LocKind } from '@/types/document'

export const FILE_LABELS: Record<string, string> = {
  pdf: 'PDF',
  docx: 'Word',
  pptx: 'PPT',
  txt: 'TXT',
  md: 'Markdown',
  markdown: 'Markdown'
}

export const STATUS_LABELS: Record<Document['status'], string> = {
  pending: '待处理',
  processing: '正在解析',
  completed: '已解析',
  failed: '解析失败'
}

export const STRUCTURE_LABELS: Record<string, string> = {
  outline: '按 PDF 书签识别章节',
  styles: '按 Word 标题样式识别章节',
  markdown: '按 Markdown 标题识别章节',
  slides: '按幻灯片标题识别章节',
  patterns: '按编号格式（如“第三章”“3.2”）识别章节',
  layout: '按字号版式识别章节',
  none: '未识别到章节，按页码分组',
  legacy: '旧版本导入，重新解析后可识别章节和图片',
  '': '尚未解析'
}

export const UNIT_NAMES: Record<LocKind, string> = {
  page: '页',
  slide: '张幻灯片',
  paragraph: '段',
  line: '行'
}

export function unitCount(document: Pick<Document, 'loc_kind' | 'page_count'>): string {
  const unit = document.loc_kind === 'line' ? '个文本块' : UNIT_NAMES[document.loc_kind]
  return `${document.page_count} ${unit}`
}

/** Readers say "教材" for long text and "PPT" for slides in learning steps. */
export function isSlides(fileType: string) {
  return fileType === 'pptx'
}
