<template>
  <div class="mx-auto max-w-3xl space-y-4 p-5">
    <header class="card p-4">
      <div class="flex flex-wrap items-center gap-2">
        <h2 class="min-w-40 flex-1 truncate text-base font-semibold text-slate-900" :title="quiz.title">{{ quiz.title }}</h2>
        <span class="text-xs text-slate-500">已答 {{ answeredCount }}/{{ questions.length }}{{ quiz.allow_skip ? '' : ' · 不可跳过' }}</span>
      </div>
      <ol class="mt-3 flex flex-wrap gap-1.5" aria-label="题目导航">
        <li v-for="(item, i) in questions" :key="item.id">
          <button
            class="flex size-8 items-center justify-center rounded-full border text-xs font-medium transition-colors"
            :class="navClass(item, i)"
            :aria-current="i === index ? 'step' : undefined"
            :aria-label="`第 ${i + 1} 题${item.skipped ? '（已跳过）' : hasAnswer(item) ? '（已作答）' : ''}`"
            :disabled="busy"
            @click="go(i)"
          >
            {{ i + 1 }}
          </button>
        </li>
      </ol>
    </header>

    <section v-if="current" class="card p-5" :aria-label="`第 ${index + 1} 题`">
      <p class="flex flex-wrap items-center gap-2 text-xs text-slate-500">
        <span class="font-semibold text-slate-700">第 {{ index + 1 }} 题</span>
        <span class="badge bg-indigo-50 text-indigo-700">{{ current.kind_label }}</span>
        <span v-if="current.knowledge_point_title">知识点：{{ current.knowledge_point_title }}</span>
      </p>
      <MarkdownContent :text="current.prompt" class="mt-3" />

      <div v-if="current.kind === 'single_choice'" class="mt-4 space-y-2" role="radiogroup" :aria-label="`第 ${index + 1} 题选项`">
        <label
          v-for="(option, i) in current.options"
          :key="i"
          class="flex cursor-pointer items-start gap-3 rounded-lg border px-3 py-2.5 transition-colors"
          :class="drafts[current.id] === LETTERS[i] ? 'border-indigo-400 bg-indigo-50/60' : 'border-slate-200 hover:bg-slate-50'"
        >
          <input v-model="drafts[current.id]" type="radio" :name="`q-${current.id}`" :value="LETTERS[i]" class="mt-1.5 accent-indigo-600" @change="persist(current)" />
          <span class="mt-0.5 text-sm font-semibold text-slate-600">{{ LETTERS[i] }}.</span>
          <MarkdownContent :text="option" class="min-w-0 flex-1" />
        </label>
      </div>
      <div v-else-if="current.kind === 'true_false'" class="mt-4 grid grid-cols-2 gap-3" role="radiogroup" :aria-label="`第 ${index + 1} 题判断`">
        <button
          v-for="choice in TRUE_FALSE"
          :key="choice.value"
          type="button"
          role="radio"
          :aria-checked="drafts[current.id] === choice.value"
          class="flex items-center justify-center gap-2 rounded-lg border py-3 text-sm font-medium transition-colors"
          :class="drafts[current.id] === choice.value ? 'border-indigo-400 bg-indigo-50 text-indigo-800' : 'border-slate-200 text-slate-700 hover:bg-slate-50'"
          @click="choose(choice.value)"
        >
          <component :is="choice.icon" class="size-5" />{{ choice.label }}
        </button>
      </div>
      <div v-else class="mt-4">
        <textarea
          v-model="drafts[current.id]"
          class="input min-h-40 leading-7"
          rows="7"
          maxlength="4000"
          :placeholder="WRITTEN_HINTS[current.kind] ?? '写下你的答案'"
          :aria-label="`第 ${index + 1} 题答案`"
          @input="scheduleSave"
        />
        <p class="hint">{{ (drafts[current.id] ?? '').length }}/4000 · 交卷后由 AI 对照参考答案和资料批改</p>
      </div>
      <p v-if="current.skipped && !hasDraft" class="mt-3 text-xs text-slate-500">这道题已跳过，可以随时回来作答。</p>
    </section>

    <footer class="flex flex-wrap items-center gap-2">
      <button class="btn btn-secondary" :disabled="busy || index === 0" @click="go(index - 1)"><ChevronLeftIcon class="size-4" />上一题</button>
      <button v-if="quiz.allow_skip && index < questions.length - 1" class="btn btn-ghost" :disabled="busy" @click="skip">跳过</button>
      <span class="flex-1" />
      <button v-if="index < questions.length - 1" class="btn btn-primary" :disabled="busy || (!quiz.allow_skip && !hasDraft)" @click="go(index + 1)">
        下一题<ChevronRightIcon class="size-4" />
      </button>
      <button v-else class="btn btn-primary" :disabled="busy" @click="requestSubmit"><CheckIcon class="size-4" />交卷</button>
    </footer>
    <p v-if="index < questions.length - 1" class="text-right text-xs text-slate-400">
      答完可以随时 <button class="text-indigo-600 hover:underline disabled:opacity-50" :disabled="busy" @click="requestSubmit">交卷</button>，进度会自动保存
    </p>

    <ConfirmDialog
      :open="confirming"
      title="交卷"
      :message="unanswered ? `还有 ${unanswered} 道题没有作答，交卷后按跳过计 0 分。确定交卷吗？` : '交卷后客观题立即判分，主观题由 AI 批改。确定交卷吗？'"
      confirm-text="交卷"
      @cancel="confirming = false"
      @confirm="submit"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, reactive, ref, watch } from 'vue'
