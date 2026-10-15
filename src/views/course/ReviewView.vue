<template>
  <div class="h-full overflow-y-auto p-6">
    <div class="mx-auto max-w-5xl space-y-5">
      <p v-if="loading" class="text-sm text-slate-400">加载复习计划…</p>
      <template v-else-if="queue">
        <section class="grid grid-cols-2 gap-3 md:grid-cols-5" aria-label="复习概况">
          <div v-for="card in cards" :key="card.label" class="card p-4">
            <p class="text-xs text-slate-500">{{ card.label }}</p>
            <p class="mt-1 text-2xl font-semibold tabular-nums" :class="card.tone">{{ card.value }}</p>
            <p v-if="card.hint" class="text-[11px] text-slate-400">{{ card.hint }}</p>
          </div>
        </section>

        <div class="grid gap-5 lg:grid-cols-5">
          <section class="card p-5 lg:col-span-3" aria-labelledby="next-title">
            <h2 id="next-title" class="section-title">下一步建议</h2>
            <p v-if="!actions.length" class="mt-3 text-sm text-slate-500">暂时没有需要处理的内容，继续按学习路径推进吧。</p>
            <ul v-else class="mt-3 space-y-2">
              <li v-for="action in actions" :key="action.kind + action.title" class="flex items-start gap-3 rounded-lg border border-slate-200 p-3">
                <component :is="ACTION_ICONS[action.kind] ?? LightBulbIcon" class="mt-0.5 size-5 shrink-0 text-indigo-500" />
                <div class="min-w-0 flex-1">
                  <p class="text-sm font-medium text-slate-900">{{ action.title }}</p>
                  <p class="mt-0.5 text-xs text-slate-500">{{ action.detail }}</p>
                </div>
                <button class="btn btn-soft btn-sm shrink-0" @click="run(action)">开始</button>
              </li>
            </ul>
          </section>

          <section class="card p-5 lg:col-span-2" aria-labelledby="advice-title">
            <div class="flex items-center justify-between gap-2">
              <h2 id="advice-title" class="section-title">AI 学习建议</h2>
              <button class="btn btn-secondary btn-sm" :disabled="advising || !models.configured" @click="askAdvice">
                <SparklesIcon class="size-4" :class="advising ? 'animate-pulse' : ''" />{{ advice ? '重新生成' : '生成建议' }}
              </button>
            </div>
            <p v-if="advising" class="mt-3 text-sm text-indigo-700" role="status">AI 正在分析你的学习数据…</p>
            <template v-else-if="advice">
              <MarkdownContent :text="advice.text" class="mt-3" />
              <div v-if="advice.focus.length" class="mt-3 rounded-lg bg-indigo-50/60 p-3">
                <p class="text-xs font-medium text-indigo-700">建议优先复习</p>
                <p class="mt-1 flex flex-wrap gap-1.5">
                  <span v-for="point in advice.focus" :key="point.id" class="badge bg-surface text-indigo-700">{{ point.title }}</span>
                </p>
                <button class="btn btn-primary btn-sm mt-2" @click="quiz({ course_id: course.id, origin: 'review', knowledge_point_ids: advice.focus.map(f => f.id) })">做这些知识点的小测</button>
              </div>
              <p class="mt-2 text-[11px] text-slate-400">由 {{ advice.model }} 根据你的掌握度、到期复习、错题和考试日期生成</p>
            </template>
            <p v-else class="mt-3 text-sm text-slate-500">让 AI 根据你的掌握度、错题、复习计划和考试日期，给出今天和接下来几天的复习安排。</p>
          </section>
        </div>

        <section class="card p-5" aria-labelledby="queue-title">
          <div class="flex flex-wrap items-center justify-between gap-2">
            <div>
              <h2 id="queue-title" class="section-title">复习队列</h2>
              <p class="text-xs text-slate-500">按间隔复习：答得好间隔变长，答得不好明天再复习；临近考试时自动缩短间隔。</p>
            </div>
            <button v-if="queue.items.length" class="btn btn-primary btn-sm" @click="quiz({ course_id: course.id, origin: 'review' })">
              <PlayIcon class="size-4" />开始复习小测
            </button>
          </div>
          <p v-if="!queue.items.length" class="mt-4 text-sm text-slate-500">
            {{ queue.summary.tested ? `没有到期的复习${queue.summary.next_due ? `，下次复习：${describeDue(queue.summary.next_due)}（${queue.summary.next_due}）` : ''}。` : '学过的知识点做过小测后，这里会按遗忘规律安排复习。' }}
          </p>
          <ul v-else class="mt-3 divide-y divide-slate-100">
            <li v-for="item in queue.items" :key="item.point.id" class="flex flex-wrap items-center gap-3 py-2.5">
              <span class="badge shrink-0" :class="REASON_CLASSES[item.reason]">{{ item.reason_label }}</span>
              <span class="min-w-40 flex-1">
                <span class="block truncate text-sm font-medium text-slate-800">{{ item.point.title }}</span>
                <span class="block truncate text-xs text-slate-400">{{ item.point.section_title ?? item.point.filename ?? '' }}</span>
              </span>
              <MasteryBar :score="item.mastery?.score ?? null" />
              <span class="w-20 text-right text-xs text-slate-500">{{ item.mastery?.due_date ? describeDue(item.mastery.due_date) : '' }}</span>
              <span class="flex gap-1">
                <button class="btn btn-ghost btn-sm" title="针对这个知识点做小测" @click="quiz({ course_id: course.id, origin: 'manual', knowledge_point_ids: [item.point.id], question_count: 3 })">小测</button>
                <RouterLink
                  v-if="item.point.section_id"
                  :to="{ name: 'learn', params: { id: course.id }, query: { mode: 'review', section: item.point.section_id } }"
                  class="btn btn-ghost btn-sm"
                  title="让 AI 带你回忆和纠错"
                >AI 复习</RouterLink>
              </span>
            </li>
          </ul>
        </section>

        <section class="card p-5" aria-labelledby="mistakes-title">
          <div class="flex flex-wrap items-center justify-between gap-2">
            <h2 id="mistakes-title" class="section-title">错题本</h2>
            <div class="flex items-center gap-2">
              <label class="flex items-center gap-1.5 text-xs text-slate-600">
                <input v-model="showResolved" type="checkbox" class="accent-indigo-600" @change="loadMistakes" />显示已解决
              </label>
              <button v-if="openCount" class="btn btn-primary btn-sm" @click="quiz({ course_id: course.id, origin: 'mistakes' })"><ArrowPathIcon class="size-4" />重练错题</button>
            </div>
          </div>
          <p v-if="!mistakes.length" class="mt-4 text-sm text-slate-500">{{ showResolved ? '还没有错题。' : '没有未解决的错题。' }}</p>
          <ul v-else class="mt-3 space-y-2">
            <li v-for="mistake in mistakes" :key="mistake.id" class="rounded-lg border p-3" :class="mistake.resolved ? 'border-slate-100 opacity-60' : 'border-slate-200'">
              <details>
                <summary class="flex cursor-pointer list-none items-start gap-2">
                  <span class="badge shrink-0 bg-slate-100 text-slate-600">{{ mistake.kind_label }}</span>
                  <span class="min-w-0 flex-1 text-sm text-slate-800">{{ plain(mistake.prompt) }}</span>
                  <span class="shrink-0 text-xs text-slate-400">{{ mistake.knowledge_point_title }}</span>
                </summary>
                <dl class="mt-3 grid gap-2 text-sm sm:grid-cols-[5.5rem_minmax(0,1fr)]">
                  <dt class="text-slate-500">你的答案</dt>
                  <dd class="whitespace-pre-wrap text-rose-700">{{ answerText(mistake, mistake.response) }}</dd>
                  <dt class="text-slate-500">{{ mistake.kind === 'single_choice' || mistake.kind === 'true_false' ? '正确答案' : '参考答案' }}</dt>
                  <dd><MarkdownContent :text="answerText(mistake, mistake.answer)" /></dd>
                  <template v-if="mistake.explanation">
                    <dt class="text-slate-500">解析</dt>
                    <dd><MarkdownContent :text="mistake.explanation" /></dd>
                  </template>
                  <template v-if="mistake.feedback && mistake.feedback !== '自评'">
                    <dt class="text-slate-500">批改反馈</dt>
                    <dd><MarkdownContent :text="mistake.feedback" /></dd>
                  </template>
                </dl>
                <div class="mt-2 flex flex-wrap gap-2">
                  <button class="btn btn-secondary btn-sm" @click="toggleResolved(mistake.id, !mistake.resolved)">{{ mistake.resolved ? '移回错题本' : '标记已解决' }}</button>
                  <RouterLink :to="{ name: 'quiz', params: { id: course.id }, query: { q: mistake.quiz_id } }" class="btn btn-ghost btn-sm">查看原小测</RouterLink>
                </div>
              </details>
            </li>
          </ul>
        </section>
      </template>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, type Component } from 'vue'
