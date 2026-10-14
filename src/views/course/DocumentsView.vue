<template>
  <div class="h-full overflow-y-auto p-6 text-slate-900">
    <div class="mb-3 flex flex-wrap items-center gap-4">
      <button @click="importFiles" :disabled="importing" class="btn btn-primary">
        {{ importing ? '正在导入…' : '导入资料' }}
      </button>
      <p class="text-sm text-slate-600">PDF、Word (.docx)、PPT (.pptx)、TXT、Markdown</p>
    </div>
    <p class="mb-6 text-sm text-slate-500">原文件保持不变。图片文字由本地 OCR 识别，支持简体中文、繁体中文和英文。</p>
    <p v-if="progress" role="status" class="mb-4 break-words text-indigo-700">{{ progress }}</p>
    <p v-if="error" role="alert" class="mb-4 break-words whitespace-pre-wrap text-rose-700">{{ error }}</p>
    <p v-if="loading" class="text-slate-500">加载中…</p>
    <EmptyState v-else-if="!documents.length" title="暂无资料" description="点击上方按钮导入教材、PPT 或笔记。" />
    <div v-else class="grid grid-cols-1 gap-5 md:grid-cols-[240px_minmax(0,1fr)] lg:grid-cols-[320px_minmax(0,1fr)]">
      <section aria-label="资料列表" class="space-y-3">
        <button v-for="document in documents" :key="document.id" @click="selectDocument(document)"
          :aria-pressed="selected?.id === document.id"
          :class="selected?.id === document.id ? 'border-indigo-500 ring-1 ring-indigo-500' : 'border-slate-200'"
          class="block w-full rounded-lg border bg-white p-4 text-left hover:border-indigo-400">
          <h2 class="font-semibold break-all">{{ document.filename }}</h2>
          <p class="mt-2 text-xs text-slate-500">{{ document.file_type.toUpperCase() }} · {{ formatBytes(document.file_size) }} · #{{ document.id }}</p>
          <p class="mt-2 text-sm" :class="document.status === 'failed' ? 'text-rose-700' : 'text-slate-600'">{{ statusLabels[document.status] }}</p>
        </button>
      </section>
      <section aria-label="提取正文" class="card min-w-0 p-5">
        <template v-if="selected">
          <h2 class="mb-4 font-semibold break-all">{{ selected.filename }}</h2>
          <p v-if="selected.status === 'failed'" role="alert" class="break-words text-rose-700">{{ selected.error || '正文提取失败，请检查文件后重新导入。' }}</p>
          <p v-else-if="selected.status !== 'completed'" class="text-slate-500">正在提取正文，扫描资料可能需要较长时间。</p>
          <p v-else-if="contentLoading" class="text-slate-500">正在读取正文…</p>
          <p v-else-if="contentError" role="alert" class="text-rose-700">{{ contentError }}</p>
          <pre v-else class="font-sans text-sm leading-7 break-words whitespace-pre-wrap">{{ content }}</pre>
        </template>
        <p v-else class="text-slate-500">选择左侧资料查看正文。</p>
      </section>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { onBeforeRouteLeave } from 'vue-router'
import { open } from '@tauri-apps/plugin-dialog'
import EmptyState from '@/components/ui/EmptyState.vue'
import { DocumentService } from '@/services/document'
import type { Course, Document } from '@/types/models'
import { errorMessage as message } from '@/utils/errors'
import { formatBytes } from '@/utils/format'

const props = defineProps<{ course: Course }>()
const courseId = props.course.id
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

onMounted(async () => {
  try {
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
