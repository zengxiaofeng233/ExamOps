<template>
  <div class="flex h-full min-h-0">
    <aside class="flex w-72 shrink-0 flex-col border-r border-slate-200 bg-surface">
      <div class="border-b border-slate-100 p-3">
        <button class="btn btn-primary w-full" :disabled="working" @click="openCreator()"><PlusIcon class="size-4" />新建小测</button>
      </div>
      <div class="min-h-0 flex-1 overflow-y-auto p-2">
        <p v-if="loading" class="p-3 text-sm text-slate-400">加载中…</p>
        <p v-else-if="!quizzes.length" class="p-3 text-sm text-slate-500">还没有做过小测。</p>
        <ul v-else class="space-y-1" aria-label="小测记录">
          <li v-for="item in quizzes" :key="item.id" class="group relative">
            <button
              class="block w-full rounded-lg px-3 py-2 pr-8 text-left"
              :class="item.id === activeId ? 'bg-indigo-50' : 'hover:bg-slate-50'"
              @click="select(item.id)"
            >
              <p class="truncate text-sm font-medium text-slate-800" :title="item.title">{{ item.title }}</p>
              <p class="mt-0.5 flex items-center gap-2 text-xs">
                <span v-if="item.status === 'graded'" class="font-semibold tabular-nums" :class="TONE_TEXT[scoreTone((item.score ?? 0) / 100)]">{{ item.score }}%</span>
                <span v-else-if="item.status === 'grading'" class="text-amber-600">待批改</span>
                <span v-else class="text-indigo-600">进行中 {{ item.answered_count }}/{{ item.question_count }}</span>
                <span class="text-slate-400">{{ item.question_count }} 题 · {{ formatRelative(item.created_at) }}</span>
              </p>
            </button>
            <button
              class="absolute top-2 right-1.5 hidden rounded p-1 text-slate-400 group-hover:block hover:bg-surface hover:text-rose-600"
              title="删除小测"
              @click="removing = item"
            >
              <TrashIcon class="size-4" />
            </button>
          </li>
        </ul>
      </div>
    </aside>

    <section class="min-w-0 flex-1 overflow-y-auto">
      <div v-if="working" class="flex h-full flex-col items-center justify-center gap-3 p-8 text-center" role="status">
        <ArrowPathIcon class="size-8 animate-spin text-indigo-500" />
        <p class="text-sm font-medium text-slate-800">{{ workingText }}</p>
        <p class="max-w-sm text-xs text-slate-500">题目只依据你导入的资料生成。请不要关闭窗口，通常需要十几秒到一分钟。</p>
      </div>
      <template v-else-if="detail">
        <QuizRunner v-if="detail.quiz.status === 'ready'" :key="detail.quiz.id" :detail="detail" :busy="working" @submit="submit" />
        <QuizResult v-else :detail="detail" :busy="grading" @regrade="regrade" @self-grade="selfGrade" @retry="retryPoints" />
      </template>
      <div v-else class="mx-auto max-w-3xl p-5">
        <QuizCreator
          :course-id="course.id"
          :sections="sections"
          :points="points"
          :defaults="app.settings.quiz"
          :open-mistakes="openMistakes"
          :initial="initial"
          @create="create"
        />
      </div>
    </section>

    <ConfirmDialog
      :open="!!removing"
      title="删除小测"
      :message="`确定删除“${removing?.title}”吗？已经计入的掌握度和复习计划不会撤销。`"
      confirm-text="删除"
      danger
      @cancel="removing = null"
      @confirm="remove"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { onBeforeRouteLeave, useRoute, useRouter } from 'vue-router'
import { ArrowPathIcon, PlusIcon, TrashIcon } from '@heroicons/vue/20/solid'
import QuizCreator from '@/components/quiz/QuizCreator.vue'
import QuizResult from '@/components/quiz/QuizResult.vue'
import QuizRunner from '@/components/quiz/QuizRunner.vue'
import ConfirmDialog from '@/components/ui/ConfirmDialog.vue'
import { QuizService, ReviewService } from '@/services/assessment'
import { DocumentService } from '@/services/document'
import { KnowledgeService } from '@/services/study'
import { useAppStore } from '@/stores/app'
import { useModelStore } from '@/stores/models'
import { useToastStore } from '@/stores/toast'
import type { CreateQuizInput, Quiz, QuizDetail } from '@/types/assessment'
import type { CourseSection } from '@/types/document'
import type { Course } from '@/types/models'
import type { KnowledgePoint } from '@/types/study'
import { scoreTone, TONE_TEXT } from '@/utils/assessment'
import { formatRelative } from '@/utils/format'

const props = defineProps<{ course: Course }>()
const route = useRoute()
const router = useRouter()
const toast = useToastStore()
const app = useAppStore()
const models = useModelStore()

