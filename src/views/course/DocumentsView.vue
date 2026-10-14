<template>
  <div class="flex h-full min-h-0">
    <!-- Library -->
    <aside v-show="showLibrary" class="flex w-64 shrink-0 flex-col border-r border-slate-200 bg-white xl:w-72">
      <div class="space-y-2 border-b border-slate-100 p-3">
        <button class="btn btn-primary w-full" :disabled="importing" @click="importFiles">
          <ArrowUpTrayIcon class="size-4" />{{ importing ? '正在导入…' : '导入资料' }}
        </button>
        <p class="text-[11px] leading-4 text-slate-500">PDF、Word(.docx)、PPT(.pptx)、TXT、Markdown。原文件保持不变；图片文字由本地 OCR 识别。</p>
      </div>
      <p v-if="importStatus" role="status" class="border-b border-slate-100 bg-indigo-50 px-3 py-2 text-xs break-words text-indigo-700">{{ importStatus }}</p>
      <div class="min-h-0 flex-1 overflow-y-auto p-2">
        <p v-if="loading" class="p-3 text-sm text-slate-400">加载中…</p>
        <EmptyState v-else-if="!documents.length" title="暂无资料" description="导入教材、课件或笔记后即可阅读、检索并让 AI 讲解。" />
        <ul v-else class="space-y-1.5">
          <li v-for="item in documents" :key="item.id">
            <button
              class="block w-full rounded-lg border p-3 text-left transition-colors"
              :class="selected?.id === item.id ? 'border-indigo-400 bg-indigo-50/60' : 'border-transparent hover:border-slate-200 hover:bg-slate-50'"
              :aria-pressed="selected?.id === item.id"
              @click="openDocument(item.id)"
            >
              <p class="line-clamp-2 text-sm font-medium break-all text-slate-800">{{ item.filename }}</p>
              <p class="mt-1 flex items-center gap-1.5 text-xs text-slate-500">
                <span class="badge" :class="badgeClass(item)">{{ STATUS_LABELS[item.status] }}</span>
                <span>{{ FILE_LABELS[item.file_type] ?? item.file_type }}</span>
                <span v-if="item.status === 'completed'">· {{ item.section_count }} 章节</span>
              </p>
              <p v-if="progress[item.id] && item.status === 'processing'" class="mt-1 text-xs text-indigo-600">{{ progressText(item.id) }}</p>
            </button>
          </li>
        </ul>
      </div>
    </aside>

    <!-- Reader / search -->
    <section class="flex min-w-0 flex-1 flex-col">
      <form class="flex items-center gap-2 border-b border-slate-200 bg-white px-3 py-2.5" role="search" @submit.prevent="runSearch">
        <button
          v-if="documents.length"
          type="button"
          class="btn btn-ghost btn-sm"
          :title="showLibrary ? '隐藏资料列表' : '显示资料列表'"
          :aria-pressed="showLibrary"
          @click="toggleLibrary"
        >
          <Bars3Icon class="size-4" />
        </button>
        <MagnifyingGlassIcon class="size-4 shrink-0 text-slate-400" />
        <input
          v-model="query"
          type="search"
          maxlength="200"
          class="min-w-0 flex-1 border-0 bg-transparent text-sm outline-none placeholder:text-slate-400"
          placeholder="在本课程资料中检索关键词，例如：导数 几何意义"
          aria-label="检索资料"
        />
        <select v-model="searchScope" class="rounded-md border border-slate-200 bg-white px-2 py-1 text-xs text-slate-600" aria-label="检索范围">
          <option value="all">全部资料</option>
          <option value="current" :disabled="!selected">当前资料</option>
        </select>
        <button type="submit" class="btn btn-soft btn-sm" :disabled="!query.trim()">检索</button>
        <button v-if="searchResult" type="button" class="btn btn-ghost btn-sm" @click="clearSearch">返回阅读</button>
      </form>
      <div class="min-h-0 flex-1">
        <SearchResults v-if="searchResult || searching" :result="searchResult" :loading="searching" @open="openHit" />
        <DocumentReader
          v-else-if="selected"
          :key="selected.id"
          :document="selected"
          :page="targetPage"
          :terms="highlight"
          :course-id="course.id"
          :busy="importing || reprocessing"
          @reprocess="reprocess"
          @delete="confirmDelete = true"
          @visible-page="rememberPage"
        />
        <EmptyState v-else-if="documents.length" title="选择一份资料开始阅读" description="左侧列表中的资料解析完成后，可按章节阅读并查看原页面。" />
        <EmptyState v-else-if="!loading" title="导入第一份资料" description="支持 PDF、Word、PPT、TXT 和 Markdown。导入后会自动识别章节，可以阅读、检索，并让 AI 按章节讲解。">
          <button class="btn btn-primary" :disabled="importing" @click="importFiles"><ArrowUpTrayIcon class="size-4" />导入资料</button>
        </EmptyState>
      </div>
    </section>

    <ConfirmDialog
      :open="confirmDelete"
      title="删除资料"
      :message="`确定删除“${selected?.filename}”吗？\n应用内副本、提取正文、章节和图片将一并删除，你电脑上的原文件不受影响。`"
      confirm-text="删除"
      danger
      :busy="deleting"
      @cancel="confirmDelete = false"
      @confirm="removeSelected"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue'
