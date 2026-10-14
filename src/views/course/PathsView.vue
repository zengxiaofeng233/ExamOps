<template>
  <div class="flex h-full min-h-0">
    <aside class="flex w-72 shrink-0 flex-col border-r border-slate-200 bg-white">
      <div class="border-b border-slate-100 p-3">
        <button class="btn btn-primary w-full" @click="creating = true"><PlusIcon class="size-4" />新建学习路径</button>
      </div>
      <div class="min-h-0 flex-1 overflow-y-auto p-2">
        <p v-if="loading" class="p-3 text-sm text-slate-400">加载中…</p>
        <p v-else-if="!paths.length" class="p-3 text-sm text-slate-500">还没有学习路径。可以按章节自动生成、让 AI 推荐，或从空白开始自己安排。</p>
        <ul v-else class="space-y-1" aria-label="学习路径">
          <li v-for="item in paths" :key="item.id">
            <button
              class="block w-full rounded-lg px-3 py-2 text-left"
              :class="[item.id === selectedId ? 'bg-indigo-50' : 'hover:bg-slate-50', item.archived ? 'opacity-60' : '']"
              @click="select(item.id)"
            >
              <p class="flex items-center gap-1.5">
                <span class="badge shrink-0" :class="MODE_CLASSES[item.mode]">{{ MODE_LABELS[item.mode] }}</span>
                <span class="truncate text-sm font-medium text-slate-800">{{ item.name }}</span>
              </p>
              <div class="mt-2 h-1.5 overflow-hidden rounded-full bg-slate-100" aria-hidden="true">
                <div class="h-full rounded-full bg-emerald-500" :style="{ width: `${pathPercent(item)}%` }" />
              </div>
              <p class="mt-1 text-[11px] text-slate-400">
                {{ item.done_count }}/{{ item.step_count }} 已完成
                <span v-if="item.id === activePathId" class="text-indigo-600">· 当前路径</span>
                <span v-if="item.archived">· 已归档</span>
              </p>
            </button>
          </li>
        </ul>
      </div>
    </aside>

    <section class="min-w-0 flex-1 overflow-y-auto">
      <div v-if="detail" class="mx-auto max-w-4xl space-y-4 p-5">
        <header class="card p-5">
          <div class="flex flex-wrap items-start gap-3">
            <div class="min-w-40 flex-1">
              <p class="flex flex-wrap items-center gap-2">
                <span class="badge" :class="MODE_CLASSES[detail.path.mode]">{{ MODE_LABELS[detail.path.mode] }}</span>
                <span class="text-xs text-slate-400">{{ ORIGIN_LABELS[detail.path.origin] ?? '' }}</span>
                <span v-if="detail.path.id === activePathId" class="badge bg-indigo-600 text-white">当前路径</span>
              </p>
              <h2 class="mt-1.5 text-lg font-semibold text-slate-900">{{ detail.path.name }}</h2>
              <p v-if="detail.path.description" class="mt-0.5 text-sm text-slate-500">{{ detail.path.description }}</p>
            </div>
            <div class="flex shrink-0 flex-wrap gap-1.5">
              <button v-if="detail.path.id !== activePathId && !detail.path.archived" class="btn btn-soft btn-sm" @click="activate">设为当前路径</button>
              <button class="btn btn-secondary btn-sm" @click="editPath"><PencilSquareIcon class="size-4" />编辑</button>
              <button class="btn btn-ghost btn-sm" @click="toggleArchive">{{ detail.path.archived ? '取消归档' : '归档' }}</button>
              <button class="btn btn-ghost btn-sm text-rose-600 hover:text-rose-700" @click="confirmDelete = true">删除</button>
            </div>
          </div>
          <div class="mt-4 flex items-center gap-3">
            <div class="h-2 flex-1 overflow-hidden rounded-full bg-slate-100" role="progressbar" :aria-valuenow="pathPercent(detail.path)" aria-valuemin="0" aria-valuemax="100">
              <div class="h-full rounded-full bg-emerald-500 transition-all" :style="{ width: `${pathPercent(detail.path)}%` }" />
            </div>
            <span class="text-xs text-slate-500">完成 {{ detail.path.done_count }}/{{ detail.path.step_count }}<template v-if="detail.path.skipped_count">，跳过 {{ detail.path.skipped_count }}</template><template v-if="remainingMinutes">，剩余约 {{ remainingMinutes }} 分钟</template></span>
          </div>
        </header>

        <EmptyState v-if="!detail.steps.length" title="还没有步骤" description="添加阅读、网课、AI 讲解、小测、回忆等步骤，按你自己的节奏安排。">
          <button class="btn btn-primary" @click="addStep(null)">添加第一步</button>
        </EmptyState>

        <ol v-else class="space-y-2" aria-label="学习步骤">
          <li
            v-for="(step, index) in detail.steps"
            :key="step.id"
            class="card flex items-start gap-3 p-3.5 transition-shadow"
            :class="[
              step.id === nextStepId ? 'ring-2 ring-indigo-300' : '',
              dragOver === step.id ? 'ring-2 ring-indigo-500' : '',
              step.status === 'done' || step.status === 'skipped' ? 'bg-slate-50/70' : ''
            ]"
            draggable="true"
            @dragstart="onDragStart(step.id, $event)"
            @dragover.prevent="dragOver = step.id"
            @dragleave="dragOver = dragOver === step.id ? null : dragOver"
            @drop.prevent="onDrop(step.id)"
            @dragend="dragging = null; dragOver = null"
          >
            <div class="flex shrink-0 flex-col items-center gap-0.5 pt-0.5">
              <Bars2Icon class="size-4 cursor-grab text-slate-300" aria-hidden="true" />
              <span class="flex size-6 items-center justify-center rounded-full text-xs font-semibold" :class="step.status === 'done' ? 'bg-emerald-100 text-emerald-700' : 'bg-slate-100 text-slate-600'">
                <CheckIcon v-if="step.status === 'done'" class="size-3.5" />
                <template v-else>{{ index + 1 }}</template>
              </span>
            </div>
            <div class="min-w-0 flex-1">
              <p class="flex flex-wrap items-center gap-1.5">
                <span class="badge bg-slate-100 text-slate-600">{{ step.kind_label }}</span>
                <span class="text-sm font-medium" :class="step.status === 'skipped' ? 'text-slate-400 line-through' : 'text-slate-900'">{{ step.title }}</span>
                <span class="badge" :class="STEP_STATUS_CLASSES[step.status]">{{ STEP_STATUS_LABELS[step.status] }}</span>
              </p>
              <p v-if="step.instructions" class="mt-1 text-xs whitespace-pre-wrap text-slate-600">{{ step.instructions }}</p>
              <p class="mt-1 flex flex-wrap gap-x-3 text-[11px] text-slate-400">
                <span v-if="step.document_name">{{ step.document_name }}</span>
                <span v-if="step.section_title && !step.title.includes(step.section_title)">{{ step.section_title }}</span>
                <span v-if="step.resource_title && !step.title.includes(step.resource_title)">网课：{{ step.resource_title }}</span>
                <span v-if="step.knowledge_point_ids.length">{{ step.knowledge_point_ids.length }} 个知识点</span>
                <span v-if="step.estimated_minutes">约 {{ step.estimated_minutes }} 分钟</span>
                <span v-if="step.completed_at && step.status === 'done'">完成于 {{ formatDateTime(step.completed_at) }}</span>
              </p>
              <div class="mt-2 flex flex-wrap gap-1">
                <template v-if="step.status !== 'done' && step.status !== 'skipped'">
                  <button v-if="canOpen(step)" class="btn btn-primary btn-sm" @click="openStep(step)">
                    <PlayIcon class="size-3.5" />{{ step.status === 'in_progress' ? '继续' : '开始' }}
                  </button>
                  <button class="btn btn-secondary btn-sm" @click="setStatus(step, 'done')"><CheckIcon class="size-3.5" />完成</button>
                  <button class="btn btn-ghost btn-sm" @click="setStatus(step, 'skipped')">跳过</button>
                </template>
                <button v-else class="btn btn-ghost btn-sm" @click="setStatus(step, 'todo')"><ArrowUturnLeftIcon class="size-3.5" />恢复为待完成</button>
              </div>
            </div>
            <div class="flex shrink-0 flex-col gap-0.5">
              <div class="flex">
                <button class="btn btn-ghost btn-sm px-1.5" :disabled="index === 0 || reordering" title="上移" @click="move(index, -1)"><ChevronUpIcon class="size-4" /></button>
                <button class="btn btn-ghost btn-sm px-1.5" :disabled="index === detail.steps.length - 1 || reordering" title="下移" @click="move(index, 1)"><ChevronDownIcon class="size-4" /></button>
              </div>
              <div class="flex">
                <button class="btn btn-ghost btn-sm px-1.5" title="编辑" @click="editStep(step)"><PencilSquareIcon class="size-4" /></button>
                <button class="btn btn-ghost btn-sm px-1.5 hover:text-rose-600" title="删除" @click="removingStep = step"><TrashIcon class="size-4" /></button>
              </div>
              <button class="btn btn-ghost btn-sm px-1.5 text-xs" title="在这一步之后插入" @click="addStep(step.id)"><PlusIcon class="size-4" /></button>
            </div>
          </li>
        </ol>
        <button v-if="detail.steps.length" class="btn btn-secondary w-full" @click="addStep(null)"><PlusIcon class="size-4" />添加步骤</button>
        <p v-if="detail.steps.length > 1" class="text-center text-xs text-slate-400">拖动步骤或使用上下箭头调整顺序</p>
      </div>
      <EmptyState v-else-if="!loading" title="选择或新建一条学习路径" description="学习路径由你来定：可以按章节、按网课、按考点，随时调整顺序和内容。">
        <button class="btn btn-primary" @click="creating = true">新建学习路径</button>
      </EmptyState>
    </section>

    <PathCreateDialog :open="creating" :course-id="course.id" :documents="documents" @close="creating = false" @created="onCreated" />
    <StepEditor
      v-if="detail"
      :open="stepEditor.open"
      :path-id="detail.path.id"
      :step="stepEditor.step"
      :after-step-id="stepEditor.after"
      :sections="sections"
      :documents="documents"
      :resources="resources"
      :points="points"
      @close="stepEditor.open = false"
      @saved="onStepSaved"
    />
    <AppModal :open="!!pathForm" title="编辑学习路径" @close="pathForm = null">
      <form v-if="pathForm" id="path-edit" class="space-y-3" novalidate @submit.prevent="savePath">
        <div>
          <label class="label" for="edit-path-name">名称</label>
          <input id="edit-path-name" v-model="pathForm.name" class="input" maxlength="40" />
        </div>
        <div>
          <label class="label" for="edit-path-mode">模式</label>
          <select id="edit-path-mode" v-model="pathForm.mode" class="input">
            <option v-for="(label, value) in PATH_MODE_LABELS" :key="value" :value="value">{{ label }}</option>
          </select>
        </div>
        <div>
          <label class="label" for="edit-path-description">说明</label>
          <textarea id="edit-path-description" v-model="pathForm.description" class="input" rows="2" maxlength="200" />
        </div>
      </form>
      <template #footer>
        <button class="btn btn-secondary" @click="pathForm = null">取消</button>
        <button class="btn btn-primary" type="submit" form="path-edit" :disabled="!pathForm?.name.trim()">保存</button>
      </template>
    </AppModal>
    <ConfirmDialog
      :open="confirmDelete"
      title="删除学习路径"
      :message="`确定删除“${detail?.path.name}”及其全部步骤吗？知识点的学习状态不受影响。`"
      confirm-text="删除"
      danger
      @cancel="confirmDelete = false"
      @confirm="removePath"
    />
    <ConfirmDialog
      :open="!!removingStep"
      title="删除步骤"
      :message="`确定删除步骤“${removingStep?.title}”吗？`"
      confirm-text="删除"
      danger
      @cancel="removingStep = null"
      @confirm="removeStep"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import {
  ArrowUturnLeftIcon, Bars2Icon, CheckIcon, ChevronDownIcon, ChevronUpIcon, PencilSquareIcon, PlayIcon, PlusIcon, TrashIcon
} from '@heroicons/vue/20/solid'
import PathCreateDialog from '@/components/study/PathCreateDialog.vue'
import StepEditor from '@/components/study/StepEditor.vue'
import AppModal from '@/components/ui/AppModal.vue'
import ConfirmDialog from '@/components/ui/ConfirmDialog.vue'
import EmptyState from '@/components/ui/EmptyState.vue'
import { DocumentService } from '@/services/document'
import { KnowledgeService, PathService, ResourceService, StudyService } from '@/services/study'
import { useToastStore } from '@/stores/toast'
import type { CourseSection, Document } from '@/types/document'
import type { Course } from '@/types/models'
import type { KnowledgePoint, LearningPath, PathDetail, PathInput, PathMode, PathStep, Resource, StepStatus } from '@/types/study'
import { formatDateTime } from '@/utils/format'
import { MODE_CLASSES, MODE_LABELS, STEP_STATUS_CLASSES, STEP_STATUS_LABELS, stepRoute } from '@/utils/study'

