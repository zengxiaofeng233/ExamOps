<template>
  <div class="flex h-full min-h-0 flex-col">
    <div class="flex flex-wrap items-center gap-2 border-b border-slate-200 bg-white px-5 py-3">
      <div class="flex flex-wrap gap-1" role="tablist" aria-label="按状态筛选">
        <button
          v-for="item in filters"
          :key="item.value"
          role="tab"
          :aria-selected="filter === item.value"
          class="rounded-full px-3 py-1 text-xs font-medium transition-colors"
          :class="filter === item.value ? 'bg-indigo-600 text-white' : 'bg-slate-100 text-slate-600 hover:bg-slate-200'"
          @click="filter = item.value"
        >
          {{ item.label }} {{ item.count }}
        </button>
      </div>
      <input v-model="keyword" type="search" class="input ml-auto w-48 py-1.5 text-xs" placeholder="搜索知识点" aria-label="搜索知识点" />
      <button class="btn btn-secondary btn-sm" :disabled="generating" title="把识别出的小节标题作为知识点（不需要 AI）" @click="fromHeadings">
        <ListBulletIcon class="size-4" />从章节标题生成
      </button>
      <button class="btn btn-secondary btn-sm" @click="extractOpen = true">
        <SparklesIcon class="size-4" />AI 提取
      </button>
      <button class="btn btn-primary btn-sm" @click="edit(null)">
        <PlusIcon class="size-4" />添加
      </button>
    </div>

    <div class="min-h-0 flex-1 overflow-y-auto px-5 py-4">
      <p v-if="loading" class="text-sm text-slate-400">加载中…</p>
      <EmptyState
        v-else-if="!points.length"
        title="还没有知识点"
        description="知识点是讲解、测验和复习安排的基本单位。可以从章节标题一键生成、让 AI 从某一章提取，或手动添加。"
      >
        <button class="btn btn-primary" :disabled="generating" @click="fromHeadings">从章节标题生成</button>
        <button class="btn btn-secondary" @click="extractOpen = true">AI 提取</button>
      </EmptyState>
      <p v-else-if="!groups.length" class="text-sm text-slate-500">没有符合条件的知识点。</p>
      <div v-else class="mx-auto max-w-4xl space-y-5">
        <section v-for="group in groups" :key="group.key" :aria-label="group.title">
          <h2 class="mb-2 flex items-baseline gap-2">
            <span class="text-sm font-semibold text-slate-800">{{ group.title }}</span>
            <span v-if="group.filename" class="truncate text-xs text-slate-400">{{ group.filename }}</span>
            <span class="text-xs text-slate-400">· {{ group.points.length }} 个</span>
          </h2>
          <ul class="card divide-y divide-slate-100">
            <li v-for="point in group.points" :key="point.id" class="flex items-start gap-3 px-4 py-3">
              <span class="badge mt-0.5 shrink-0" :class="POINT_STATUS_CLASSES[point.status]">{{ POINT_STATUS_LABELS[point.status] }}</span>
              <div class="min-w-0 flex-1">
                <p class="flex flex-wrap items-center gap-1.5">
                  <span class="text-sm font-medium text-slate-900">{{ point.title }}</span>
                  <span v-if="point.importance > 1" class="badge" :class="point.importance === 3 ? 'bg-rose-50 text-rose-700' : 'bg-amber-50 text-amber-700'">{{ IMPORTANCE_LABELS[point.importance] }}</span>
                  <span v-if="point.difficult" class="badge bg-orange-50 text-orange-700"><FlagIcon class="size-3" />疑难</span>
                  <span v-if="point.origin === 'ai'" class="text-[11px] text-slate-400">AI 提取</span>
                </p>
                <p v-if="point.summary" class="mt-0.5 line-clamp-2 text-xs text-slate-600">{{ point.summary }}</p>
                <p class="mt-1 flex flex-wrap gap-x-3 text-[11px] text-slate-400">
                  <RouterLink
                    v-if="point.document_id && point.page_start"
                    :to="{ name: 'documents', params: { id: course.id }, query: { doc: point.document_id, page: point.page_start } }"
                    class="hover:text-indigo-600"
                  >
                    {{ point.section_title && point.section_title !== point.title ? point.section_title : point.filename }}{{ point.loc_label ? ` · ${point.loc_label}` : '' }}
                  </RouterLink>
                  <span v-if="point.learned_at">学过于 {{ formatDateTime(point.learned_at) }}</span>
                </p>
              </div>
              <div class="flex shrink-0 items-center gap-0.5">
                <RouterLink
                  v-if="point.section_id"
                  :to="{ name: 'learn', params: { id: course.id }, query: { mode: 'learn', section: point.section_id } }"
                  class="btn btn-ghost btn-sm"
                  title="让 AI 讲解这一节"
                >
                  <AcademicCapIcon class="size-4" />
                </RouterLink>
                <button
                  class="btn btn-ghost btn-sm"
                  :class="point.difficult ? 'text-orange-600' : ''"
                  :title="point.difficult ? '取消疑难标记' : '标记为疑难'"
                  :aria-pressed="point.difficult"
                  @click="toggleDifficult(point)"
                >
                  <FlagIcon class="size-4" />
                </button>
                <button
                  v-if="point.status === 'new'"
                  class="btn btn-ghost btn-sm"
                  title="标记为学过"
                  @click="setStatus(point, 'learned')"
                >
                  <CheckIcon class="size-4" />
                </button>
                <button
                  v-else-if="point.status === 'learned'"
                  class="btn btn-ghost btn-sm text-sky-600"
                  title="设为未学"
                  @click="setStatus(point, 'new')"
                >
                  <ArrowUturnLeftIcon class="size-4" />
                </button>
                <button class="btn btn-ghost btn-sm" title="编辑" @click="edit(point)"><PencilSquareIcon class="size-4" /></button>
                <button class="btn btn-ghost btn-sm hover:text-rose-600" title="删除" @click="removing = point"><TrashIcon class="size-4" /></button>
              </div>
            </li>
          </ul>
        </section>
        <p class="pb-2 text-center text-xs text-slate-400">“学过”表示已经学习过；“已掌握”只能通过测验获得，两者分开记录。</p>
      </div>
    </div>

    <AppModal :open="!!form" :title="form?.id ? '编辑知识点' : '添加知识点'" @close="form = null">
      <form v-if="form" id="point-form" class="space-y-3" novalidate @submit.prevent="save">
        <div>
          <label class="label" for="point-title">名称</label>
          <input id="point-title" v-model="form.title" class="input" maxlength="60" placeholder="例如：导数的几何意义" />
        </div>
        <div>
          <label class="label" for="point-summary">要点（可选）</label>
          <textarea id="point-summary" v-model="form.summary" class="input" rows="3" maxlength="500" placeholder="用一两句话概括" />
        </div>
        <div>
          <label class="label" for="point-section">所属章节</label>
          <SectionSelect id="point-section" v-model="form.section_id" :sections="sections" />
          <p class="hint">关联章节后，AI 讲解和测验会使用该章节的资料。</p>
        </div>
        <div>
          <label class="label" for="point-importance">重要程度</label>
          <select id="point-importance" v-model.number="form.importance" class="input">
            <option :value="1">一般</option>
            <option :value="2">重要</option>
            <option :value="3">核心考点</option>
          </select>
        </div>
      </form>
      <template #footer>
        <button class="btn btn-secondary" @click="form = null">取消</button>
        <button class="btn btn-primary" type="submit" form="point-form" :disabled="saving || !form?.title.trim()">{{ saving ? '保存中…' : '保存' }}</button>
      </template>
    </AppModal>

    <AppModal :open="extractOpen" title="AI 提取知识点" :persistent="extracting" @close="extractOpen = false">
      <div class="space-y-3">
        <p class="text-sm text-slate-600">AI 会阅读所选章节的资料，整理出可以单独讲解和测验的知识点，并标注重要程度。结果会自动去重，你可以随时修改或删除。</p>
        <div>
          <label class="label" for="extract-section">章节</label>
          <SectionSelect id="extract-section" v-model="extractSection" :sections="sections" placeholder="选择章节" :max-level="2" />
        </div>
        <p v-if="extracting" class="flex items-center gap-2 text-sm text-indigo-700" role="status">
          <ArrowPathIcon class="size-4 animate-spin" />AI 正在阅读本章，通常需要十几秒…
        </p>
        <p v-if="!models.configured && models.loaded" class="text-sm text-amber-700">
          尚未配置 AI 模型。<RouterLink to="/settings?section=models" class="underline">前往设置</RouterLink>
        </p>
      </div>
      <template #footer>
        <button class="btn btn-secondary" :disabled="extracting" @click="extractOpen = false">关闭</button>
        <button class="btn btn-primary" :disabled="extracting || !extractSection || !models.configured" @click="extract">开始提取</button>
      </template>
    </AppModal>

    <ConfirmDialog
      :open="!!removing"
      title="删除知识点"
      :message="`确定删除“${removing?.title}”吗？`"
      confirm-text="删除"
      danger
      @cancel="removing = null"
      @confirm="remove"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import {
  AcademicCapIcon, ArrowPathIcon, ArrowUturnLeftIcon, CheckIcon, FlagIcon, ListBulletIcon, PencilSquareIcon, PlusIcon,
  SparklesIcon, TrashIcon
} from '@heroicons/vue/20/solid'
import SectionSelect from '@/components/study/SectionSelect.vue'
import AppModal from '@/components/ui/AppModal.vue'
import ConfirmDialog from '@/components/ui/ConfirmDialog.vue'
import EmptyState from '@/components/ui/EmptyState.vue'
import { DocumentService } from '@/services/document'
import { KnowledgeService } from '@/services/study'
import { useModelStore } from '@/stores/models'
import { useToastStore } from '@/stores/toast'
import type { CourseSection } from '@/types/document'
import type { Course } from '@/types/models'
import type { KnowledgeInput, KnowledgePoint } from '@/types/study'
import { formatDateTime } from '@/utils/format'
import { IMPORTANCE_LABELS, POINT_STATUS_CLASSES, POINT_STATUS_LABELS } from '@/utils/study'

