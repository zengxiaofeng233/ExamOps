<template>
  <form class="card space-y-4 p-5" novalidate @submit.prevent="create">
    <div>
      <h2 class="section-title">新建小测</h2>
      <p class="mt-0.5 text-sm text-slate-500">AI 根据你的资料出题，结果会记入知识点掌握度并安排复习。</p>
    </div>

    <div>
      <p class="label">出题范围</p>
      <div class="grid grid-cols-2 gap-2 sm:grid-cols-4" role="radiogroup" aria-label="出题范围">
        <label
          v-for="item in scopes"
          :key="item.value"
          class="cursor-pointer rounded-lg border p-2.5 text-sm"
          :class="[scope === item.value ? 'border-indigo-400 bg-indigo-50/60' : 'border-slate-200 hover:bg-slate-50', item.disabled ? 'cursor-not-allowed opacity-50' : '']"
        >
          <input v-model="scope" type="radio" :value="item.value" class="sr-only" :disabled="item.disabled" />
          <span class="font-medium text-slate-800">{{ item.label }}</span>
          <span class="mt-0.5 block text-xs text-slate-500">{{ item.hint }}</span>
        </label>
      </div>
    </div>

    <div v-if="scope === 'section'">
      <label class="label" for="quiz-section">章节</label>
      <SectionSelect id="quiz-section" v-model="sectionId" :sections="sections" placeholder="选择章节" :max-level="2" />
    </div>
    <div v-else-if="scope === 'points'">
      <div class="mb-1.5 flex items-center justify-between gap-2">
        <p class="label mb-0">知识点（已选 {{ pointIds.length }}）</p>
        <input v-model="pointFilter" type="search" class="input w-40 py-1 text-xs" placeholder="筛选" aria-label="筛选知识点" />
      </div>
      <div class="max-h-48 overflow-y-auto rounded-lg border border-slate-200 p-2">
        <label v-for="point in filteredPoints" :key="point.id" class="flex items-center gap-2 rounded px-1.5 py-1 text-sm hover:bg-slate-50">
          <input v-model="pointIds" type="checkbox" :value="point.id" class="accent-indigo-600" />
          <span class="min-w-0 flex-1 truncate">{{ point.title }}</span>
          <MasteryBar :score="point.mastery" compact />
        </label>
        <p v-if="!filteredPoints.length" class="p-2 text-xs text-slate-400">没有知识点</p>
      </div>
    </div>
    <p v-else-if="scope === 'review'" class="rounded-lg bg-slate-50 p-3 text-sm text-slate-600">
      从复习队列中选取到期、薄弱和学过未测的知识点（最多 8 个）。
    </p>
    <p v-else class="rounded-lg bg-slate-50 p-3 text-sm text-slate-600">
      针对错题本中未解决的错题出变式题；某个知识点答得好，相关错题会自动标记为已解决。
    </p>

    <div class="grid gap-4 sm:grid-cols-[8rem_1fr]">
      <div>
        <label class="label" for="quiz-count">题目数量</label>
        <input id="quiz-count" v-model.number="count" type="number" min="1" max="20" step="1" class="input" />
      </div>
      <div>
        <p class="label">题型</p>
        <div class="flex flex-wrap gap-x-4 gap-y-1.5">
          <label v-for="(label, kind) in QUESTION_KIND_LABELS" :key="kind" class="flex items-center gap-1.5 text-sm text-slate-700" :title="QUESTION_KIND_HINTS[kind]">
            <input v-model="kinds" type="checkbox" :value="kind" class="accent-indigo-600" />{{ label }}
          </label>
        </div>
      </div>
    </div>
    <label class="flex items-center gap-2 text-sm text-slate-700">
      <input v-model="allowSkip" type="checkbox" class="accent-indigo-600" />允许跳过题目（跳过的题按答错计入）
    </label>

    <p v-if="!models.configured && models.loaded" class="text-sm text-amber-700">
      尚未配置 AI 模型。<RouterLink to="/settings?section=models" class="underline">前往设置</RouterLink>
    </p>
    <p v-if="busy" class="flex items-center gap-2 text-sm text-indigo-700" role="status">
      <ArrowPathIcon class="size-4 animate-spin" />AI 正在阅读资料并出题，通常需要十几秒到一分钟…
    </p>
    <div class="flex justify-end">
      <button type="submit" class="btn btn-primary" :disabled="!canCreate">
        <SparklesIcon class="size-4" />{{ busy ? '出题中…' : '生成小测' }}
      </button>
    </div>
  </form>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { ArrowPathIcon, SparklesIcon } from '@heroicons/vue/20/solid'
