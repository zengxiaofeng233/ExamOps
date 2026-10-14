<template>
  <div class="flex h-full min-h-0 flex-col">
    <!-- Document header -->
    <header class="border-b border-slate-200 bg-white px-5 py-3">
      <div class="flex flex-wrap items-start justify-between gap-3">
        <div class="min-w-0">
          <h2 class="truncate text-base font-semibold text-slate-900" :title="document.filename">{{ document.filename }}</h2>
          <p class="mt-0.5 flex flex-wrap gap-x-3 gap-y-1 text-xs text-slate-500">
            <span>{{ FILE_LABELS[document.file_type] ?? document.file_type }}</span>
            <span v-if="document.status === 'completed'">{{ unitCount(document) }}</span>
            <span v-if="document.status === 'completed'">{{ document.section_count }} 个章节</span>
            <span v-if="document.image_count">{{ document.image_count }} 张图片</span>
            <span v-if="document.char_count">{{ document.char_count.toLocaleString() }} 字</span>
            <span v-if="document.status === 'completed'" :class="document.structure_source === 'legacy' ? 'text-amber-600' : ''">
              {{ STRUCTURE_LABELS[document.structure_source] ?? document.structure_source }}
            </span>
          </p>
        </div>
        <div class="flex shrink-0 gap-1.5">
          <button class="btn btn-secondary btn-sm" :disabled="busy" @click="emit('reprocess')">
            <ArrowPathIcon class="size-3.5" />重新解析
          </button>
          <button class="btn btn-ghost btn-sm text-rose-600 hover:text-rose-700" :disabled="busy" @click="emit('delete')">删除</button>
        </div>
      </div>
      <ul v-if="document.warnings.length" class="mt-2 space-y-1">
        <li v-for="warning in document.warnings" :key="warning" class="rounded-md bg-amber-50 px-3 py-1.5 text-xs text-amber-800">{{ warning }}</li>
      </ul>
    </header>

    <div v-if="document.status === 'failed'" class="p-6">
      <div class="rounded-lg border border-rose-200 bg-rose-50 p-4 text-sm text-rose-800" role="alert">
        <p class="font-medium">解析失败</p>
        <p class="mt-1 break-words whitespace-pre-wrap">{{ document.error || '请检查文件后点击“重新解析”或重新导入。' }}</p>
      </div>
    </div>
    <div v-else-if="document.status !== 'completed'" class="p-6 text-sm text-slate-500">正在解析，扫描资料可能需要较长时间…</div>

    <div v-else class="flex min-h-0 flex-1">
      <aside class="w-56 shrink-0 overflow-y-auto border-r border-slate-200 bg-white p-2 xl:w-64">
        <p v-if="loadingSections" class="p-3 text-sm text-slate-400">加载目录…</p>
        <SectionTree v-else :sections="sections" :active-id="active?.id" @select="(s) => selectSection(s)" />
      </aside>

      <section class="flex min-w-0 flex-1 flex-col bg-slate-50">
        <div v-if="active" class="flex flex-wrap items-center gap-2 border-b border-slate-200 bg-white px-4 py-2">
          <div class="min-w-0 flex-1">
            <p class="truncate text-sm font-medium text-slate-800" :title="active.title">{{ active.title }}</p>
            <p class="text-xs text-slate-500">{{ active.loc_label }}</p>
          </div>
          <form v-if="jumpable" class="flex items-center gap-1" @submit.prevent="jump">
            <label class="text-xs text-slate-500" for="jump-page">跳到</label>
            <input id="jump-page" v-model.number="jumpTo" type="number" min="1" :max="document.page_count" class="input w-20 py-1 text-xs" />
            <span class="text-xs text-slate-500">{{ UNIT_NAMES[document.loc_kind] }}</span>
          </form>
          <label v-if="document.file_type === 'pdf'" class="flex items-center gap-1.5 text-xs text-slate-600">
            <input v-model="previewAll" type="checkbox" class="accent-indigo-600" />显示原页面
          </label>
          <div v-if="courseId" class="flex gap-1">
            <RouterLink
              :to="{ name: 'learn', params: { id: courseId }, query: { mode: 'learn', section: active.id } }"
              class="btn btn-soft btn-sm"
              title="让 AI 从零讲解这一节"
            >
              <AcademicCapIcon class="size-4" />AI 讲解
            </RouterLink>
            <RouterLink
              :to="{ name: 'notes', params: { id: courseId }, query: { new: 1, doc: document.id, page: currentPage ?? active.page_start, section: active.id } }"
              class="btn btn-secondary btn-sm"
              title="记一条关联到当前位置的笔记"
            >
              <PencilSquareIcon class="size-4" />记笔记
            </RouterLink>
          </div>
          <div class="flex gap-1">
            <button class="btn btn-ghost btn-sm" :disabled="!previous" title="上一节" @click="previous && selectSection(previous)">
              <ChevronLeftIcon class="size-4" />
            </button>
            <button class="btn btn-ghost btn-sm" :disabled="!next" title="下一节" @click="next && selectSection(next)">
              <ChevronRightIcon class="size-4" />
            </button>
          </div>
        </div>

        <div ref="scroller" class="min-h-0 flex-1 overflow-y-auto px-4 py-4">
          <p v-if="loadingPages" class="text-sm text-slate-400">加载内容…</p>
          <p v-else-if="pageError" class="text-sm text-rose-600">{{ pageError }}</p>
          <div v-else class="mx-auto max-w-3xl space-y-3">
            <div :class="compact ? 'card space-y-0.5 px-3 py-4' : 'space-y-3'">
              <PageCard
                v-for="page in pages"
                :key="`${page.page_number}-${previewAll}`"
                :page="page"
                :document-id="document.id"
                :file-type="document.file_type"
                :terms="terms"
                :targeted="page.page_number === targeted"
                :preview-all="previewAll"
                :compact="compact"
                @open-image="(image) => (viewing = { image, label: page.label })"
              />
            </div>
            <button v-if="hasMore" class="btn btn-secondary w-full" @click="loadMore">继续加载本节内容</button>
            <div v-if="active && !hasMore" class="flex justify-between pt-2 text-sm">
              <button v-if="previous" class="text-indigo-600 hover:underline" @click="selectSection(previous)">← {{ previous.title }}</button>
              <span v-else />
              <button v-if="next" class="text-indigo-600 hover:underline" @click="selectSection(next)">{{ next.title }} →</button>
            </div>
          </div>
        </div>
      </section>
    </div>

    <ImageViewer :image="viewing?.image ?? null" :label="viewing?.label" @close="viewing = null" />
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import { AcademicCapIcon, ArrowPathIcon, ChevronLeftIcon, ChevronRightIcon, PencilSquareIcon } from '@heroicons/vue/20/solid'
import SectionTree from './SectionTree.vue'
import PageCard from './PageCard.vue'
import ImageViewer from './ImageViewer.vue'
import { DocumentService } from '@/services/document'
import type { Document, ImageInfo, PageView, Section } from '@/types/document'
import { FILE_LABELS, STRUCTURE_LABELS, UNIT_NAMES, unitCount } from '@/utils/documents'
import { errorMessage } from '@/utils/errors'

