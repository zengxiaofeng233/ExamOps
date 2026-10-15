<template>
  <div class="flex h-full min-h-0">
    <aside class="flex w-64 shrink-0 flex-col border-r border-slate-200 bg-surface">
      <div class="space-y-2 border-b border-slate-100 p-3">
        <button class="btn btn-primary w-full" @click="startNew"><PlusIcon class="size-4" />添加网课</button>
        <p class="text-[11px] leading-4 text-slate-500">保存网课链接、时间戳和笔记。视频在浏览器中打开观看，ExamOps 不嵌入网站。</p>
      </div>
      <div class="min-h-0 flex-1 overflow-y-auto p-2">
        <p v-if="loading" class="p-3 text-sm text-slate-400">加载中…</p>
        <p v-else-if="!resources.length && !form" class="p-3 text-sm text-slate-500">还没有网课。</p>
        <ul v-else class="space-y-1" aria-label="网课列表">
          <li v-for="item in resources" :key="item.id">
            <button
              class="block w-full rounded-lg px-3 py-2 text-left"
              :class="item.id === form?.id ? 'bg-indigo-50' : 'hover:bg-slate-50'"
              @click="select(item.id)"
            >
              <p class="truncate text-sm font-medium text-slate-800">{{ item.title }}</p>
              <p class="mt-0.5 truncate text-xs text-slate-500">{{ item.section_title ?? '未关联章节' }}</p>
              <p class="mt-0.5 flex gap-2 text-[11px] text-slate-400">
                <span>{{ item.marks.length }} 个时间戳</span>
                <span :class="hasText(item) ? 'text-emerald-600' : 'text-amber-600'">{{ hasText(item) ? '有文字资料' : '无文字资料' }}</span>
              </p>
            </button>
          </li>
        </ul>
      </div>
    </aside>

    <section class="min-w-0 flex-1 overflow-y-auto">
      <div v-if="form" class="mx-auto max-w-4xl space-y-4 p-5">
        <form class="card space-y-3 p-5" novalidate @submit.prevent="save">
          <div class="flex flex-wrap items-center justify-between gap-2">
            <h2 class="section-title">{{ form.id ? '网课信息' : '添加网课' }}</h2>
            <div v-if="current" class="flex gap-1.5">
              <RouterLink
                :to="{ name: 'learn', params: { id: course.id }, query: { mode: 'learn', resource: current.id } }"
                class="btn btn-soft btn-sm"
                :title="hasText(current) ? '' : '还没有字幕、总结或笔记，AI 无法讲解视频内容'"
              >
                <AcademicCapIcon class="size-4" />AI 学习这节网课
              </RouterLink>
              <button type="button" class="btn btn-ghost btn-sm text-rose-600 hover:text-rose-700" @click="confirmDelete = true">删除</button>
            </div>
          </div>
          <div class="grid gap-3 sm:grid-cols-2">
            <div class="sm:col-span-2">
              <label class="label" for="video-title">名称</label>
              <input id="video-title" v-model="form.title" class="input" maxlength="100" placeholder="例如：高数第二章 导数（B 站）" />
            </div>
            <div class="sm:col-span-2">
              <label class="label" for="video-url">链接</label>
              <div class="flex gap-2">
                <input id="video-url" v-model="form.url" class="input" type="url" placeholder="https://" />
                <button type="button" class="btn btn-secondary shrink-0" :disabled="!form.url.trim()" @click="openUrl(form.url)">
                  <ArrowTopRightOnSquareIcon class="size-4" />打开
                </button>
              </div>
            </div>
            <div v-if="bilibiliVideo" class="space-y-2 sm:col-span-2">
              <BilibiliLogin @logged-in="readBilibili(true)" />
              <div class="flex flex-wrap items-center gap-2">
                <button type="button" class="btn btn-soft btn-sm" :disabled="textJob?.pending" @click="readBilibili(true)">
                  {{ textJob?.pending ? '正在读取网课文字…' : '读取字幕 / AI 总结' }}
                </button>
                <span class="text-xs text-slate-500">只读取链接所指的分 P；未指定时读取 P1。</span>
              </div>
              <p v-if="textJob?.pending" class="hint" role="status">正在后台尝试读取字幕或总结，你可以继续填写、保存或使用其他页面。</p>
              <p v-if="textJob?.error" class="text-xs text-amber-700" role="status">{{ textJob.error }}</p>
              <div v-if="textJob?.result" class="space-y-2 rounded-lg bg-indigo-50 p-3 text-xs text-indigo-700">
                <p role="status">{{ textJob.result.notice }}</p>
                <p v-if="!form.transcript.includes(textJob.result.transcript)">已保留你的现有文字，获取结果尚未加入文字稿。</p>
                <details>
                  <summary class="cursor-pointer">查看获取内容</summary>
                  <pre class="mt-2 max-h-48 overflow-y-auto whitespace-pre-wrap break-words font-sans">{{ textJob.result.transcript }}</pre>
                </details>
                <button v-if="!form.transcript.includes(textJob.result.transcript)" type="button" class="btn btn-secondary btn-sm" @click="appendBilibili">追加到文字稿</button>
              </div>
            </div>
            <div>
              <label class="label" for="video-section">关联章节</label>
              <SectionSelect id="video-section" v-model="form.section_id" :sections="sections" :max-level="2" />
            </div>
            <div>
              <label class="label" for="video-kind">类型</label>
              <select id="video-kind" v-model="form.kind" class="input">
                <option value="video">视频网课</option>
                <option value="link">其他链接</option>
              </select>
            </div>
          </div>
          <div>
            <label class="label" for="video-transcript">字幕 / 文字稿 / AI 总结</label>
            <textarea
              id="video-transcript"
              v-model="form.transcript"
              class="input font-mono text-xs"
              rows="8"
              placeholder="B 站链接会尝试读取字幕或 AI 总结，也可手动粘贴。AI 讲解网课时只使用已保存的文字和笔记。"
            />
            <p class="hint">{{ form.transcript.length.toLocaleString() }} 字{{ transcriptParts ? `，AI 会分 ${transcriptParts} 部分讲解` : '' }}</p>
          </div>
          <div>
            <label class="label" for="video-notes">学习笔记</label>
            <textarea id="video-notes" v-model="form.notes" class="input" rows="4" placeholder="记录老师讲的重点、例题、你的疑问…" />
          </div>
          <div class="flex justify-end gap-2">
            <button v-if="!form.id" type="button" class="btn btn-secondary" @click="cancelNew">取消</button>
            <button type="submit" class="btn btn-primary" :disabled="saving || !form.title.trim()">{{ saving ? '保存中…' : '保存' }}</button>
          </div>
        </form>

        <section v-if="current" class="card p-5" aria-label="时间戳">
          <h2 class="section-title">时间戳</h2>
          <p class="mt-0.5 text-xs text-slate-500">标记视频里的重点位置，点击时间可在浏览器中从该处播放（B 站、YouTube 支持）。</p>
          <ul v-if="current.marks.length" class="mt-3 divide-y divide-slate-100">
            <li v-for="mark in current.marks" :key="mark.id" class="flex items-start gap-3 py-2">
              <button class="shrink-0 rounded bg-indigo-50 px-2 py-0.5 font-mono text-xs font-semibold text-indigo-700 hover:bg-indigo-100" :title="current.url ? '在浏览器中从这里播放' : ''" @click="current.url && openUrl(urlAt(current.url, mark.seconds))">
                {{ formatSeconds(mark.seconds) }}
              </button>
              <div class="min-w-0 flex-1 text-sm">
                <p class="font-medium text-slate-800">{{ mark.label || '（无标题）' }}</p>
                <p v-if="mark.note" class="text-xs whitespace-pre-wrap text-slate-600">{{ mark.note }}</p>
                <p v-if="mark.knowledge_point_id" class="text-[11px] text-slate-400">知识点：{{ pointTitle(mark.knowledge_point_id) }}</p>
              </div>
              <button class="btn btn-ghost btn-sm hover:text-rose-600" title="删除时间戳" @click="removeMark(mark.id)"><TrashIcon class="size-4" /></button>
            </li>
          </ul>
          <form class="mt-3 grid gap-2 sm:grid-cols-[6rem_1fr_1fr_auto]" novalidate @submit.prevent="addMark">
            <input v-model="markForm.time" class="input font-mono" placeholder="12:34" aria-label="时间" />
            <input v-model="markForm.label" class="input" maxlength="60" placeholder="标题，例如：几何意义" aria-label="标题" />
            <select v-model="markForm.knowledge_point_id" class="input" aria-label="知识点">
              <option :value="null">不关联知识点</option>
              <option v-for="point in points" :key="point.id" :value="point.id">{{ point.title }}</option>
            </select>
            <button type="submit" class="btn btn-secondary" :disabled="!markForm.time.trim()">添加</button>
            <textarea v-model="markForm.note" class="input sm:col-span-4" rows="2" placeholder="笔记（可选）" aria-label="笔记" />
          </form>
          <p v-if="markError" class="mt-1 text-xs text-rose-600" role="alert">{{ markError }}</p>
        </section>
      </div>
      <EmptyState v-else-if="!loading" title="添加你看的网课" description="保存链接、关联章节、粘贴字幕或写笔记后，AI 就能结合这节网课和教材为你讲解。">
        <button class="btn btn-primary" @click="startNew">添加网课</button>
      </EmptyState>
    </section>

    <ConfirmDialog
      :open="confirmDelete"
      title="删除网课"
      :message="`确定删除“${current?.title}”及其时间戳和笔记吗？`"
      confirm-text="删除"
      danger
      @cancel="confirmDelete = false"
      @confirm="remove"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { AcademicCapIcon, ArrowTopRightOnSquareIcon, PlusIcon, TrashIcon } from '@heroicons/vue/20/solid'
