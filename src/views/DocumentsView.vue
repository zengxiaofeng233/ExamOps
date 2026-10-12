<template>
  <div class="w-full h-full flex flex-col bg-gray-50 text-gray-900">
    <header class="bg-white border-b border-gray-200 px-6 py-4">
      <button :disabled="importing" @click="router.push('/courses')" class="text-sm text-blue-600 disabled:opacity-50">← 返回课程</button>
      <h1 class="text-2xl font-bold mt-2 break-words">{{ course?.name || '课程' }} · 资料</h1>
    </header>
    <main class="flex-1 min-h-0 overflow-y-auto p-6">
      <div class="flex flex-wrap items-center gap-4 mb-3">
        <button @click="importFiles" :disabled="importing || !course" class="px-4 py-2 rounded-lg bg-blue-600 text-white hover:bg-blue-700 disabled:opacity-50">
          {{ importing ? '正在导入…' : '导入资料' }}
        </button>
        <p class="text-sm text-gray-600">PDF、Word (.docx)、PPT (.pptx)、TXT、Markdown</p>
      </div>
      <p class="text-sm text-gray-500 mb-6">原文件保持不变。图片文字由本地 OCR 识别，支持简体中文、繁体中文和英文。</p>
      <p v-if="progress" role="status" class="mb-4 text-blue-700 break-words">{{ progress }}</p>
      <p v-if="error" role="alert" class="mb-4 text-red-700 whitespace-pre-wrap break-words">{{ error }}</p>
      <p v-if="loading" class="text-gray-500">加载中…</p>
      <p v-else-if="!documents.length" class="py-12 text-center text-gray-500">暂无资料，点击上方按钮导入。</p>
      <div v-else class="grid grid-cols-1 md:grid-cols-[240px_minmax(0,1fr)] lg:grid-cols-[320px_minmax(0,1fr)] gap-5">
        <section aria-label="资料列表" class="space-y-3">
          <button v-for="document in documents" :key="document.id" @click="selectDocument(document)"
            :aria-pressed="selected?.id === document.id"
            :class="selected?.id === document.id ? 'border-blue-500 ring-1 ring-blue-500' : 'border-gray-200'"
            class="block w-full text-left bg-white border rounded-lg p-4 hover:border-blue-400">
            <h2 class="font-semibold break-all">{{ document.filename }}</h2>
            <p class="text-xs text-gray-500 mt-2">{{ document.file_type.toUpperCase() }} · {{ formatSize(document.file_size) }} · #{{ document.id }}</p>
            <p class="mt-2 text-sm" :class="document.status === 'failed' ? 'text-red-700' : 'text-gray-600'">{{ statusLabels[document.status] }}</p>
          </button>
        </section>
        <section aria-label="提取正文" class="min-w-0 bg-white border border-gray-200 rounded-lg p-5">
          <template v-if="selected">
            <h2 class="font-semibold break-all mb-4">{{ selected.filename }}</h2>
            <p v-if="selected.status === 'failed'" role="alert" class="text-red-700 break-words">{{ selected.error || '正文提取失败，请检查文件后重新导入。' }}</p>
            <p v-else-if="selected.status !== 'completed'" class="text-gray-500">正在提取正文，扫描资料可能需要较长时间。</p>
            <p v-else-if="contentLoading" class="text-gray-500">正在读取正文…</p>
            <p v-else-if="contentError" role="alert" class="text-red-700">{{ contentError }}</p>
            <pre v-else class="font-sans text-sm leading-7 whitespace-pre-wrap break-words">{{ content }}</pre>
          </template>
          <p v-else class="text-gray-500">选择左侧资料查看正文。</p>
        </section>
      </div>
    </main>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useRoute, useRouter, onBeforeRouteLeave } from 'vue-router'
import { open } from '@tauri-apps/plugin-dialog'
import { CourseService } from '@/services/course'
import { DocumentService } from '@/services/document'
import type { Course, Document } from '@/types/models'

const route = useRoute()
const router = useRouter()
const courseId = Number(route.params.id)
const course = ref<Course>()
const documents = ref<Document[]>([])
const selected = ref<Document>()
const content = ref('')
const contentError = ref('')
const contentLoading = ref(false)
const loading = ref(true)
const importing = ref(false)
const error = ref('')
const progress = ref('')
const statusLabels = { pending: '待处理', processing: '正在提取正文', completed: '提取完成', failed: '提取失败' }
const message = (e: unknown) => e instanceof Error ? e.message : String(e)
const formatSize = (bytes: number) => bytes < 1024 * 1024 ? `${(bytes / 1024).toFixed(1)} KB` : `${(bytes / 1024 / 1024).toFixed(1)} MB`

onMounted(async () => {
  try {
    course.value = (await CourseService.getCourses()).find(item => item.id === courseId)
    if (!course.value) throw new Error('课程不存在')
    documents.value = await DocumentService.list(courseId)
  } catch (e) { error.value = message(e) }
  finally { loading.value = false }
})

// Keep the page that owns the sequential import queue alive until it is finished.
onBeforeRouteLeave(() => !importing.value)

async function selectDocument(document: Document) {
  selected.value = document
  content.value = ''
  contentError.value = ''
  contentLoading.value = false
  if (document.status !== 'completed') return
  contentLoading.value = true
  try {
    const body = await DocumentService.content(document.id)
    if (selected.value?.id === document.id) content.value = body
  } catch (e) {
    if (selected.value?.id === document.id) contentError.value = message(e)
  } finally {
    if (selected.value?.id === document.id) contentLoading.value = false
  }
}

async function importFiles() {
  importing.value = true
  error.value = ''
  progress.value = ''
  let refresh: ReturnType<typeof setInterval> | undefined
  try {
    const paths = await open({ multiple: true, directory: false, filters: [
      { name: '课程资料', extensions: ['pdf', 'docx', 'pptx', 'txt', 'md', 'markdown'] }
    ] })
    if (!paths) return
    const files = Array.isArray(paths) ? paths : [paths]
    let failures = 0
    refresh = setInterval(async () => {
      try { documents.value = await DocumentService.list(courseId) }
      catch (e) { error.value = message(e) }
    }, 1000)
    for (let index = 0; index < files.length; index++) {
      const filename = files[index].split(/[\\/]/).pop()
      progress.value = `正在处理 ${index + 1}/${files.length}：${filename}。OCR 按页执行，请稍候。`
      try {
        const document = await DocumentService.import(courseId, files[index])
        if (document.status === 'failed') failures++
        documents.value = await DocumentService.list(courseId)
        await selectDocument(document)
      } catch (e) {
        failures++
        error.value += `${filename}：${message(e)}\n`
      }
    }
    progress.value = `处理结束：${files.length - failures} 个成功，${failures} 个失败。`
  } catch (e) { error.value = message(e) }
  finally {
    if (refresh) clearInterval(refresh)
    importing.value = false
  }
}
</script>