const props = defineProps<{ course: Course }>()
const toast = useToastStore()
const models = useModelStore()

const points = ref<KnowledgePoint[]>([])
const sections = ref<CourseSection[]>([])
const loading = ref(true)
const generating = ref(false)
const saving = ref(false)
const extractOpen = ref(false)
const extracting = ref(false)
const extractSection = ref<number | null>(null)
const removing = ref<KnowledgePoint | null>(null)
const form = ref<KnowledgeInput | null>(null)
const filter = ref<'all' | 'new' | 'learned' | 'mastered' | 'difficult'>('all')
const keyword = ref('')

const filters = computed(() => [
  { value: 'all' as const, label: '全部', count: points.value.length },
  { value: 'new' as const, label: '未学', count: points.value.filter(p => p.status === 'new').length },
  { value: 'learned' as const, label: '学过', count: points.value.filter(p => p.status === 'learned').length },
  { value: 'mastered' as const, label: '已掌握', count: points.value.filter(p => p.status === 'mastered').length },
  { value: 'difficult' as const, label: '疑难', count: points.value.filter(p => p.difficult).length }
])

const sectionById = computed(() => new Map(sections.value.map(s => [s.id, s])))

/** The chapter (top-level section) a section belongs to. */
function chapterOf(sectionId: number | null) {
  let section = sectionId ? sectionById.value.get(sectionId) : undefined
  while (section?.parent_id && sectionById.value.get(section.parent_id)) section = sectionById.value.get(section.parent_id)
  return section ?? null
}