import SectionSelect from '@/components/study/SectionSelect.vue'
import BilibiliLogin from '@/components/study/BilibiliLogin.vue'
import ConfirmDialog from '@/components/ui/ConfirmDialog.vue'
import EmptyState from '@/components/ui/EmptyState.vue'
import { AppService } from '@/services/app'
import { DocumentService } from '@/services/document'
import { KnowledgeService, ResourceService } from '@/services/study'
import { useToastStore } from '@/stores/toast'
import { isBilibiliVideo, useBilibiliStore } from '@/stores/bilibili'
import type { CourseSection } from '@/types/document'
import type { Course } from '@/types/models'
import type { KnowledgePoint, Resource, ResourceInput } from '@/types/study'
import { errorMessage } from '@/utils/errors'
import { formatSeconds, parseTimestamp, urlAt } from '@/utils/study'

const PART_CHARS = 1500

const props = defineProps<{ course: Course }>()
const route = useRoute()
const router = useRouter()
const toast = useToastStore()
const bilibili = useBilibiliStore()

const resources = ref<Resource[]>([])
const sections = ref<CourseSection[]>([])
const points = ref<KnowledgePoint[]>([])
const loading = ref(true)
const saving = ref(false)
const confirmDelete = ref(false)
const form = ref<ResourceInput | null>(null)
const markForm = reactive({ time: '', label: '', note: '', knowledge_point_id: null as number | null })
const markError = ref('')
const bilibiliVideo = computed(() => isBilibiliVideo(form.value?.url ?? ''))
const textJob = computed(() => bilibili.jobs[form.value?.url.trim() ?? ''])
let textTimer: ReturnType<typeof setTimeout> | undefined
let active = true

