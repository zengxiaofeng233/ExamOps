<template>
  <div class="mx-auto max-w-3xl space-y-4 p-5">
    <header class="card p-5">
      <div class="flex flex-wrap items-center gap-5">
        <div class="text-center">
          <p v-if="quiz.status === 'graded'" class="text-4xl font-bold tabular-nums" :class="TONE_TEXT[scoreTone((quiz.score ?? 0) / 100)]">{{ quiz.score }}<span class="text-xl">%</span></p>
          <p v-else class="text-lg font-semibold text-amber-600">批改中</p>
          <p class="text-xs text-slate-500">得分</p>
        </div>
        <div class="min-w-0 flex-1">
          <h2 class="truncate text-base font-semibold text-slate-900" :title="quiz.title">{{ quiz.title }}</h2>
          <p class="mt-0.5 text-sm text-slate-600">答对 {{ correctCount }}/{{ questions.length }}<template v-if="partialCount"> · 部分正确 {{ partialCount }}</template><template v-if="skippedCount"> · 跳过 {{ skippedCount }}</template></p>
          <p class="text-xs text-slate-400">{{ formatDateTime(quiz.submitted_at ?? quiz.created_at) }}<template v-if="quiz.model"> · {{ quiz.model }}</template></p>
        </div>
        <div class="flex flex-wrap gap-2">
          <button v-if="wrongPoints.length" class="btn btn-soft btn-sm" :disabled="busy" @click="emit('retry', wrongPoints)"><ArrowPathIcon class="size-4" />针对错题再练</button>
          <RouterLink
            v-if="quiz.section_id"
            :to="{ name: 'learn', params: { id: quiz.course_id }, query: { mode: 'review', section: quiz.section_id } }"
            class="btn btn-secondary btn-sm"
          >
            <AcademicCapIcon class="size-4" />AI 复习本章
          </RouterLink>
        </div>
      </div>
      <div v-if="quiz.pending" class="mt-4 rounded-lg border border-amber-200 bg-amber-50 p-3 text-sm text-amber-800" role="status">
        <p>{{ quiz.pending }} 道主观题还没有评分<template v-if="quiz.grading_error">：{{ quiz.grading_error }}</template>。</p>
        <p class="mt-1 text-xs">可以重新让 AI 批改，或对照参考答案自己评分。全部评分后才会更新掌握度和复习计划。</p>
        <button class="btn btn-secondary btn-sm mt-2" :disabled="busy" @click="emit('regrade')"><ArrowPathIcon class="size-4" :class="busy ? 'animate-spin' : ''" />重新 AI 批改</button>
      </div>
    </header>

    <section v-if="detail.outcome.length" class="card p-5" aria-labelledby="outcome-title">
      <h3 id="outcome-title" class="section-title">知识点掌握情况</h3>
      <ul class="mt-3 divide-y divide-slate-100">
        <li v-for="item in detail.outcome" :key="item.knowledge_point_id" class="flex flex-wrap items-center gap-3 py-2.5">
          <span class="min-w-0 flex-1 truncate text-sm font-medium text-slate-800">{{ item.title }}</span>
          <span class="text-xs text-slate-500">本次 {{ percent(item.result) }}</span>
          <span class="flex items-center gap-1.5 text-xs text-slate-500">
            <template v-if="item.before !== null">{{ percent(item.before) }} →</template>
            <MasteryBar :score="item.after" compact />
          </span>
          <span class="badge" :class="POINT_STATUS_CLASSES[item.status as PointStatus] ?? ''">{{ POINT_STATUS_LABELS[item.status as PointStatus] ?? item.status }}</span>
          <span v-if="item.due_date" class="text-xs text-slate-500">下次复习：{{ describeDue(item.due_date) }}</span>
        </li>
      </ul>
    </section>

    <ol class="space-y-3" aria-label="答题详情">
      <li v-for="(question, i) in questions" :key="question.id" class="card p-5">
        <p class="flex flex-wrap items-center gap-2 text-xs text-slate-500">
          <span class="font-semibold text-slate-700">第 {{ i + 1 }} 题</span>
          <span class="badge bg-slate-100 text-slate-600">{{ question.kind_label }}</span>
          <span v-if="question.knowledge_point_title">{{ question.knowledge_point_title }}</span>
          <span class="ml-auto badge" :class="verdict(question).class">{{ verdict(question).label }}</span>
        </p>
        <MarkdownContent :text="question.prompt" class="mt-2" />

        <ul v-if="question.kind === 'single_choice'" class="mt-3 space-y-1.5">
          <li
            v-for="(option, k) in question.options"
            :key="k"
            class="flex items-start gap-2 rounded-lg border px-3 py-2 text-sm"
            :class="optionClass(question, LETTERS[k])"
          >
            <span class="font-semibold">{{ LETTERS[k] }}.</span>
            <MarkdownContent :text="option" class="min-w-0 flex-1" />
            <span v-if="LETTERS[k] === question.answer" class="text-xs font-medium text-emerald-700">正确答案</span>
            <span v-else-if="LETTERS[k] === question.response" class="text-xs font-medium text-rose-700">你的选择</span>
          </li>
        </ul>
        <dl v-else class="mt-3 grid gap-2 text-sm sm:grid-cols-[5.5rem_minmax(0,1fr)]">
          <dt class="text-slate-500">你的答案</dt>
          <dd class="whitespace-pre-wrap text-slate-800">{{ answerText(question, question.response) }}</dd>
          <dt class="text-slate-500">{{ question.kind === 'true_false' ? '正确答案' : '参考答案' }}</dt>
          <dd><MarkdownContent :text="answerText(question, question.answer)" /></dd>
          <template v-if="question.rubric.length">
            <dt class="text-slate-500">评分要点</dt>
            <dd><ul class="list-disc pl-5 text-slate-700"><li v-for="point in question.rubric" :key="point">{{ point }}</li></ul></dd>
          </template>
        </dl>

        <div v-if="question.feedback && question.feedback !== '自评'" class="mt-3 rounded-lg bg-indigo-50/60 p-3">
          <p class="mb-1 text-xs font-medium text-indigo-700">{{ question.grading === 'ai' ? 'AI 批改反馈' : '说明' }}</p>
          <MarkdownContent :text="question.feedback" :sources="detail.sources" @cite="openSource" />
        </div>
        <div v-if="question.explanation" class="mt-3">
          <p class="mb-1 text-xs font-medium text-slate-500">解析</p>
          <MarkdownContent :text="question.explanation" :sources="detail.sources" @cite="openSource" />
        </div>
        <div v-if="question.grading === 'pending'" class="mt-3 flex flex-wrap items-center gap-2 rounded-lg border border-amber-200 bg-amber-50 p-3">
          <span class="text-xs text-amber-800">对照参考答案自己评分：</span>
          <button v-for="option in SELF_GRADES" :key="option.score" class="btn btn-secondary btn-sm" :disabled="busy" @click="emit('self-grade', question.id, option.score)">{{ option.label }}</button>
        </div>
        <SourceList :sources="question.sources" @open="openSource" />
      </li>
    </ol>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useRouter } from 'vue-router'
