<template>
  <div class="flex h-full min-h-0">
    <aside class="flex w-72 shrink-0 flex-col border-r border-slate-200 bg-white">
      <div class="space-y-2.5 border-b border-slate-100 p-3">
        <div class="grid grid-cols-3 gap-1 rounded-lg bg-slate-100 p-1" role="radiogroup" aria-label="学习模式">
          <button
            v-for="item in MODES"
            :key="item"
            role="radio"
            :aria-checked="mode === item"
            class="rounded-md px-1 py-1.5 text-xs font-medium transition-colors"
            :class="mode === item ? 'bg-white text-indigo-700 shadow-sm' : 'text-slate-600 hover:text-slate-900'"
            @click="mode = item"
          >
            {{ MODE_LABELS[item] }}
          </button>
        </div>
        <p class="text-[11px] leading-4 text-slate-500">{{ MODE_DESCRIPTIONS[mode] }}</p>
        <SectionSelect
          v-model="sectionId"
          :sections="sections"
          :placeholder="mode === 'learn' ? '选择要学习的章节' : '全部内容'"
          label="学习章节"
          class="text-xs"
        />
        <button class="btn btn-primary w-full" :disabled="starting || (mode === 'learn' && !sectionId)" @click="start({ mode, section_id: sectionId })">
          <PlayIcon class="size-4" />{{ starting ? '正在打开…' : '开始' }}
        </button>
        <p v-if="!sections.length && !loading" class="text-[11px] text-amber-700">
          还没有已解析的章节，请先在“资料”中导入教材或课件。
        </p>
      </div>
      <div class="min-h-0 flex-1 overflow-y-auto p-2">
        <p v-if="loading" class="p-3 text-sm text-slate-400">加载中…</p>
        <p v-else-if="!conversations.length" class="p-3 text-sm text-slate-500">还没有学习记录。选择模式和章节后点击“开始”，学习进度会自动保存。</p>
        <ul v-else class="space-y-1" aria-label="学习记录">
          <li v-for="item in conversations" :key="item.id" class="group relative">
            <button
              class="block w-full rounded-lg px-3 py-2 pr-8 text-left"
              :class="item.id === activeId ? 'bg-indigo-50' : 'hover:bg-slate-50'"
              @click="select(item.id)"
            >
              <p class="flex items-center gap-1.5">
                <span class="badge shrink-0" :class="MODE_CLASSES[item.mode]">{{ MODE_LABELS[item.mode] }}</span>
                <span class="truncate text-sm font-medium" :class="item.id === activeId ? 'text-indigo-800' : 'text-slate-800'" :title="item.title">{{ subject(item) }}</span>
              </p>
              <div v-if="item.mode === 'learn' && item.progress.parts" class="mt-2 h-1.5 overflow-hidden rounded-full bg-slate-100" aria-hidden="true">
                <div class="h-full rounded-full" :class="item.progress.finished ? 'bg-emerald-500' : 'bg-indigo-500'" :style="{ width: `${percent(item)}%` }" />
              </div>
              <p class="mt-1 text-[11px] text-slate-400">{{ progressText(item) }} · {{ formatRelative(item.updated_at) }}</p>
            </button>
            <button
              class="absolute top-2 right-1.5 hidden rounded p-1 text-slate-400 group-hover:block hover:bg-white hover:text-rose-600"
              title="删除学习记录"
              @click="removing = item"
            >
              <TrashIcon class="size-4" />
            </button>
          </li>
        </ul>
      </div>
    </aside>

    <section class="flex min-w-0 flex-1">
      <div class="min-w-0 flex-1">
        <ChatPanel
          v-if="active"
          :key="active.id"
          ref="panel"
          :conversation-id="active.id"
          :course-id="course.id"
          :placeholder="placeholder"
          @updated="onUpdated"
          @answered="onAnswered"
        >
          <template #header>
            <span class="badge" :class="MODE_CLASSES[active.mode]">{{ MODE_LABELS[active.mode] }}</span>
            <RouterLink v-if="active.resource_id" :to="{ name: 'videos', params: { id: course.id }, query: { r: active.resource_id } }" class="btn btn-ghost btn-sm">网课资料</RouterLink>
            <RouterLink v-else-if="active.section_id && sourceSection" :to="sourceLink" class="btn btn-ghost btn-sm">查看原文</RouterLink>
          </template>
          <template #intro="{ messages }">
            <div v-if="!messages.length" class="card p-5 text-sm text-slate-600">
              <p class="font-medium text-slate-800">{{ introTitle }}</p>
              <p class="mt-1">{{ MODE_DESCRIPTIONS[active.mode as PathMode] }}</p>
              <p v-if="active.mode === 'learn' && active.progress.parts" class="mt-1">内容共分 {{ active.progress.parts }} 个部分，进度会自动保存，下次可以接着学。</p>
              <p v-if="active.resource_id" class="mt-1 text-amber-700">AI 只根据你在“网课”页填写的字幕、笔记和时间戳讲解，不会凭链接猜测视频内容。</p>
              <p class="mt-2 text-xs text-slate-500">讲解中随时可以直接提问，或使用下方的“举个例子”“换种方式讲讲”等按钮。</p>
              <button class="btn btn-primary mt-3" :disabled="panelBusy || !models.configured" @click="act('start')">
                <PlayIcon class="size-4" />{{ startText }}
              </button>
            </div>
          </template>
          <template #after>
            <div v-if="suggestion" class="flex flex-wrap items-center gap-3 rounded-xl border border-indigo-200 bg-indigo-50/70 p-4" role="status">
              <ClipboardDocumentCheckIcon class="size-5 shrink-0 text-indigo-600" />
              <p class="min-w-40 flex-1 text-sm text-indigo-900">{{ suggestion.text }}</p>
              <RouterLink :to="{ name: 'quiz', params: { id: course.id }, query: suggestion.query }" class="btn btn-primary btn-sm">开始小测</RouterLink>
              <button class="btn btn-ghost btn-sm" @click="suggestion = null">稍后</button>
            </div>
          </template>
          <template #actions="{ busy, ready }">
            <div v-if="active.message_count" class="mb-2 flex flex-wrap items-center gap-1.5">
              <button
                v-for="button in actionButtons"
                :key="button.action"
                class="btn btn-sm"
                :class="button.primary ? 'btn-soft' : 'btn-secondary'"
                :disabled="busy || !ready"
                :title="button.title"
                @click="act(button.action)"
              >
                {{ button.label }}
              </button>
              <button
                v-if="active.mode === 'learn' && !active.progress.finished"
                class="btn btn-ghost btn-sm ml-auto"
                :disabled="busy"
                title="不再逐段讲解，直接把本章知识点标记为“学过”"
                @click="finishing = true"
              >
                <CheckCircleIcon class="size-4" />标记已学完
              </button>
            </div>
          </template>
        </ChatPanel>
        <EmptyState v-else-if="!loading" title="选择学习模式和章节" description="从零学习按章节分段讲解；正常复习用回忆题巩固；考前冲刺直击考点。学习进度会保存在本机。" />
      </div>

      <aside v-if="active?.mode === 'learn' && outline.length" class="hidden w-60 shrink-0 overflow-y-auto border-l border-slate-200 bg-white p-3 lg:block" aria-label="本次学习内容">
        <p class="text-xs font-semibold text-slate-500">本次内容 · 已讲 {{ taughtCount }}/{{ outline.length }}</p>
        <p class="mt-0.5 text-[11px] text-slate-400">点击任一部分可以直接讲解它</p>
        <ol class="mt-2 space-y-1">
          <li v-for="part in outline" :key="part.index">
            <button
              class="w-full rounded-md border px-2 py-1.5 text-left text-xs transition-colors disabled:cursor-not-allowed"
              :class="partClass(part)"
              :disabled="panelBusy || !models.configured"
              @click="act('goto', part.index)"
            >
              <span class="flex items-center gap-1 font-medium">
                <CheckIcon v-if="part.status === 'taught'" class="size-3.5 text-emerald-600" />
                <ForwardIcon v-else-if="part.status === 'skipped'" class="size-3.5 text-slate-400" />
                <ArrowRightIcon v-else-if="part.status === 'next'" class="size-3.5 text-indigo-600" />
                第 {{ part.index + 1 }} 部分
                <span class="ml-auto font-normal text-slate-400">{{ part.chars }} 字</span>
              </span>
              <span class="mt-0.5 block truncate" :title="part.title">{{ part.title }}</span>
              <span class="block text-[11px] text-slate-400">{{ part.loc_label }}</span>
            </button>
          </li>
        </ol>
        <p v-if="active.progress.finished" class="mt-3 rounded-md bg-emerald-50 p-2 text-xs text-emerald-700">已学完，相关知识点已标记为“学过”。去做一次小测，检验是否真正掌握。</p>
      </aside>
    </section>

    <ConfirmDialog
      :open="!!removing"
      title="删除学习记录"
      :message="`确定删除“${removing?.title}”的对话和学习进度吗？知识点的学习状态不受影响。`"
      confirm-text="删除"
      danger
      @cancel="removing = null"
      @confirm="remove"
    />
    <ConfirmDialog
      :open="finishing"
      title="标记已学完"
      message="将本次内容标记为已学完，相关知识点会记为“学过”（不是“已掌握”，掌握程度由测验决定）。关联的学习任务也会完成。"
      confirm-text="标记已学完"
      @cancel="finishing = false"
      @confirm="finishLesson"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { ArrowRightIcon, CheckCircleIcon, CheckIcon, ClipboardDocumentCheckIcon, ForwardIcon, PlayIcon, TrashIcon } from '@heroicons/vue/20/solid'