import { CheckCircleIcon, CheckIcon, ChevronLeftIcon, ChevronRightIcon, XCircleIcon } from '@heroicons/vue/20/solid'
import MarkdownContent from '@/components/chat/MarkdownContent.vue'
import ConfirmDialog from '@/components/ui/ConfirmDialog.vue'
import { QuizService } from '@/services/assessment'
import { useToastStore } from '@/stores/toast'
import type { Question, QuizDetail } from '@/types/assessment'
import { LETTERS } from '@/utils/assessment'

const TRUE_FALSE = [
  { value: 'true', label: '正确', icon: CheckCircleIcon },
  { value: 'false', label: '错误', icon: XCircleIcon }
]
const WRITTEN_HINTS: Record<string, string> = {
  short_answer: '简要写出答案要点',
  recall: '不看资料，用自己的话复述这个概念、定理或方法',
  scenario: '写出分析思路、关键步骤和结论'
}

const props = defineProps<{ detail: QuizDetail; busy?: boolean }>()
const emit = defineEmits<{ submit: [] }>()
const toast = useToastStore()

const quiz = computed(() => props.detail.quiz)
const questions = ref<Question[]>([...props.detail.questions])
const index = ref(Math.min(props.detail.quiz.current_index, Math.max(0, props.detail.questions.length - 1)))
const drafts = reactive<Record<number, string>>({})
for (const question of props.detail.questions) drafts[question.id] = question.response ?? ''
const confirming = ref(false)
let saveTimer: ReturnType<typeof setTimeout> | undefined

watch(() => props.detail, (detail) => {
  questions.value = [...detail.questions]
})

const current = computed(() => questions.value[index.value] ?? null)
const hasDraft = computed(() => !!current.value && !!(drafts[current.value.id] ?? '').trim())
const answeredCount = computed(() => questions.value.filter(q => hasAnswer(q) || q.skipped).length)
const unanswered = computed(() => questions.value.filter(q => !hasAnswer(q)).length)

function hasAnswer(question: Question) {
  return !!(drafts[question.id] ?? '').trim()
}

function navClass(question: Question, i: number) {
  if (i === index.value) return 'border-indigo-600 bg-indigo-600 text-white'
  if (hasAnswer(question)) return 'border-indigo-200 bg-indigo-50 text-indigo-700'
  if (question.skipped) return 'border-dashed border-slate-300 text-slate-400'
  return 'border-slate-200 text-slate-600 hover:bg-slate-50'
}

/** Saves the draft of a question if it differs from what is stored. */
async function persist(question: Question | null) {
  if (!question) return true
  const draft = (drafts[question.id] ?? '').trim()
  if (draft === (question.response ?? '')) return true
  try {
    const saved = await QuizService.answer({ question_id: question.id, response: draft || null, skipped: false })
    const at = questions.value.findIndex(q => q.id === saved.id)
    if (at >= 0) questions.value[at] = saved
    return true
  } catch (e) {
    toast.error(e)
    return false
  }
}

function choose(value: string) {
  if (!current.value) return
  drafts[current.value.id] = value
  persist(current.value)
}

function scheduleSave() {
  clearTimeout(saveTimer)
  const question = current.value
  saveTimer = setTimeout(() => persist(question), 1500)
}
onBeforeUnmount(() => {
  clearTimeout(saveTimer)
  persist(current.value)
})

async function go(target: number) {
  if (target < 0 || target >= questions.value.length || target === index.value) return
  clearTimeout(saveTimer)
  if (!(await persist(current.value))) return
  index.value = target
  QuizService.setPosition(quiz.value.id, target).catch(() => undefined)
}

async function skip() {
  const question = current.value
  if (!question) return
  clearTimeout(saveTimer)
  try {
    if ((drafts[question.id] ?? '').trim()) {
      await persist(question)
    } else {
      const saved = await QuizService.answer({ question_id: question.id, response: null, skipped: true })
      questions.value[index.value] = saved
    }
    await go(index.value + 1)
  } catch (e) {
    toast.error(e)
  }
}

async function requestSubmit() {
  clearTimeout(saveTimer)
  if (!(await persist(current.value))) return
  if (unanswered.value && !quiz.value.allow_skip) {
    toast.error(`还有 ${unanswered.value} 道题没有作答（这份小测不允许跳过）`)
    const first = questions.value.findIndex(q => !hasAnswer(q))
    if (first >= 0) await go(first)
    return
  }
  confirming.value = true
}

function submit() {
  confirming.value = false
  emit('submit')
}
</script>