import { onBeforeRouteLeave, useRoute, useRouter } from 'vue-router'
import { open } from '@tauri-apps/plugin-dialog'
import { ArrowUpTrayIcon, Bars3Icon, MagnifyingGlassIcon } from '@heroicons/vue/20/solid'
import ConfirmDialog from '@/components/ui/ConfirmDialog.vue'
import EmptyState from '@/components/ui/EmptyState.vue'
import DocumentReader from '@/components/documents/DocumentReader.vue'
import SearchResults from '@/components/documents/SearchResults.vue'
import { DocumentService } from '@/services/document'
import { StudyService } from '@/services/study'
import { useToastStore } from '@/stores/toast'
import type { Course } from '@/types/models'
import type { Document, ImportProgress, SearchHit, SearchResult } from '@/types/document'
import { FILE_LABELS, STATUS_LABELS, UNIT_NAMES } from '@/utils/documents'
import { errorMessage } from '@/utils/errors'

const props = defineProps<{ course: Course }>()
const route = useRoute()
const router = useRouter()
const toast = useToastStore()

const documents = ref<Document[]>([])
const loading = ref(true)
const importing = ref(false)
const reprocessing = ref(false)
const deleting = ref(false)
const confirmDelete = ref(false)
const importStatus = ref('')
const progress = reactive<Record<number, ImportProgress>>({})
const targetPage = ref<number | null>(null)
const query = ref('')
const searchScope = ref<'all' | 'current'>('all')
const searchResult = ref<SearchResult | null>(null)
const searching = ref(false)
const highlight = ref<string[]>([])
const LIBRARY_KEY = 'examops.documents.libraryHidden'
const libraryHidden = ref(readFlag(LIBRARY_KEY))

function readFlag(key: string) {
  try { return localStorage.getItem(key) === '1' } catch { return false }
}

// The smallest window (800 px) cannot show the library, the chapter tree and the page side by
// side, so in narrow windows the library folds away while a document is open (not remembered).
const NARROW = '(max-width: 1100px)'
const narrow = ref(false)
const libraryOpenWhenNarrow = ref(false)
let narrowQuery: MediaQueryList | null = null
const syncNarrow = () => { narrow.value = narrowQuery?.matches ?? false }

function toggleLibrary() {
  if (narrow.value) {
    libraryOpenWhenNarrow.value = !showLibrary.value
    return
  }
  libraryHidden.value = !libraryHidden.value
  try { localStorage.setItem(LIBRARY_KEY, libraryHidden.value ? '1' : '0') } catch { /* storage unavailable */ }
}

const selectedId = computed(() => Number(route.query.doc) || null)
const selected = computed(() => documents.value.find(d => d.id === selectedId.value) ?? null)
const showLibrary = computed(() => {
  if (!documents.value.length) return true
  return narrow.value ? libraryOpenWhenNarrow.value || !selected.value : !libraryHidden.value
})

let poll: ReturnType<typeof setInterval> | undefined
let reportedPage: number | null = null

async function refresh() {
  documents.value = await DocumentService.list(props.course.id)
}

onMounted(async () => {
  narrowQuery = window.matchMedia(NARROW)
  syncNarrow()
  narrowQuery.addEventListener('change', syncNarrow)
  try {
    await refresh()
    targetPage.value = Number(route.query.page) || null
    reportedPage = targetPage.value
    if (route.query.q) highlight.value = String(route.query.q).split(/\s+/).filter(Boolean)
    if (!selected.value && documents.value.length) {
      const first = documents.value.find(d => d.status === 'completed') ?? documents.value[0]
      router.replace({ query: { ...route.query, doc: first.id } })
    }
  } catch (e) {
    toast.error(e)
  } finally {
    loading.value = false
  }
})

onBeforeUnmount(() => {
  clearInterval(poll)
  narrowQuery?.removeEventListener('change', syncNarrow)
})
// Keep the page that owns the sequential import queue alive until it is finished.
onBeforeRouteLeave(() => !importing.value)