import ChatPanel from '@/components/chat/ChatPanel.vue'
import SectionSelect from '@/components/study/SectionSelect.vue'
import ConfirmDialog from '@/components/ui/ConfirmDialog.vue'
import EmptyState from '@/components/ui/EmptyState.vue'
import { ChatService, TeachingService } from '@/services/ai'
import { DocumentService } from '@/services/document'
import { useAppStore } from '@/stores/app'
import { useModelStore } from '@/stores/models'
import { useToastStore } from '@/stores/toast'
import type { Conversation, Message } from '@/types/ai'
import type { CourseSection } from '@/types/document'
import type { Course } from '@/types/models'
import type { LessonPart, PathMode, StartTeachingInput, TeachAction } from '@/types/study'
import { formatRelative } from '@/utils/format'
import { MODE_CLASSES, MODE_DESCRIPTIONS, MODE_LABELS } from '@/utils/study'

const MODES: PathMode[] = ['learn', 'review', 'sprint']

const props = defineProps<{ course: Course }>()
const route = useRoute()
const router = useRouter()
const toast = useToastStore()
const models = useModelStore()
const app = useAppStore()
/** Quiz offered after a lesson part or a whole chapter (per the quiz settings). */
const suggestion = ref<{ text: string; query: Record<string, string | number> } | null>(null)