const ORIGIN_LABELS: Record<string, string> = { manual: '手动创建', template: '按章节生成', ai: 'AI 推荐' }
const PATH_MODE_LABELS: Record<PathMode, string> = { learn: '从零学习', review: '正常复习', sprint: '考前冲刺' }

const props = defineProps<{ course: Course }>()
const route = useRoute()
const router = useRouter()
const toast = useToastStore()

const paths = ref<LearningPath[]>([])
const detail = ref<PathDetail | null>(null)
const sections = ref<CourseSection[]>([])
const documents = ref<Document[]>([])
const resources = ref<Resource[]>([])
const points = ref<KnowledgePoint[]>([])
const activePathId = ref<number | null>(null)
const loading = ref(true)
const creating = ref(false)
const reordering = ref(false)
const confirmDelete = ref(false)
const removingStep = ref<PathStep | null>(null)
const pathForm = ref<PathInput | null>(null)
const stepEditor = reactive<{ open: boolean; step: PathStep | null; after: number | null }>({ open: false, step: null, after: null })
const dragging = ref<number | null>(null)
const dragOver = ref<number | null>(null)

const selectedId = computed(() => Number(route.query.p) || null)
const nextStepId = computed(() => detail.value?.steps.find(s => s.status === 'todo' || s.status === 'in_progress')?.id ?? null)
const remainingMinutes = computed(() => detail.value?.steps
  .filter(s => s.status === 'todo' || s.status === 'in_progress')
  .reduce((sum, s) => sum + (s.estimated_minutes ?? 0), 0) ?? 0)