const WINDOW = 120

const props = defineProps<{
  document: Document
  /** Unit to open and highlight (from search, citations or a restored location). */
  page?: number | null
  terms?: string[]
  busy?: boolean
  /** Enables the "AI 讲解" and "记笔记" shortcuts. */
  courseId?: number
}>()
const emit = defineEmits<{ reprocess: []; delete: []; 'visible-page': [page: number] }>()

const sections = ref<Section[]>([])
const active = ref<Section | null>(null)
const pages = ref<PageView[]>([])
const loadedEnd = ref(0)
const loadingSections = ref(false)
const loadingPages = ref(false)
const pageError = ref('')
const targeted = ref<number | null>(null)
const jumpTo = ref<number | null>(null)
const previewAll = ref(false)
const scroller = ref<HTMLElement>()
const viewing = ref<{ image: ImageInfo; label: string } | null>(null)
/** Top-most visible unit (for notes taken while reading). */
const currentPage = ref<number | null>(null)
const terms = computed(() => props.terms ?? [])

const jumpable = computed(() => props.document.loc_kind === 'page' || props.document.loc_kind === 'slide')
const compact = computed(() => props.document.loc_kind === 'paragraph' || props.document.loc_kind === 'line')
const index = computed(() => sections.value.findIndex(s => s.id === active.value?.id))
const previous = computed(() => (index.value > 0 ? sections.value[index.value - 1] : null))
const next = computed(() => (index.value >= 0 && index.value < sections.value.length - 1 ? sections.value[index.value + 1] : null))
const hasMore = computed(() => !!active.value && loadedEnd.value < active.value.page_end)