const groups = computed(() => {
  const text = keyword.value.trim().toLowerCase()
  const visible = points.value.filter((p) => {
    if (filter.value === 'difficult' ? !p.difficult : filter.value !== 'all' && p.status !== filter.value) return false
    return !text || p.title.toLowerCase().includes(text) || p.summary.toLowerCase().includes(text)
  })
  const map = new Map<string, { key: string; title: string; filename: string; points: KnowledgePoint[] }>()
  for (const point of visible) {
    const chapter = chapterOf(point.section_id)
    const key = chapter ? `s${chapter.id}` : point.section_title ? `t${point.section_title}` : 'none'
    if (!map.has(key)) {
      map.set(key, { key, title: chapter?.title ?? point.section_title ?? '未关联章节', filename: chapter?.filename ?? point.filename ?? '', points: [] })
    }
    map.get(key)!.points.push(point)
  }
  return [...map.values()]
})

async function refresh() {
  points.value = await KnowledgeService.list(props.course.id)
}

onMounted(async () => {
  models.load().catch(() => undefined)
  try {
    const [, list] = await Promise.all([refresh(), DocumentService.courseSections(props.course.id)])
    sections.value = list
  } catch (e) {
    toast.error(e)
  } finally {
    loading.value = false
  }
})

function replace(point: KnowledgePoint) {
  const index = points.value.findIndex(p => p.id === point.id)
  if (index >= 0) points.value[index] = point
}