import { AcademicCapIcon, ArrowPathIcon } from '@heroicons/vue/20/solid'
import MarkdownContent from '@/components/chat/MarkdownContent.vue'
import SourceList from '@/components/chat/SourceList.vue'
import MasteryBar from '@/components/study/MasteryBar.vue'
import type { SourceRef } from '@/types/ai'
import type { Question, QuizDetail } from '@/types/assessment'
import type { PointStatus } from '@/types/study'
import { answerText, describeDue, LETTERS, percent, scoreTone, TONE_TEXT } from '@/utils/assessment'
import { formatDateTime } from '@/utils/format'
import { POINT_STATUS_CLASSES, POINT_STATUS_LABELS } from '@/utils/study'

const SELF_GRADES = [
  { score: 0, label: '没答对' },
  { score: 0.5, label: '部分正确' },
  { score: 1, label: '答对了' }
]

const props = defineProps<{ detail: QuizDetail; busy?: boolean }>()
const emit = defineEmits<{ regrade: []; 'self-grade': [questionId: number, score: number]; retry: [pointIds: number[]] }>()
const router = useRouter()

const quiz = computed(() => props.detail.quiz)
const questions = computed(() => props.detail.questions)
const correctCount = computed(() => questions.value.filter(q => (q.score ?? 0) >= 0.8).length)
const partialCount = computed(() => questions.value.filter(q => q.score !== null && q.score >= 0.3 && q.score < 0.8).length)
const skippedCount = computed(() => questions.value.filter(q => q.skipped).length)
const wrongPoints = computed(() => [...new Set(questions.value.filter(q => q.score !== null && q.score < 0.6 && q.knowledge_point_id).map(q => q.knowledge_point_id as number))])

function verdict(question: Question) {
  if (question.score === null) return { label: '待批改', class: 'bg-amber-50 text-amber-700' }
  if (question.skipped) return { label: '跳过', class: 'bg-slate-100 text-slate-500' }
  if (question.score >= 0.8) return { label: question.score < 1 ? `正确 ${percent(question.score)}` : '正确', class: 'bg-emerald-50 text-emerald-700' }
  if (question.score >= 0.3) return { label: `部分正确 ${percent(question.score)}`, class: 'bg-amber-50 text-amber-700' }
  return { label: '错误', class: 'bg-rose-50 text-rose-700' }
}

function optionClass(question: Question, letter: string) {
  if (letter === question.answer) return 'border-emerald-300 bg-emerald-50/70'
  if (letter === question.response) return 'border-rose-300 bg-rose-50/70'
  return 'border-slate-200'
}

function openSource(source: SourceRef) {
  router.push({ name: 'documents', params: { id: quiz.value.course_id }, query: { doc: source.document_id, page: source.page_start } })
}
</script>