const quizzes = ref<Quiz[]>([])
const detail = ref<QuizDetail | null>(null)
const sections = ref<CourseSection[]>([])
const points = ref<KnowledgePoint[]>([])
const openMistakes = ref(0)
const loading = ref(true)
const working = ref(false)
const workingText = ref('')
const grading = ref(false)
const removing = ref<Quiz | null>(null)
const initial = ref<{ scope?: 'section' | 'points' | 'review' | 'mistakes'; sectionId?: number | null; pointIds?: number[] }>()

const activeId = computed(() => Number(route.query.q) || null)

async function refreshList() {
  quizzes.value = await QuizService.list(props.course.id)
}

async function loadDetail() {
  if (!activeId.value) {
    detail.value = null
    return
  }
  try {
    detail.value = await QuizService.get(activeId.value)
  } catch (e) {
    toast.error(e)
    detail.value = null
  }
}

watch(activeId, loadDetail)

function select(id: number) {
  router.replace({ query: { q: id } })
}

function openCreator(preset?: typeof initial.value) {
  initial.value = preset
  router.replace({ query: {} })
}

function replaceInList(quiz: Quiz) {
  const index = quizzes.value.findIndex(q => q.id === quiz.id)
  if (index >= 0) quizzes.value[index] = quiz
  else quizzes.value.unshift(quiz)
}

async function create(input: CreateQuizInput) {
  working.value = true
  workingText.value = 'AI 正在阅读资料并出题…'
  try {
    const created = await QuizService.create(input)
    replaceInList(created.quiz)
    detail.value = created
    router.replace({ query: { q: created.quiz.id } })
  } catch (e) {
    toast.error(e)
  } finally {
    working.value = false
  }
}

async function submit() {
  if (!detail.value) return
  working.value = true
  workingText.value = '正在批改：客观题自动判分，主观题由 AI 批改…'
  try {
    detail.value = await QuizService.submit(detail.value.quiz.id)
    replaceInList(detail.value.quiz)
    if (detail.value.quiz.status === 'graded') toast.success(`得分 ${detail.value.quiz.score}%，已更新掌握度和复习计划`)
  } catch (e) {
    toast.error(e)
    await loadDetail()
  } finally {
    working.value = false
  }
}

async function regrade() {
  if (!detail.value) return
  grading.value = true
  try {
    detail.value = await QuizService.regrade(detail.value.quiz.id)
    replaceInList(detail.value.quiz)
  } catch (e) {
    toast.error(e)
  } finally {
    grading.value = false
  }
}

async function selfGrade(questionId: number, score: number) {
  grading.value = true
  try {
    detail.value = await QuizService.selfGrade(questionId, score)
    replaceInList(detail.value.quiz)
  } catch (e) {
    toast.error(e)
  } finally {
    grading.value = false
  }
}

function retryPoints(pointIds: number[]) {
  create({ course_id: props.course.id, origin: 'manual', knowledge_point_ids: pointIds, question_count: Math.min(10, Math.max(3, pointIds.length * 2)) })
}

async function remove() {
  if (!removing.value) return
  const id = removing.value.id
  try {
    await QuizService.remove(id)
    removing.value = null
    quizzes.value = quizzes.value.filter(q => q.id !== id)
    if (activeId.value === id) router.replace({ query: {} })
  } catch (e) {
    toast.error(e)
  }
}

// The answering progress is saved continuously; generation and grading must not be interrupted.
onBeforeRouteLeave(() => !working.value)

onMounted(async () => {
  models.load().catch(() => undefined)
  if (!app.ready) await app.load().catch(() => undefined)
  try {
    const [, sectionList, pointList, mistakes] = await Promise.all([
      refreshList(),
      DocumentService.courseSections(props.course.id),
      KnowledgeService.list(props.course.id),
      ReviewService.mistakes(props.course.id)
    ])
    sections.value = sectionList
    points.value = pointList
    openMistakes.value = mistakes.length
    const query = route.query
    const pointIds = String(query.points ?? '').split(',').map(Number).filter(Boolean)
    const sectionId = Number(query.section) || null
    if (query.step) {
      // From a learning step: re-open its quiz or write one for the step's chapter.
      await create({ course_id: props.course.id, step_id: Number(query.step), section_id: sectionId })
    } else if (query.auto) {
      const origin = String(query.origin ?? (pointIds.length ? 'manual' : sectionId ? 'chapter' : 'review')) as CreateQuizInput['origin']
      await create({
        course_id: props.course.id,
        origin,
        knowledge_point_ids: pointIds,
        section_id: sectionId,
        ...(Number(query.count) ? { question_count: Number(query.count) } : {})
      })
    } else if (pointIds.length || sectionId || query.origin) {
      openCreator({
        scope: pointIds.length ? 'points' : sectionId ? 'section' : (String(query.origin) as 'review' | 'mistakes'),
        sectionId,
        pointIds
      })
    } else {
      await loadDetail()
    }
  } catch (e) {
    toast.error(e)
  } finally {
    loading.value = false
  }
})
</script>