function pathPercent(path: LearningPath) {
  return path.step_count ? Math.round(((path.done_count + path.skipped_count) / path.step_count) * 100) : 0
}

async function refreshList() {
  const [list, state] = await Promise.all([PathService.list(props.course.id), StudyService.state(props.course.id)])
  paths.value = list
  activePathId.value = state.active_path?.id ?? null
}

async function loadDetail() {
  detail.value = selectedId.value ? await PathService.get(selectedId.value).catch((e) => { toast.error(e); return null }) : null
}

function select(id: number) {
  router.replace({ query: { p: id } })
}

watch(selectedId, loadDetail)

onMounted(async () => {
  try {
    const [, sectionList, documentList, resourceList, pointList] = await Promise.all([
      refreshList(),
      DocumentService.courseSections(props.course.id),
      DocumentService.list(props.course.id),
      ResourceService.list(props.course.id),
      KnowledgeService.list(props.course.id)
    ])
    sections.value = sectionList
    documents.value = documentList.filter(d => d.status === 'completed')
    resources.value = resourceList
    points.value = pointList
    if (!selectedId.value && paths.value.length) select(activePathId.value ?? paths.value[0].id)
    else await loadDetail()
  } catch (e) {
    toast.error(e)
  } finally {
    loading.value = false
  }
})

