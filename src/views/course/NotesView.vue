<template>
  <div class="flex h-full min-h-0">
    <aside class="flex w-72 shrink-0 flex-col border-r border-slate-200 bg-white">
      <div class="space-y-2 border-b border-slate-100 p-3">
        <button class="btn btn-primary w-full" @click="startNew({})"><PlusIcon class="size-4" />新建笔记</button>
        <input v-model="keyword" type="search" class="input py-1.5 text-xs" placeholder="搜索笔记" aria-label="搜索笔记" />
      </div>
      <div class="min-h-0 flex-1 overflow-y-auto p-2">
        <p v-if="loading" class="p-3 text-sm text-slate-400">加载中…</p>
        <p v-else-if="!notes.length" class="p-3 text-sm text-slate-500">还没有笔记。阅读资料时点击“记笔记”，笔记会自动关联到当前页。</p>
        <ul v-else class="space-y-1" aria-label="笔记列表">
          <li v-for="note in visible" :key="note.id">
            <button
              class="block w-full rounded-lg px-3 py-2 text-left"
              :class="note.id === form?.id ? 'bg-indigo-50' : 'hover:bg-slate-50'"
              @click="select(note.id)"
            >
              <p class="truncate text-sm font-medium text-slate-800">{{ note.title }}</p>
              <p class="mt-0.5 line-clamp-2 text-xs text-slate-500">{{ note.content }}</p>
              <p class="mt-0.5 truncate text-[11px] text-slate-400">{{ linkLabel(note) || '未关联' }} · {{ formatRelative(note.updated_at) }}</p>
            </button>
          </li>
        </ul>
      </div>
    </aside>

    <section class="min-w-0 flex-1 overflow-y-auto">
      <form v-if="form" class="mx-auto max-w-3xl space-y-3 p-5" novalidate @submit.prevent="save">
        <div class="flex flex-wrap items-center gap-2">
          <input v-model="form.title" class="input flex-1 text-base font-medium" maxlength="80" placeholder="标题（留空则取正文第一行）" aria-label="标题" />
          <button type="button" class="btn btn-ghost btn-sm" :aria-pressed="preview" @click="preview = !preview">{{ preview ? '编辑' : '预览' }}</button>
        </div>
        <textarea v-if="!preview" v-model="form.content" class="input min-h-72 font-[inherit] leading-7" rows="14" placeholder="支持 Markdown 和 LaTeX 公式（$...$）" aria-label="笔记内容" />
        <div v-else class="card min-h-72 p-4">
          <MarkdownContent v-if="form.content.trim()" :text="form.content" />
          <p v-else class="text-sm text-slate-400">（空）</p>
        </div>
        <fieldset class="card grid gap-3 p-4 sm:grid-cols-2">
          <legend class="px-1 text-xs font-medium text-slate-500">关联位置（可选）</legend>
          <div>
            <label class="label" for="note-document">资料</label>
            <select id="note-document" v-model="form.document_id" class="input">
              <option :value="null">不关联</option>
              <option v-for="doc in documents" :key="doc.id" :value="doc.id">{{ doc.filename }}</option>
            </select>
          </div>
          <div>
            <label class="label" for="note-page">{{ unitName }}</label>
            <input id="note-page" v-model.number="form.page_number" type="number" min="1" step="1" class="input" :disabled="!form.document_id" />
          </div>
          <div>
            <label class="label" for="note-section">章节</label>
            <SectionSelect id="note-section" v-model="form.section_id" :sections="sections" />
          </div>
          <div>
            <label class="label" for="note-point">知识点</label>
            <select id="note-point" v-model="form.knowledge_point_id" class="input">
              <option :value="null">不关联</option>
              <option v-for="point in points" :key="point.id" :value="point.id">{{ point.title }}</option>
            </select>
          </div>
        </fieldset>
        <div class="flex flex-wrap items-center gap-2">
          <RouterLink v-if="form.document_id" :to="sourceLink" class="btn btn-ghost btn-sm"><BookOpenIcon class="size-4" />打开原文</RouterLink>
          <span v-if="current" class="text-xs text-slate-400">更新于 {{ formatDateTime(current.updated_at) }}</span>
          <div class="ml-auto flex gap-2">
            <button v-if="current" type="button" class="btn btn-ghost text-rose-600 hover:text-rose-700" @click="confirmDelete = true">删除</button>
            <button type="submit" class="btn btn-primary" :disabled="saving || (!form.title.trim() && !form.content.trim())">{{ saving ? '保存中…' : '保存' }}</button>
          </div>
        </div>
      </form>
      <EmptyState v-else-if="!loading" title="选择或新建一条笔记" description="笔记保存在本机，可以关联到资料页、章节或知识点，方便复习时回看。">
        <button class="btn btn-primary" @click="startNew({})">新建笔记</button>
      </EmptyState>
    </section>

    <ConfirmDialog
      :open="confirmDelete"
      title="删除笔记"
      :message="`确定删除“${current?.title}”吗？`"
      confirm-text="删除"
      danger
      @cancel="confirmDelete = false"
      @confirm="remove"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { onBeforeRouteLeave, useRoute, useRouter } from 'vue-router'
import { BookOpenIcon, PlusIcon } from '@heroicons/vue/20/solid'
import MarkdownContent from '@/components/chat/MarkdownContent.vue'
import SectionSelect from '@/components/study/SectionSelect.vue'
import ConfirmDialog from '@/components/ui/ConfirmDialog.vue'
import EmptyState from '@/components/ui/EmptyState.vue'
import { DocumentService } from '@/services/document'
import { KnowledgeService, NoteService } from '@/services/study'
import { useToastStore } from '@/stores/toast'
import type { CourseSection, Document } from '@/types/document'
import type { Course } from '@/types/models'
import type { KnowledgePoint, Note, NoteInput } from '@/types/study'
import { UNIT_NAMES } from '@/utils/documents'
import { formatDateTime, formatRelative } from '@/utils/format'