async function fromHeadings() {
  generating.value = true
  try {
    const created = await KnowledgeService.fromHeadings(props.course.id)
    await refresh()
    if (created) toast.success(`已从章节标题生成 ${created} 个知识点`)
    else toast.info(sections.value.length ? '没有新的章节标题可以生成（已存在的不会重复添加）' : '还没有已解析的章节，请先导入资料')
  } catch (e) {
    toast.error(e)
  } finally {
    generating.value = false
  }
}

async function extract() {
  if (!extractSection.value) return
  extracting.value = true
  try {
    const result = await KnowledgeService.extract(props.course.id, extractSection.value)
    await refresh()
    extractOpen.value = false
    const parts = [`新增 ${result.created.length} 个知识点`]
    if (result.skipped) parts.push(`${result.skipped} 个重复或无效的已跳过`)
    if (result.truncated) parts.push('章节较长，只分析了前面部分')
    toast.success(parts.join('，'))
  } catch (e) {
    toast.error(e)
  } finally {
    extracting.value = false
  }
}

function edit(point: KnowledgePoint | null) {
  form.value = point
    ? { id: point.id, course_id: props.course.id, section_id: point.section_id, title: point.title, summary: point.summary, importance: point.importance }
    : { id: null, course_id: props.course.id, section_id: null, title: '', summary: '', importance: 1 }
}

async function save() {
  if (!form.value || !form.value.title.trim()) return
  saving.value = true
  try {
    await KnowledgeService.save(form.value)
    form.value = null
    await refresh()
  } catch (e) {
    toast.error(e)
  } finally {
    saving.value = false
  }
}

async function toggleDifficult(point: KnowledgePoint) {
  try {
    replace(await KnowledgeService.setDifficult(point.id, !point.difficult))
  } catch (e) {
    toast.error(e)
  }
}

async function setStatus(point: KnowledgePoint, status: 'new' | 'learned') {
  try {
    replace(await KnowledgeService.setStatus(point.id, status))
  } catch (e) {
    toast.error(e)
  }
}

async function remove() {
  if (!removing.value) return
  try {
    await KnowledgeService.remove(removing.value.id)
    points.value = points.value.filter(p => p.id !== removing.value?.id)
    removing.value = null
  } catch (e) {
    toast.error(e)
  }
}
</script>