const conversations = ref<Conversation[]>([])
const sections = ref<CourseSection[]>([])
const outline = ref<LessonPart[]>([])
const loading = ref(true)
const starting = ref(false)
const finishing = ref(false)
const removing = ref<Conversation | null>(null)
const mode = ref<PathMode>('learn')
const sectionId = ref<number | null>(null)
const panel = ref<InstanceType<typeof ChatPanel>>()

const activeId = computed(() => Number(route.query.c) || null)
const active = computed(() => conversations.value.find(c => c.id === activeId.value) ?? null)
const panelBusy = computed(() => !!panel.value?.busy)
const taughtCount = computed(() => outline.value.filter(p => p.status === 'taught').length)
const sourceSection = computed(() => sections.value.find(s => s.id === active.value?.section_id) ?? null)
const sourceLink = computed(() => ({
  name: 'documents',
  params: { id: props.course.id },
  query: sourceSection.value ? { doc: sourceSection.value.document_id, page: sourceSection.value.page_start } : {}
}))

const introTitle = computed(() => {
  const item = active.value
  if (!item) return ''
  if (item.resource_id) return `用 AI 学习网课《${item.resource_title ?? ''}》`
  return `${MODE_LABELS[item.mode]}：${item.section_title ?? '全部内容'}`
})
const startText = computed(() => ({ learn: active.value?.resource_id ? '开始学习这节网课' : '开始学习本章', review: '开始复习', sprint: '开始考前冲刺', qa: '开始' })[active.value?.mode ?? 'learn'])
const placeholder = computed(() => ({
  learn: '有疑问随时问，例如：这个公式为什么成立？',
  review: '在这里回答复习题，或提出问题',
  sprint: '问一个考试相关的问题，例如：这类题怎么拿满分？',
  qa: undefined
})[active.value?.mode ?? 'qa'])

