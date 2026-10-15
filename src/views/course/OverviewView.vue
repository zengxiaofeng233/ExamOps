<template>
  <div class="h-full overflow-y-auto p-6">
    <div class="mx-auto max-w-5xl space-y-5">
      <p v-if="loading" class="text-sm text-slate-400">加载学习状态…</p>

      <section v-else class="card p-5" aria-labelledby="continue-title">
        <h2 id="continue-title" class="section-title">继续学习</h2>
        <div v-if="hasProgress" class="mt-3 grid gap-3 md:grid-cols-3">
          <div v-if="state?.current_step" class="rounded-lg border border-indigo-200 bg-indigo-50/50 p-4 md:col-span-3">
            <p class="text-xs text-indigo-700">当前路径「{{ state.active_path?.name }}」的下一步 · 已完成 {{ state.active_path?.done_count }}/{{ state.active_path?.step_count }}</p>
            <p class="mt-1 flex flex-wrap items-center gap-2">
              <span class="badge bg-surface text-indigo-700">{{ state.current_step.kind_label }}</span>
              <span class="font-medium text-slate-900">{{ state.current_step.title }}</span>
              <span v-if="state.current_step.estimated_minutes" class="text-xs text-slate-500">约 {{ state.current_step.estimated_minutes }} 分钟</span>
            </p>
            <div class="mt-3 flex flex-wrap gap-2">
              <button v-if="stepTarget" class="btn btn-primary btn-sm" @click="openStep"><PlayIcon class="size-4" />{{ state.current_step.status === 'in_progress' ? '继续' : '开始' }}</button>
              <RouterLink :to="{ name: 'paths', params: { id: course.id }, query: { p: state.active_path?.id } }" class="btn btn-secondary btn-sm">查看路径</RouterLink>
            </div>
          </div>
          <RouterLink
            v-if="state?.last_conversation"
            :to="conversationLink"
            class="rounded-lg border border-slate-200 p-4 transition-colors hover:border-indigo-300 hover:bg-slate-50"
          >
            <p class="flex items-center gap-1.5 text-xs text-slate-500"><ChatBubbleLeftRightIcon class="size-4" />上次的 AI 学习</p>
            <p class="mt-1 truncate text-sm font-medium text-slate-800">{{ state.last_conversation.title || '对话' }}</p>
            <p class="mt-0.5 text-xs text-slate-400">{{ MODE_LABELS[state.last_conversation.mode] }} · {{ formatRelative(state.last_conversation.updated_at) }}</p>
          </RouterLink>
          <RouterLink
            v-if="state?.last_document_id"
            :to="{ name: 'documents', params: { id: course.id }, query: { doc: state.last_document_id, ...(state.last_page ? { page: state.last_page } : {}) } }"
            class="rounded-lg border border-slate-200 p-4 transition-colors hover:border-indigo-300 hover:bg-slate-50"
          >
            <p class="flex items-center gap-1.5 text-xs text-slate-500"><BookOpenIcon class="size-4" />上次阅读</p>
            <p class="mt-1 truncate text-sm font-medium text-slate-800">{{ state.last_document_name }}</p>
            <p class="mt-0.5 text-xs text-slate-400">{{ state.current_section_title ?? '' }}{{ state.last_page ? ` · 第 ${state.last_page} 处` : '' }}</p>
          </RouterLink>
          <RouterLink
            v-if="!state?.active_path"
            :to="{ name: 'paths', params: { id: course.id } }"
            class="rounded-lg border border-dashed border-slate-300 p-4 text-sm text-slate-500 transition-colors hover:border-indigo-300 hover:text-indigo-700"
          >
            <p class="font-medium">制定学习路径</p>
            <p class="mt-0.5 text-xs">按章节自动生成或让 AI 推荐，之后这里会显示下一步。</p>
          </RouterLink>
        </div>
        <ol v-else class="mt-3 grid gap-3 md:grid-cols-3">
          <li v-for="(item, index) in gettingStarted" :key="item.title" class="rounded-lg border border-slate-200 p-4">
            <p class="text-xs font-semibold text-indigo-600">第 {{ index + 1 }} 步</p>
            <p class="mt-1 text-sm font-medium text-slate-800">{{ item.title }}</p>
            <p class="mt-0.5 text-xs text-slate-500">{{ item.text }}</p>
            <RouterLink :to="item.to" class="btn btn-soft btn-sm mt-3">{{ item.action }}</RouterLink>
          </li>
        </ol>
      </section>

      <section v-if="!loading && (actions.length || reviewSummary?.tested)" class="card p-5" aria-labelledby="review-title">
        <div class="flex flex-wrap items-center justify-between gap-2">
          <h2 id="review-title" class="section-title">复习与建议</h2>
          <RouterLink :to="{ name: 'review', params: { id: course.id } }" class="text-xs text-indigo-600 hover:underline">查看复习计划</RouterLink>
        </div>
        <p v-if="reviewSummary" class="mt-2 flex flex-wrap gap-2 text-xs">
          <span class="badge" :class="reviewSummary.due ? 'bg-rose-50 text-rose-700' : 'bg-slate-100 text-slate-600'">今日到期 {{ reviewSummary.due }}</span>
          <span class="badge bg-amber-50 text-amber-700">薄弱 {{ reviewSummary.weak }}</span>
          <span class="badge bg-sky-50 text-sky-700">学过未测 {{ reviewSummary.untested }}</span>
          <span class="badge bg-slate-100 text-slate-600">错题 {{ reviewSummary.open_mistakes }}</span>
          <span v-if="reviewSummary.next_due && !reviewSummary.due" class="text-slate-400">下次复习：{{ describeDue(reviewSummary.next_due) }}</span>
        </p>
        <ul v-if="actions.length" class="mt-3 grid gap-2 md:grid-cols-3">
          <li v-for="action in actions.slice(0, 3)" :key="action.kind + action.title" class="flex flex-col rounded-lg border border-slate-200 p-3">
            <p class="text-sm font-medium text-slate-900">{{ action.title }}</p>
            <p class="mt-0.5 flex-1 text-xs text-slate-500">{{ action.detail }}</p>
            <button class="btn btn-soft btn-sm mt-2 self-start" @click="runAction(action)">开始</button>
          </li>
        </ul>
      </section>

      <div class="grid gap-5 md:grid-cols-3">
        <section class="card p-5 md:col-span-2" aria-labelledby="progress-title">
          <div class="flex items-center justify-between">
            <h2 id="progress-title" class="section-title">学习进度</h2>
            <RouterLink :to="{ name: 'knowledge', params: { id: course.id } }" class="text-xs text-indigo-600 hover:underline">管理知识点</RouterLink>
          </div>
          <template v-if="counts && counts.points">
            <div class="mt-4 flex h-3 overflow-hidden rounded-full bg-slate-100" role="img" :aria-label="`共 ${counts.points} 个知识点，学过 ${learnedOnly}，已掌握 ${counts.points_mastered}`">
              <div class="bg-emerald-500" :style="{ width: `${(counts.points_mastered / counts.points) * 100}%` }" />
              <div class="bg-sky-400" :style="{ width: `${(learnedOnly / counts.points) * 100}%` }" />
            </div>
            <dl class="mt-3 grid grid-cols-2 gap-3 text-sm sm:grid-cols-4">
              <div><dt class="text-xs text-slate-500">知识点</dt><dd class="text-lg font-semibold text-slate-900">{{ counts.points }}</dd></div>
              <div><dt class="flex items-center gap-1 text-xs text-slate-500"><span class="size-2 rounded-full bg-sky-400" />学过</dt><dd class="text-lg font-semibold text-sky-700">{{ learnedOnly }}</dd></div>
              <div><dt class="flex items-center gap-1 text-xs text-slate-500"><span class="size-2 rounded-full bg-emerald-500" />已掌握</dt><dd class="text-lg font-semibold text-emerald-700">{{ counts.points_mastered }}</dd></div>
              <div><dt class="text-xs text-slate-500">疑难</dt><dd class="text-lg font-semibold text-orange-600">{{ counts.points_difficult }}</dd></div>
            </dl>
            <p class="mt-2 text-xs text-slate-400">“学过”是学习过（阅读、讲解、网课），“已掌握”由测验结果决定。</p>
          </template>
          <p v-else class="mt-3 text-sm text-slate-500">还没有知识点。导入资料后，可以在“知识点”页从章节标题生成或让 AI 提取。</p>
          <p v-if="counts && counts.steps" class="mt-4 text-sm text-slate-600">学习路径步骤：已完成 {{ counts.steps_done }}/{{ counts.steps }}</p>
        </section>

        <section class="card p-5" aria-labelledby="exam-title">
          <h2 id="exam-title" class="section-title">考试日期</h2>
          <p v-if="countdown" class="mt-2 text-2xl font-semibold" :class="(days ?? 99) <= 7 ? 'text-rose-600' : 'text-slate-900'">{{ countdown }}</p>
          <p v-else class="mt-2 text-sm text-slate-500">设置考试日期后，考前冲刺和复习安排会参考剩余天数。</p>
          <div class="mt-3 flex gap-2">
            <input v-model="examDate" type="date" class="input py-1.5" aria-label="考试日期" />
            <button class="btn btn-secondary btn-sm shrink-0" :disabled="savingDate || examDate === (course.exam_date ?? '')" @click="saveExamDate">保存</button>
          </div>
          <button v-if="course.exam_date" class="mt-1 text-xs text-slate-400 hover:text-rose-600" @click="clearExamDate">清除日期</button>
        </section>
      </div>

      <div class="grid gap-5 md:grid-cols-3">
        <section class="card p-5 md:col-span-2" aria-labelledby="activity-title">
          <h2 id="activity-title" class="section-title">近两周学习记录</h2>
          <div v-if="state" class="mt-4 flex h-24 items-end gap-1.5" role="img" :aria-label="`近 14 天共 ${totalEvents} 次学习活动`">
            <div v-for="day in state.activity" :key="day.date" class="flex flex-1 flex-col items-center gap-1" :title="`${day.date}：${day.events} 次`">
              <div class="w-full rounded-t bg-indigo-400" :style="{ height: `${barHeight(day.events)}px` }" />
              <span class="text-[10px] text-slate-400">{{ day.date.slice(8) }}</span>
            </div>
          </div>
          <p class="mt-2 text-xs text-slate-400">统计阅读、AI 讲解、完成学习步骤等活动。</p>
        </section>
        <section class="card p-5" aria-labelledby="about-title">
          <h2 id="about-title" class="section-title">课程简介</h2>
          <p class="mt-2 text-sm whitespace-pre-wrap text-slate-600">{{ course.description || '暂无简介，可在课程列表中编辑。' }}</p>
          <p class="mt-3 text-xs text-slate-400">{{ counts?.documents ?? 0 }} 份资料 · {{ counts?.sections ?? 0 }} 个章节 · 创建于 {{ formatDateTime(course.created_at).slice(0, 10) }}</p>
        </section>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { BookOpenIcon, ChatBubbleLeftRightIcon, PlayIcon } from '@heroicons/vue/20/solid'