async function reloadAll() {
  await Promise.all([refreshList(), loadDetail()])
}

async function onCreated(id: number) {
  creating.value = false
  await refreshList()
  select(id)
}

function canOpen(step: PathStep) {
  const target = stepRoute(step, props.course.id, detail.value?.path.mode ?? 'learn')
  if (!target) return false
  const name = typeof target === 'object' && 'name' in target ? target.name : undefined
  return !name || router.hasRoute(name)
}

async function openStep(step: PathStep) {
  const target = stepRoute(step, props.course.id, detail.value?.path.mode ?? 'learn')
  if (!target) return
  try {
    if (step.status === 'todo') await PathService.setStatus(step.id, 'in_progress')
    await router.push(target)
  } catch (e) {
    toast.error(e)
  }
}

async function setStatus(step: PathStep, status: StepStatus) {
  try {
    await PathService.setStatus(step.id, status)
    await reloadAll()
    if (status === 'done') toast.success('已完成这一步')
  } catch (e) {
    toast.error(e)
  }
}

async function applyOrder(ids: number[]) {
  if (!detail.value) return
  reordering.value = true
  try {
    const steps = await PathService.reorder(detail.value.path.id, ids)
    detail.value = { ...detail.value, steps }
  } catch (e) {
    toast.error(e)
    await loadDetail()
  } finally {
    reordering.value = false
  }
}