const props = defineProps<{ course: Course }>()
const route = useRoute()
const router = useRouter()
const toast = useToastStore()

const notes = ref<Note[]>([])
const documents = ref<Document[]>([])
const sections = ref<CourseSection[]>([])
const points = ref<KnowledgePoint[]>([])
const loading = ref(true)
const saving = ref(false)
const preview = ref(false)
const confirmDelete = ref(false)
const keyword = ref('')
const form = ref<NoteInput | null>(null)
let savedSnapshot = ''

const selectedId = computed(() => Number(route.query.n) || null)
const current = computed(() => notes.value.find(n => n.id === form.value?.id) ?? null)
const visible = computed(() => {
  const text = keyword.value.trim().toLowerCase()
  return text ? notes.value.filter(n => n.title.toLowerCase().includes(text) || n.content.toLowerCase().includes(text)) : notes.value
})
const unitName = computed(() => {
  const kind = documents.value.find(d => d.id === form.value?.document_id)?.loc_kind ?? 'page'
  return kind === 'line' ? '位置（第几个文本块）' : `位置（第几${UNIT_NAMES[kind]}）`
})
const sourceLink = computed(() => ({
  name: 'documents',
  params: { id: props.course.id },
  query: { doc: form.value?.document_id ?? undefined, ...(form.value?.page_number ? { page: form.value.page_number } : {}) }
}))
const dirty = computed(() => !!form.value && JSON.stringify(form.value) !== savedSnapshot)

function linkLabel(note: Note) {
  const parts = []
  if (note.document_name) parts.push(note.page_number ? `${note.document_name} · ${note.page_number}` : note.document_name)
  else if (note.section_title) parts.push(note.section_title)
  if (note.knowledge_point_title) parts.push(note.knowledge_point_title)
  return parts.join(' · ')
}

function toForm(note: Note): NoteInput {
  return {
    id: note.id, course_id: props.course.id, title: note.title, content: note.content, document_id: note.document_id,
    page_number: note.page_number, section_id: note.section_id, knowledge_point_id: note.knowledge_point_id
  }
}

function setForm(value: NoteInput | null) {
  form.value = value
  savedSnapshot = JSON.stringify(value)
  preview.value = false
}

async function select(id: number) {
  await persist(true)
  router.replace({ query: { n: id } })
}

watch(selectedId, (id) => {
  const note = notes.value.find(n => n.id === id)
  if (note) setForm(toForm(note))
})

async function startNew(link: Partial<NoteInput>) {
  await persist(true)
  setForm({
    id: null, course_id: props.course.id, title: '', content: '', document_id: null, page_number: null, section_id: null,
    knowledge_point_id: null, ...link
  })
}

onMounted(async () => {
  try {
    const [list, documentList, sectionList, pointList] = await Promise.all([
      NoteService.list(props.course.id),
      DocumentService.list(props.course.id),
      DocumentService.courseSections(props.course.id),
      KnowledgeService.list(props.course.id)
    ])
    notes.value = list
    documents.value = documentList.filter(d => d.status === 'completed')
    sections.value = sectionList
    points.value = pointList
    const query = route.query
    if (query.new) {
      // "记笔记" from the reader or the knowledge list links the new note.
      startNew({
        document_id: Number(query.doc) || null,
        page_number: Number(query.page) || null,
        section_id: Number(query.section) || null,
        knowledge_point_id: Number(query.point) || null
      })
      router.replace({ query: {} })
    } else {
      const wanted = notes.value.find(n => n.id === selectedId.value) ?? notes.value[0]
      if (wanted) {
        setForm(toForm(wanted))
        if (wanted.id !== selectedId.value) select(wanted.id)
      }
    }
  } catch (e) {
    toast.error(e)
  } finally {
    loading.value = false
  }
})

// Unsaved edits are kept when leaving the page or switching notes.
onBeforeRouteLeave(async () => {
  await persist(true)
  return true
})

/** Saves the open note; `silent` skips messages and ignores empty drafts. */
async function persist(silent: boolean) {
  if (!form.value || (silent && !dirty.value)) return
  if (silent && !form.value.title.trim() && !form.value.content.trim()) return
  saving.value = true
  try {
    const input = { ...form.value, page_number: form.value.document_id ? form.value.page_number || null : null }
    const saved = await NoteService.save(input)
    const index = notes.value.findIndex(n => n.id === saved.id)
    if (index >= 0) notes.value.splice(index, 1)
    notes.value.unshift(saved)
    setForm(toForm(saved))
    if (!silent) {
      if (saved.id !== selectedId.value) router.replace({ query: { n: saved.id } })
      toast.success('笔记已保存')
    }
  } catch (e) {
    toast.error(e)
  } finally {
    saving.value = false
  }
}

const save = () => persist(false)

async function remove() {
  if (!current.value) return
  try {
    await NoteService.remove(current.value.id)
    notes.value = notes.value.filter(n => n.id !== current.value?.id)
    confirmDelete.value = false
    setForm(notes.value[0] ? toForm(notes.value[0]) : null)
    router.replace({ query: notes.value[0] ? { n: notes.value[0].id } : {} })
  } catch (e) {
    toast.error(e)
  }
}
</script>