import { ReviewService } from '@/services/assessment'
import { PathService, StudyService } from '@/services/study'
import { useCourseStore } from '@/stores/course'
import { useToastStore } from '@/stores/toast'
import type { Course } from '@/types/models'
import type { ReviewSummary, StudyAction } from '@/types/assessment'
import type { StudyState } from '@/types/study'
import { actionTarget, describeDue } from '@/utils/assessment'
import { formatDateTime, formatRelative } from '@/utils/format'
import { daysUntil, examCountdown, MODE_LABELS, stepRoute } from '@/utils/study'

const props = defineProps<{ course: Course }>()
const router = useRouter()
const toast = useToastStore()
const courses = useCourseStore()

const state = ref<StudyState | null>(null)
const reviewSummary = ref<ReviewSummary | null>(null)
const actions = ref<StudyAction[]>([])
const loading = ref(true)
const savingDate = ref(false)
const examDate = ref(props.course.exam_date ?? '')

const counts = computed(() => state.value?.counts ?? null)
const learnedOnly = computed(() => (counts.value ? counts.value.points_learned - counts.value.points_mastered : 0))
const days = computed(() => daysUntil(props.course.exam_date))
const countdown = computed(() => examCountdown(props.course.exam_date))
const totalEvents = computed(() => state.value?.activity.reduce((sum, d) => sum + d.events, 0) ?? 0)
const maxEvents = computed(() => Math.max(1, ...(state.value?.activity.map(d => d.events) ?? [1])))
const hasProgress = computed(() => !!(state.value?.current_step || state.value?.last_conversation || state.value?.last_document_id))