watch(() => [form.value, form.value?.url], () => {
  clearTimeout(textTimer)
  if (bilibiliVideo.value && !form.value?.transcript.trim()) {
    textTimer = setTimeout(() => readBilibili(), 700)
  }
})

onBeforeUnmount(() => { active = false; clearTimeout(textTimer) })

async function readBilibili(retry = false) {
  const target = form.value
  if (!target || !isBilibiliVideo(target.url)) return
  clearTimeout(textTimer)
  const url = target.url.trim()
  const before = target.transcript
  const result = await bilibili.fetchText(url, retry)
  // A slow reply must never overwrite another resource or text typed while fetching.
  if (!result || !active || form.value !== target || target.url.trim() !== url) return
  if (!target.title.trim()) target.title = result.title
  if (!before.trim() && target.transcript === before) target.transcript = result.transcript
}

function appendBilibili() {
  const result = textJob.value?.result
  if (!form.value || !result || form.value.transcript.includes(result.transcript)) return
  form.value.transcript = [form.value.transcript, result.transcript].filter(Boolean).join('\n\n')
}

const selectedId = computed(() => Number(route.query.r) || null)
const current = computed(() => resources.value.find(r => r.id === form.value?.id) ?? null)
const transcriptParts = computed(() => {
  const text = form.value?.transcript.trim() ?? ''
  return text ? Math.max(1, Math.ceil(text.length / PART_CHARS)) : 0
})