import MasteryBar from '@/components/study/MasteryBar.vue'
import SectionSelect from '@/components/study/SectionSelect.vue'
import { useModelStore } from '@/stores/models'
import type { QuestionKind, QuizSettings } from '@/types/app'
import type { CreateQuizInput } from '@/types/assessment'
import type { CourseSection } from '@/types/document'
import type { KnowledgePoint } from '@/types/study'
import { QUESTION_KIND_HINTS, QUESTION_KIND_LABELS } from '@/utils/assessment'

type Scope = 'section' | 'points' | 'review' | 'mistakes'

const props = defineProps<{
  courseId: number
  sections: CourseSection[]
  points: KnowledgePoint[]
  defaults: QuizSettings
  busy?: boolean
  openMistakes?: number
  initial?: { scope?: Scope; sectionId?: number | null; pointIds?: number[] }
}>()
const emit = defineEmits<{ create: [input: CreateQuizInput] }>()
const models = useModelStore()

const scope = ref<Scope>(props.initial?.scope ?? 'section')
const sectionId = ref<number | null>(props.initial?.sectionId ?? null)
const pointIds = ref<number[]>(props.initial?.pointIds ?? [])
const pointFilter = ref('')
const count = ref(props.defaults.question_count)
const kinds = ref<QuestionKind[]>([...props.defaults.kinds])
const allowSkip = ref(props.defaults.allow_skip)

watch(() => props.defaults, (value) => {
  count.value = value.question_count
  kinds.value = [...value.kinds]
  allowSkip.value = value.allow_skip
})
watch(() => props.initial, (value) => {
  if (!value) return
  if (value.scope) scope.value = value.scope
  if (value.sectionId !== undefined) sectionId.value = value.sectionId
  if (value.pointIds) pointIds.value = [...value.pointIds]
})

const scopes = computed(() => [
  { value: 'section' as const, label: '按章节', hint: '整章的知识点', disabled: !props.sections.length },
  { value: 'points' as const, label: '选知识点', hint: '自己挑选', disabled: !props.points.length },
  { value: 'review' as const, label: '待复习', hint: '到期和薄弱的', disabled: false },
  { value: 'mistakes' as const, label: '错题重练', hint: `${props.openMistakes ?? 0} 道未解决`, disabled: !props.openMistakes }
])

const filteredPoints = computed(() => {
  const text = pointFilter.value.trim().toLowerCase()
  return text ? props.points.filter(p => p.title.toLowerCase().includes(text) || pointIds.value.includes(p.id)) : props.points
})

const canCreate = computed(() => {
  if (props.busy || !models.configured || !kinds.value.length || !(count.value >= 1 && count.value <= 20)) return false
  if (scope.value === 'section') return !!sectionId.value
  if (scope.value === 'points') return pointIds.value.length > 0
  return true
})

function create() {
  if (!canCreate.value) return
  const input: CreateQuizInput = {
    course_id: props.courseId,
    question_count: Math.round(count.value),
    kinds: kinds.value,
    allow_skip: allowSkip.value
  }
  if (scope.value === 'section') Object.assign(input, { origin: 'chapter', section_id: sectionId.value })
  else if (scope.value === 'points') Object.assign(input, { origin: 'manual', knowledge_point_ids: pointIds.value })
  else input.origin = scope.value
  emit('create', input)
}
</script>