/** Deepest section that contains a unit. */
function sectionFor(page: number) {
  return sections.value
    .filter(s => s.page_start <= page && page <= s.page_end)
    .sort((a, b) => b.level - a.level || a.ordinal - b.ordinal)[0]
}

async function loadSections() {
  loadingSections.value = true
  try {
    sections.value = await DocumentService.sections(props.document.id)
  } catch (e) {
    pageError.value = errorMessage(e)
    sections.value = []
  } finally {
    loadingSections.value = false
  }
}

async function selectSection(section: Section, page?: number) {
  active.value = section
  targeted.value = page ?? null
  pageError.value = ''
  loadingPages.value = true
  // Start the window at the requested page when the section is very long.
  const start = page && page - section.page_start >= WINDOW ? page : section.page_start
  const end = Math.min(section.page_end, start + WINDOW - 1)
  try {
    pages.value = await DocumentService.pages(props.document.id, start, end)
    loadedEnd.value = end
  } catch (e) {
    pageError.value = errorMessage(e)
    pages.value = []
  } finally {
    loadingPages.value = false
  }
  await nextTick()
  if (page) document.getElementById(`unit-${page}`)?.scrollIntoView({ block: 'start' })
  else scroller.value?.scrollTo({ top: 0 })
  observePages()
}

async function loadMore() {
  if (!active.value) return
  const start = loadedEnd.value + 1
  const end = Math.min(active.value.page_end, start + WINDOW - 1)
  try {
    pages.value = [...pages.value, ...(await DocumentService.pages(props.document.id, start, end))]
    loadedEnd.value = end
    await nextTick()
    observePages()
  } catch (e) {
    pageError.value = errorMessage(e)
  }
}

function openPage(page: number | null | undefined) {
  if (!sections.value.length) return
  const target = page ? sectionFor(page) : undefined
  selectSection(target ?? sections.value[0], target ? page ?? undefined : undefined)
}

function jump() {
  if (!jumpTo.value) return
  const page = Math.max(1, Math.min(props.document.page_count, Math.round(jumpTo.value)))
  openPage(page)
}

// Report the top-most visible unit so the reading position can be restored later.
let observer: IntersectionObserver | undefined
let visibleTimer: ReturnType<typeof setTimeout> | undefined
const visibleUnits = new Set<number>()
function observePages() {
  observer?.disconnect()
  visibleUnits.clear()
  if (!scroller.value || !('IntersectionObserver' in window)) return
  observer = new IntersectionObserver((entries) => {
    for (const entry of entries) {
      const number = Number((entry.target as HTMLElement).id.replace('unit-', ''))
      if (entry.isIntersecting) visibleUnits.add(number)
      else visibleUnits.delete(number)
    }
    clearTimeout(visibleTimer)
    visibleTimer = setTimeout(() => {
      if (!visibleUnits.size) return
      currentPage.value = Math.min(...visibleUnits)
      emit('visible-page', currentPage.value)
    }, 800)
  }, { root: scroller.value, threshold: 0.15 })
  scroller.value.querySelectorAll('[id^="unit-"]').forEach(el => observer!.observe(el))
}
onBeforeUnmount(() => {
  observer?.disconnect()
  clearTimeout(visibleTimer)
})

watch(() => [props.document.id, props.document.indexed_at, props.document.status], async () => {
  pages.value = []
  active.value = null
  if (props.document.status !== 'completed') return
  await loadSections()
  openPage(props.page)
}, { immediate: true })

watch(() => props.page, (page) => {
  if (page && sections.value.length && page !== targeted.value) openPage(page)
})
</script>