// Navigation from elsewhere (citations, learning steps) changes doc/page in the URL.
watch(() => [route.query.doc, route.query.page], ([, page]) => {
  const wanted = Number(page) || null
  if (wanted !== reportedPage) {
    targetPage.value = wanted
    reportedPage = wanted
  }
  if (route.query.q) highlight.value = String(route.query.q).split(/\s+/).filter(Boolean)
})

function openDocument(id: number, page?: number) {
  libraryOpenWhenNarrow.value = false
  searchResult.value = null
  targetPage.value = page ?? null
  reportedPage = targetPage.value
  router.replace({ query: { doc: id, ...(page ? { page } : {}) } })
}

function rememberPage(page: number) {
  reportedPage = page
  router.replace({ query: { ...route.query, page } })
  // Restored from the overview ("上次阅读") and counted as study activity.
  if (selected.value) StudyService.saveReading(props.course.id, selected.value.id, page).catch(() => undefined)
}

function badgeClass(document: Document) {
  return {
    completed: 'bg-emerald-50 text-emerald-700',
    failed: 'bg-rose-50 text-rose-700',
    processing: 'bg-indigo-50 text-indigo-700',
    pending: 'bg-slate-100 text-slate-600'
  }[document.status]
}

function progressText(id: number) {
  const p = progress[id]
  if (!p || !p.total) return '正在解析…'
  const unit = p.stage === 'slide' ? '张幻灯片' : p.stage === 'paragraph' ? '段' : UNIT_NAMES.page
  return `正在解析：第 ${p.current}/${p.total} ${unit}`
}

function onProgress(p: ImportProgress) {
  progress[p.document_id] = p
}

async function importFiles() {
  let files: string[] = []
  try {
    const picked = await open({ multiple: true, directory: false, filters: [
      { name: '课程资料', extensions: ['pdf', 'docx', 'pptx', 'txt', 'md', 'markdown'] }
    ] })
    if (!picked) return
    files = Array.isArray(picked) ? picked : [picked]
  } catch (e) {
    toast.error(e)
    return
  }
  importing.value = true
  poll = setInterval(() => refresh().catch(() => undefined), 1500)
  let failures = 0
  const errors: string[] = []
  try {
    for (let index = 0; index < files.length; index++) {
      const filename = files[index].split(/[\\/]/).pop()
      importStatus.value = `正在导入 ${index + 1}/${files.length}：${filename}（扫描页需要 OCR，请稍候）`
      try {
        const document = await DocumentService.import(props.course.id, files[index], onProgress)
        if (document.status === 'failed') {
          failures++
          errors.push(`${filename}：${document.error ?? '解析失败'}`)
        }
        await refresh()
        if (index === files.length - 1 || !selected.value) openDocument(document.id)
      } catch (e) {
        failures++
        errors.push(`${filename}：${errorMessage(e)}`)
      }
    }
    importStatus.value = ''
    if (failures) toast.error(`导入完成：${files.length - failures} 个成功，${failures} 个失败。\n${errors.join('\n')}`)
    else toast.success(`已导入 ${files.length} 份资料`)
  } finally {
    clearInterval(poll)
    importing.value = false
    await refresh().catch(() => undefined)
  }
}

async function reprocess() {
  if (!selected.value) return
  reprocessing.value = true
  const id = selected.value.id
  poll = setInterval(() => refresh().catch(() => undefined), 1500)
  try {
    const document = await DocumentService.reprocess(id, onProgress)
    if (document.status === 'failed') toast.error(document.error ?? '解析失败')
    else toast.success('已重新解析')
  } catch (e) {
    toast.error(e)
  } finally {
    clearInterval(poll)
    reprocessing.value = false
    await refresh().catch(() => undefined)
  }
}

async function removeSelected() {
  if (!selected.value) return
  deleting.value = true
  try {
    await DocumentService.remove(selected.value.id)
    confirmDelete.value = false
    await refresh()
    router.replace({ query: {} })
    toast.success('资料已删除')
  } catch (e) {
    toast.error(e)
  } finally {
    deleting.value = false
  }
}

async function runSearch() {
  const text = query.value.trim()
  if (!text) return
  searching.value = true
  searchResult.value = null
  try {
    searchResult.value = await DocumentService.search(
      props.course.id, text, searchScope.value === 'current' ? selected.value?.id : undefined)
    highlight.value = searchResult.value.terms
  } catch (e) {
    toast.error(e)
  } finally {
    searching.value = false
  }
}

function clearSearch() {
  searchResult.value = null
}

function openHit(hit: SearchHit) {
  const terms = searchResult.value?.terms ?? []
  openDocument(hit.document_id, hit.page_start)
  highlight.value = terms
}
</script>