const stepTarget = computed(() => {
  const step = state.value?.current_step
  if (!step || !state.value?.active_path) return null
  const target = stepRoute(step, props.course.id, state.value.active_path.mode)
  const name = target && typeof target === 'object' && 'name' in target ? target.name : undefined
  return target && (!name || router.hasRoute(name)) ? target : null
})

const conversationLink = computed(() => {
  const conversation = state.value?.last_conversation
  if (!conversation) return { name: 'learn', params: { id: props.course.id } }
  return conversation.mode === 'qa'
    ? { name: 'tutor', params: { id: props.course.id }, query: { c: conversation.id } }
    : { name: 'learn', params: { id: props.course.id }, query: { c: conversation.id } }
})

const gettingStarted = computed(() => [
  { title: '导入资料', text: '教材、讲义 PDF、PPT、Word 都可以，系统会识别章节。', action: '导入资料', to: { name: 'documents', params: { id: props.course.id } } },
  { title: '整理知识点', text: '从章节标题一键生成，或让 AI 提取每章的核心知识点。', action: '去整理', to: { name: 'knowledge', params: { id: props.course.id } } },
  { title: '开始学习', text: '制定学习路径，或直接让 AI 按章节讲解。', action: 'AI 学习', to: { name: 'learn', params: { id: props.course.id } } }
])