interface ActionButton { action: TeachAction; label: string; title?: string; primary?: boolean }
const actionButtons = computed<ActionButton[]>(() => {
  const item = active.value
  if (!item) return []
  if (item.mode === 'learn') {
    const atEnd = item.progress.next >= item.progress.parts
    const list: ActionButton[] = []
    if (!item.progress.finished) {
      list.push(atEnd
        ? { action: 'continue', label: '讲完了，总结全部内容', primary: true }
        : { action: 'continue', label: '继续讲解下一部分', primary: true })
    }
    list.push({ action: 'example', label: '举个例子' }, { action: 'rephrase', label: '换种方式讲讲' })
    if (!atEnd && !item.progress.finished) list.push({ action: 'skip', label: '跳过下一部分', title: '下一部分已经会了，直接讲再下一部分' })
    list.push({ action: 'summary', label: '总结要点' })
    return list
  }
  if (item.mode === 'review') {
    return [
      { action: 'continue', label: '再来一组复习题', primary: true },
      { action: 'example', label: '举个例子' },
      { action: 'summary', label: '总结要点' }
    ]
  }
  return [
    { action: 'continue', label: '讲一道典型题', primary: true },
    { action: 'rephrase', label: '换种方式讲讲' },
    { action: 'summary', label: '总结要点' }
  ]
})

function subject(item: Conversation) {
  return item.title.replace(/^(学习|复习|冲刺)｜/, '') || MODE_LABELS[item.mode]
}

function percent(item: Conversation) {
  if (item.progress.finished) return 100
  const done = new Set([...item.progress.taught, ...item.progress.skipped]).size
  return item.progress.parts ? Math.round((done / item.progress.parts) * 100) : 0
}

function progressText(item: Conversation) {
  if (item.mode !== 'learn') return `${item.message_count} 条消息`
  if (item.progress.finished) return '已学完'
  if (!item.progress.parts) return '暂无可讲内容'
  return `已讲 ${item.progress.taught.length}/${item.progress.parts}`
}

function partClass(part: LessonPart) {
  return {
    taught: 'border-emerald-100 bg-emerald-50/50 text-slate-700 hover:bg-emerald-50',
    skipped: 'border-slate-100 text-slate-400 hover:bg-slate-50',
    next: 'border-indigo-300 bg-indigo-50 text-indigo-800 hover:bg-indigo-100',
    todo: 'border-slate-100 text-slate-600 hover:bg-slate-50'
  }[part.status]
}

async function refresh() {
  conversations.value = (await ChatService.list(props.course.id)).filter(c => c.mode !== 'qa')
}

async function loadOutline() {
  outline.value = active.value?.mode === 'learn' ? await TeachingService.outline(active.value.id).catch(() => []) : []
}

function select(id: number) {
  router.replace({ query: { c: id } })
}