import { useRouter } from 'vue-router'
import {
  AcademicCapIcon, ArrowPathIcon, BookOpenIcon, ClipboardDocumentCheckIcon, ExclamationTriangleIcon, FireIcon, LightBulbIcon,
  ListBulletIcon, MapIcon, PlayIcon, SparklesIcon
} from '@heroicons/vue/20/solid'
import MarkdownContent from '@/components/chat/MarkdownContent.vue'
import MasteryBar from '@/components/study/MasteryBar.vue'
import { ReviewService } from '@/services/assessment'
import { useModelStore } from '@/stores/models'
import { useToastStore } from '@/stores/toast'
import type { Advice, CreateQuizInput, Mistake, ReviewItem, ReviewQueue, StudyAction } from '@/types/assessment'
import type { Course } from '@/types/models'
import { actionTarget, answerText, describeDue, percent } from '@/utils/assessment'

const ACTION_ICONS: Record<string, Component> = {
  review_quiz: ClipboardDocumentCheckIcon,
  mistakes_quiz: ExclamationTriangleIcon,
  point_quiz: ClipboardDocumentCheckIcon,
  review_session: AcademicCapIcon,
  path_step: MapIcon,
  sprint: FireIcon,
  learn: BookOpenIcon,
  extract_points: ListBulletIcon
}
const REASON_CLASSES: Record<ReviewItem['reason'], string> = {
  due: 'bg-rose-50 text-rose-700',
  weak: 'bg-amber-50 text-amber-700',
  difficult: 'bg-orange-50 text-orange-700',
  untested: 'bg-sky-50 text-sky-700'
}

