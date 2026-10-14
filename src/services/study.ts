import { invoke } from '@tauri-apps/api/core'
import type {
  CourseDigest, ExtractResult, KnowledgeInput, KnowledgePoint, LearningPath, MarkInput, Note, NoteInput, PathDetail, PathInput,
  PathStep, RecommendInput, Resource, ResourceInput, StepInput, StepStatus, StudyState, TemplateInput
} from '@/types/study'

export const KnowledgeService = {
  list: (courseId: number) => invoke<KnowledgePoint[]>('list_knowledge_points', { courseId }),
  save: (input: KnowledgeInput) => invoke<KnowledgePoint>('save_knowledge_point', { input }),
  remove: (id: number) => invoke<void>('delete_knowledge_point', { id }),
  setDifficult: (id: number, difficult: boolean) => invoke<KnowledgePoint>('set_knowledge_point_difficult', { id, difficult }),
  setStatus: (id: number, status: 'new' | 'learned') => invoke<KnowledgePoint>('set_knowledge_point_status', { id, status }),
  fromHeadings: (courseId: number, documentId?: number) =>
    invoke<number>('generate_knowledge_points', { courseId, documentId: documentId ?? null }),
  extract: (courseId: number, sectionId: number) => invoke<ExtractResult>('extract_knowledge_points', { courseId, sectionId })
}

export const PathService = {
  list: (courseId: number) => invoke<LearningPath[]>('list_learning_paths', { courseId }),
  get: (id: number) => invoke<PathDetail>('get_learning_path', { id }),
  save: (input: PathInput) => invoke<LearningPath>('save_learning_path', { input }),
  remove: (id: number) => invoke<void>('delete_learning_path', { id }),
  activate: (courseId: number, id: number) => invoke<StudyState>('activate_learning_path', { courseId, id }),
  fromTemplate: (input: TemplateInput) => invoke<PathDetail>('create_path_from_template', { input }),
  recommend: (input: RecommendInput) => invoke<PathDetail>('recommend_learning_path', { input }),
  saveStep: (input: StepInput) => invoke<PathStep>('save_path_step', { input }),
  removeStep: (id: number) => invoke<void>('delete_path_step', { id }),
  reorder: (pathId: number, stepIds: number[]) => invoke<PathStep[]>('reorder_path_steps', { pathId, stepIds }),
  setStatus: (id: number, status: StepStatus, note?: string) => invoke<PathStep>('set_path_step_status', { id, status, note: note ?? null })
}

export const ResourceService = {
  list: (courseId: number) => invoke<Resource[]>('list_resources', { courseId }),
  save: (input: ResourceInput) => invoke<Resource>('save_resource', { input }),
  remove: (id: number) => invoke<void>('delete_resource', { id }),
  saveMark: (input: MarkInput) => invoke<Resource>('save_resource_mark', { input }),
  removeMark: (resourceId: number, id: number) => invoke<Resource>('delete_resource_mark', { resourceId, id })
}

export const NoteService = {
  list: (courseId: number) => invoke<Note[]>('list_notes', { courseId }),
  save: (input: NoteInput) => invoke<Note>('save_note', { input }),
  remove: (id: number) => invoke<void>('delete_note', { id })
}

export const StudyService = {
  state: (courseId: number) => invoke<StudyState>('get_study_state', { courseId }),
  dashboard: () => invoke<CourseDigest[]>('get_dashboard'),
  saveReading: (courseId: number, documentId: number, page: number) =>
    invoke<void>('save_reading_position', { courseId, documentId, page })
}