function hasText(resource: Resource) {
  return !!(resource.transcript.trim() || resource.notes.trim() || resource.marks.length)
}

function pointTitle(id: number) {
  return points.value.find(p => p.id === id)?.title ?? '已删除'
}

function toForm(resource: Resource): ResourceInput {
  return {
    id: resource.id, course_id: props.course.id, kind: resource.kind, title: resource.title, url: resource.url,
    section_id: resource.section_id, transcript: resource.transcript, notes: resource.notes
  }
}

function select(id: number) {
  router.replace({ query: { r: id } })
}

watch(selectedId, (id) => {
  const resource = resources.value.find(r => r.id === id)
  if (resource) form.value = toForm(resource)
})

function startNew() {
  router.replace({ query: {} })
  form.value = { id: null, course_id: props.course.id, kind: 'video', title: '', url: '', section_id: null, transcript: '', notes: '' }
}

function cancelNew() {
  form.value = null
  if (resources.value.length) select(resources.value[0].id)
}

onMounted(async () => {
  try {
    const [list, sectionList, pointList] = await Promise.all([
      ResourceService.list(props.course.id),
      DocumentService.courseSections(props.course.id),
      KnowledgeService.list(props.course.id)
    ])
    resources.value = list
    sections.value = sectionList
    points.value = pointList
    const wanted = resources.value.find(r => r.id === selectedId.value) ?? resources.value[0]
    if (wanted) {
      form.value = toForm(wanted)
      if (wanted.id !== selectedId.value) select(wanted.id)
    }
  } catch (e) {
    toast.error(e)
  } finally {
    loading.value = false
  }
})

function upsert(resource: Resource) {
  const index = resources.value.findIndex(r => r.id === resource.id)
  if (index >= 0) resources.value[index] = resource
  else resources.value.push(resource)
}

async function save() {
  if (!form.value || !form.value.title.trim()) return
  saving.value = true
  try {
    const saved = await ResourceService.save(form.value)
    upsert(saved)
    form.value = toForm(saved)
    if (saved.id !== selectedId.value) select(saved.id)
    toast.success('已保存')
  } catch (e) {
    toast.error(e)
  } finally {
    saving.value = false
  }
}

async function openUrl(url: string) {
  try {
    await AppService.openExternal(url)
  } catch (e) {
    toast.error(e)
  }
}

async function addMark() {
  if (!current.value) return
  markError.value = ''
  const seconds = parseTimestamp(markForm.time)
  if (seconds === null) {
    markError.value = '时间格式应为 分:秒（如 12:34）或 时:分:秒'
    return
  }
  try {
    upsert(await ResourceService.saveMark({
      id: null, resource_id: current.value.id, seconds, label: markForm.label, note: markForm.note, knowledge_point_id: markForm.knowledge_point_id
    }))
    Object.assign(markForm, { time: '', label: '', note: '', knowledge_point_id: null })
  } catch (e) {
    markError.value = errorMessage(e)
  }
}

async function removeMark(id: number) {
  if (!current.value) return
  try {
    upsert(await ResourceService.removeMark(current.value.id, id))
  } catch (e) {
    toast.error(e)
  }
}

async function remove() {
  if (!current.value) return
  try {
    await ResourceService.remove(current.value.id)
    resources.value = resources.value.filter(r => r.id !== current.value?.id)
    confirmDelete.value = false
    form.value = null
    router.replace({ query: resources.value[0] ? { r: resources.value[0].id } : {} })
    if (resources.value[0]) form.value = toForm(resources.value[0])
  } catch (e) {
    toast.error(e)
  }
}
</script>