const props = defineProps<{ course: Course }>()
const router = useRouter()
const toast = useToastStore()
const models = useModelStore()

const queue = ref<ReviewQueue | null>(null)
const actions = ref<StudyAction[]>([])
const mistakes = ref<Mistake[]>([])
const advice = ref<Advice | null>(null)
const loading = ref(true)
const advising = ref(false)
const showResolved = ref(false)

const openCount = computed(() => queue.value?.summary.open_mistakes ?? 0)
const cards = computed(() => {
  const s = queue.value?.summary
  if (!s) return []
  return [
    { label: '今日到期', value: s.due, hint: s.overdue ? `其中过期 ${s.overdue}` : s.next_due ? `下次 ${describeDue(s.next_due)}` : '', tone: s.due ? 'text-rose-600' : 'text-slate-900' },
    { label: '薄弱知识点', value: s.weak, hint: '掌握度低于 60%', tone: s.weak ? 'text-amber-600' : 'text-slate-900' },
    { label: '学过未测', value: s.untested, hint: '学过不等于掌握', tone: 'text-sky-700' },
    { label: '未解决错题', value: s.open_mistakes, hint: '', tone: s.open_mistakes ? 'text-rose-600' : 'text-slate-900' },
    { label: '已掌握', value: `${s.mastered}/${s.tested}`, hint: s.average !== null ? `平均掌握度 ${percent(s.average)}` : '还没有测验', tone: 'text-emerald-700' }
  ]
})

function plain(text: string) {
  return text.replace(/\$+/g, '').replace(/\s+/g, ' ').slice(0, 120)
}

async function loadMistakes() {
  mistakes.value = await ReviewService.mistakes(props.course.id, showResolved.value).catch((e) => { toast.error(e); return [] })
}

async function refresh() {
  const [q, a] = await Promise.all([ReviewService.queue(props.course.id), ReviewService.recommendations(props.course.id)])
  queue.value = q
  actions.value = a
  await loadMistakes()
}

onMounted(async () => {
  models.load().catch(() => undefined)
  try {
    await refresh()
  } catch (e) {
    toast.error(e)
  } finally {
    loading.value = false
  }
})

/** Quizzes are written on the 测验 page (it shows progress while the AI works). */
function quiz(input: CreateQuizInput) {
  router.push({
    name: 'quiz',
    params: { id: props.course.id },
    query: {
      auto: 1,
      origin: input.origin,
      ...(input.knowledge_point_ids?.length ? { points: input.knowledge_point_ids.join(',') } : {}),
      ...(input.question_count ? { count: input.question_count } : {})
    }
  })
}

function run(action: StudyAction) {
  const target = actionTarget(action, props.course.id)
  if (target.type === 'quiz') quiz(target.input)
  else router.push(target.to)
}

async function askAdvice() {
  advising.value = true
  try {
    advice.value = await ReviewService.advice(props.course.id)
  } catch (e) {
    toast.error(e)
  } finally {
    advising.value = false
  }
}

async function toggleResolved(id: number, resolved: boolean) {
  try {
    await ReviewService.resolveMistake(id, resolved)
    await refresh()
  } catch (e) {
    toast.error(e)
  }
}
</script>