function barHeight(events: number) {
  return events ? Math.max(6, Math.round((events / maxEvents.value) * 72)) : 2
}

async function openStep() {
  const step = state.value?.current_step
  if (!step || !stepTarget.value) return
  try {
    if (step.status === 'todo') await PathService.setStatus(step.id, 'in_progress')
    await router.push(stepTarget.value)
  } catch (e) {
    toast.error(e)
  }
}

async function updateExamDate(value: string) {
  savingDate.value = true
  try {
    await courses.updateCourse({ id: props.course.id, exam_date: value })
    examDate.value = value
    toast.success(value ? '考试日期已保存' : '已清除考试日期')
  } catch (e) {
    toast.error(e)
  } finally {
    savingDate.value = false
  }
}

const saveExamDate = () => updateExamDate(examDate.value)
const clearExamDate = () => updateExamDate('')

function runAction(action: StudyAction) {
  const target = actionTarget(action, props.course.id)
  if (target.type === 'route') {
    router.push(target.to)
    return
  }
  const input = target.input
  router.push({
    name: 'quiz',
    params: { id: props.course.id },
    query: { auto: 1, origin: input.origin, ...(input.knowledge_point_ids?.length ? { points: input.knowledge_point_ids.join(',') } : {}) }
  })
}

onMounted(async () => {
  try {
    const [studyState, queue, recommendations] = await Promise.all([
      StudyService.state(props.course.id),
      ReviewService.queue(props.course.id),
      ReviewService.recommendations(props.course.id)
    ])
    state.value = studyState
    reviewSummary.value = queue.summary
    actions.value = recommendations
  } catch (e) {
    toast.error(e)
  } finally {
    loading.value = false
  }
})
</script>