async function start(input: Omit<StartTeachingInput, 'course_id'>) {
  starting.value = true
  try {
    const conversation = await TeachingService.start({ course_id: props.course.id, ...input })
    const index = conversations.value.findIndex(c => c.id === conversation.id)
    if (index >= 0) conversations.value[index] = conversation
    else conversations.value.unshift(conversation)
    select(conversation.id)
  } catch (e) {
    toast.error(e)
  } finally {
    starting.value = false
  }
}

function act(action: TeachAction, target?: number) {
  panel.value?.send('', { action, target })
}

function onUpdated(conversation: Conversation) {
  const index = conversations.value.findIndex(c => c.id === conversation.id)
  const before = index >= 0 ? conversations.value[index] : null
  if (index >= 0) conversations.value[index] = conversation
  conversations.value.sort((a, b) => b.updated_at.localeCompare(a.updated_at) || b.id - a.id)
  if (before && !before.progress.finished && conversation.progress.finished) {
    toast.success('已学完！相关知识点已记为“学过”，可以去做小测检验掌握程度')
    if (app.settings.quiz.trigger === 'chapter' && conversation.section_id) {
      suggestion.value = {
        text: '本章学完了，来做一次章节小测，检验是否真正掌握？',
        query: { auto: 1, origin: 'chapter', section: conversation.section_id }
      }
    }
  }
  if (!before || JSON.stringify(before.progress) !== JSON.stringify(conversation.progress) || !outline.value.length) loadOutline()
}

function onAnswered(message: Message) {
  const covered = message.meta.plan?.covered_points ?? []
  if (message.status !== 'done' || active.value?.mode !== 'learn' || app.settings.quiz.trigger !== 'point' || !covered.length) return
  suggestion.value = {
    text: `这一部分讲完了 ${covered.length} 个知识点，做个小测检验一下？`,
    query: { auto: 1, origin: 'point', points: covered.join(','), count: Math.min(app.settings.quiz.question_count, covered.length * 2) }
  }
}

async function finishLesson() {
  if (!active.value) return
  try {
    const conversation = await TeachingService.markFinished(active.value.id)
    finishing.value = false
    onUpdated(conversation)
    await panel.value?.reload()
  } catch (e) {
    toast.error(e)
  }
}

async function remove() {
  if (!removing.value) return
  const id = removing.value.id
  try {
    await ChatService.remove(id)
    removing.value = null
    await refresh()
    if (activeId.value === id) router.replace({ query: conversations.value[0] ? { c: conversations.value[0].id } : {} })
  } catch (e) {
    toast.error(e)
  }
}

/** The start form follows the open conversation (mode and chapter). */
function syncForm() {
  if (!active.value) return
  mode.value = active.value.mode === 'qa' ? 'learn' : active.value.mode
  sectionId.value = active.value.section_id
}

watch(activeId, () => {
  outline.value = []
  suggestion.value = null
  syncForm()
  loadOutline()
})

onMounted(async () => {
  models.load().catch(() => undefined)
  if (!app.ready) app.load().catch(() => undefined)
  try {
    const [, list] = await Promise.all([refresh(), DocumentService.courseSections(props.course.id)])
    sections.value = list
    // Links from learning steps, the reader and the video page start a lesson directly.
    const query = route.query
    if (query.mode || query.section || query.resource) {
      const wanted = (MODES as string[]).includes(String(query.mode)) ? (String(query.mode) as PathMode) : 'learn'
      mode.value = wanted
      sectionId.value = Number(query.section) || null
      await start({
        mode: wanted,
        section_id: Number(query.section) || null,
        resource_id: Number(query.resource) || null,
        step_id: Number(query.step) || null
      })
    } else if (!activeId.value && conversations.value.length) {
      select(conversations.value[0].id)
    } else {
      syncForm()
      await loadOutline()
    }
  } catch (e) {
    toast.error(e)
  } finally {
    loading.value = false
  }
})
</script>