function move(index: number, delta: number) {
  if (!detail.value) return
  const ids = detail.value.steps.map(s => s.id)
  const [id] = ids.splice(index, 1)
  ids.splice(index + delta, 0, id)
  applyOrder(ids)
}

function onDragStart(id: number, event: DragEvent) {
  dragging.value = id
  event.dataTransfer?.setData('text/plain', String(id))
  if (event.dataTransfer) event.dataTransfer.effectAllowed = 'move'
}

function onDrop(targetId: number) {
  const source = dragging.value
  dragging.value = null
  dragOver.value = null
  if (!detail.value || source === null || source === targetId) return
  const ids = detail.value.steps.map(s => s.id)
  const from = ids.indexOf(source)
  const to = ids.indexOf(targetId)
  ids.splice(from, 1)
  ids.splice(to, 0, source)
  applyOrder(ids)
}

function addStep(after: number | null) {
  stepEditor.step = null
  stepEditor.after = after
  stepEditor.open = true
}

function editStep(step: PathStep) {
  stepEditor.step = step
  stepEditor.after = null
  stepEditor.open = true
}

async function onStepSaved() {
  stepEditor.open = false
  await reloadAll()
}

async function removeStep() {
  if (!removingStep.value) return
  try {
    await PathService.removeStep(removingStep.value.id)
    removingStep.value = null
    await reloadAll()
  } catch (e) {
    toast.error(e)
  }
}

async function activate() {
  if (!detail.value) return
  try {
    const state = await PathService.activate(props.course.id, detail.value.path.id)
    activePathId.value = state.active_path?.id ?? null
    toast.success('已设为当前学习路径，概览页会显示下一步')
  } catch (e) {
    toast.error(e)
  }
}

function editPath() {
  if (!detail.value) return
  const path = detail.value.path
  pathForm.value = { id: path.id, course_id: props.course.id, name: path.name, description: path.description, mode: path.mode }
}

async function savePath() {
  if (!pathForm.value) return
  try {
    await PathService.save(pathForm.value)
    pathForm.value = null
    await reloadAll()
  } catch (e) {
    toast.error(e)
  }
}

async function toggleArchive() {
  if (!detail.value) return
  const path = detail.value.path
  try {
    await PathService.save({ id: path.id, course_id: props.course.id, name: path.name, description: path.description, mode: path.mode, archived: !path.archived })
    await reloadAll()
  } catch (e) {
    toast.error(e)
  }
}

async function removePath() {
  if (!detail.value) return
  try {
    await PathService.remove(detail.value.path.id)
    confirmDelete.value = false
    await refreshList()
    router.replace({ query: paths.value[0] ? { p: paths.value[0].id } : {} })
  } catch (e) {
    toast.error(e)
  }
}
</script>
