import { defineStore } from 'pinia'
import { reactive } from 'vue'
import { DocumentService } from '@/services/document'
import { useToastStore } from '@/stores/toast'
import type { ImportProgress } from '@/types/document'
import { errorMessage } from '@/utils/errors'

interface CourseTasks {
  importing: boolean
  reprocessing: boolean
  importStatus: string
  revision: number
  lastImportedId: number | null
}

// Tasks belong to the app, so leaving the documents page does not lose the queue or progress.
export const useDocumentTasksStore = defineStore('documentTasks', () => {
  const courses = reactive<Record<number, CourseTasks>>({})
  const progress = reactive<Record<number, ImportProgress>>({})
  const toast = useToastStore()

  function forCourse(courseId: number) {
    return courses[courseId] ??= {
      importing: false, reprocessing: false, importStatus: '', revision: 0, lastImportedId: null
    }
  }

  function onProgress(p: ImportProgress) {
    progress[p.document_id] = p
  }

  async function importFiles(courseId: number, files: string[]) {
    const task = forCourse(courseId)
    if (task.importing || !files.length) return
    task.importing = true
    let failures = 0
    let queued = 0
    const errors: string[] = []
    try {
      for (let index = 0; index < files.length; index++) {
        const filename = files[index].split(/[\\/]/).pop()
        task.importStatus = `正在导入 ${index + 1}/${files.length}：${filename}（正在保存资料）`
        try {
          const document = await DocumentService.import(courseId, files[index], onProgress)
          if (document.status === 'pending' || document.status === 'processing') queued++
          if (document.status === 'failed') {
            failures++
            errors.push(`${filename}：${document.error ?? '解析失败'}`)
          }
          task.lastImportedId = document.id
          delete progress[document.id]
        } catch (e) {
          failures++
          errors.push(`${filename}：${errorMessage(e)}`)
        } finally {
          task.revision++
        }
      }
      if (failures) toast.error(`导入完成：${files.length - failures} 个成功，${failures} 个失败。\n${errors.join('\n')}`)
      else toast.success(queued ? `已添加 ${files.length} 份资料，正在后台解析` : `已导入 ${files.length} 份资料`)
    } finally {
      task.importStatus = ''
      task.importing = false
    }
  }

  async function reprocess(courseId: number, id: number) {
    const task = forCourse(courseId)
    if (task.importing || task.reprocessing) return
    task.reprocessing = true
    try {
      const document = await DocumentService.reprocess(id, onProgress)
      if (document.status === 'failed') toast.error(document.error ?? '解析失败')
      else toast.success(document.status === 'completed' ? '已重新解析' : '已加入后台解析队列')
    } catch (e) {
      toast.error(e)
    } finally {
      delete progress[id]
      task.reprocessing = false
      task.revision++
    }
  }

  return { forCourse, progress, importFiles, reprocess }
})
