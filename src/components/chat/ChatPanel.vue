<template>
  <div class="flex h-full min-h-0 flex-col">
    <header class="flex flex-wrap items-center gap-2 border-b border-slate-200 bg-white px-4 py-2.5">
      <input
        v-if="conversation"
        v-model="title"
        class="min-w-0 flex-1 rounded-md border border-transparent px-2 py-1 text-sm font-medium text-slate-900 hover:border-slate-200 focus:border-indigo-400 focus:outline-none"
        placeholder="未命名对话"
        aria-label="对话标题"
        @change="saveTitle"
      />
      <slot name="header" :conversation="conversation" />
      <label class="flex items-center gap-1 text-xs text-slate-500">
        上下文
        <select v-model="contextMode" class="rounded-md border border-slate-200 bg-white px-1.5 py-1 text-xs text-slate-700" @change="saveContextMode">
          <option value="auto">自动</option>
          <option value="full">全文</option>
          <option value="rag">检索</option>
        </select>
      </label>
      <div v-if="showScope" ref="scopeBox" class="relative">
        <button class="rounded-md border border-slate-200 bg-white px-2 py-1 text-xs text-slate-700 hover:bg-slate-50" :aria-expanded="scopeOpen" @click="scopeOpen = !scopeOpen">
          资料：{{ scopeLabel }}
        </button>
        <div v-if="scopeOpen" class="absolute right-0 z-20 mt-1 w-72 rounded-lg border border-slate-200 bg-white p-2 shadow-lg">
          <label class="flex items-center gap-2 rounded px-2 py-1.5 text-sm hover:bg-slate-50">
            <input type="checkbox" class="accent-indigo-600" :checked="!scopeDocs.length" @change="setScope([])" />全部资料
          </label>
          <label v-for="doc in documents" :key="doc.id" class="flex items-center gap-2 rounded px-2 py-1.5 text-sm hover:bg-slate-50">
            <input type="checkbox" class="accent-indigo-600" :checked="scopeDocs.includes(doc.id)" @change="toggleDoc(doc.id)" />
            <span class="truncate" :title="doc.filename">{{ doc.filename }}</span>
          </label>
        </div>
      </div>
      <label v-if="models.profiles.length > 1" class="flex items-center gap-1 text-xs text-slate-500">
        模型
        <select v-model="profileId" class="max-w-36 rounded-md border border-slate-200 bg-white px-1.5 py-1 text-xs text-slate-700" @change="saveProfile">
          <option :value="0">默认（{{ models.defaultProfile?.name }}）</option>
          <option v-for="p in models.profiles" :key="p.id" :value="p.id">{{ p.name }}</option>
        </select>
      </label>
    </header>

    <div v-if="!models.configured && models.loaded" class="border-b border-amber-200 bg-amber-50 px-4 py-2 text-sm text-amber-800">
      尚未配置 AI 模型。<RouterLink to="/settings?section=models" class="font-medium underline">前往设置添加</RouterLink>后即可提问。
    </div>

    <div ref="scroller" class="min-h-0 flex-1 overflow-y-auto px-4 py-5" @scroll="onScroll">
      <div class="mx-auto max-w-3xl space-y-6">
        <slot name="intro" :conversation="conversation" :messages="messages" />
        <p v-if="loading" class="text-sm text-slate-400">加载对话…</p>
        <ChatMessage
          v-for="(message, index) in messages"
          :key="message.id"
          :message="message"
          :is-last="index === messages.length - 1"
          :busy="busy"
          :live-reasoning="message.id === streamingId ? reasoning : undefined"
          @open-source="openSource"
          @regenerate="regenerate"
        />
        <slot name="after" :conversation="conversation" :messages="messages" :busy="busy" />
      </div>
    </div>

    <div class="border-t border-slate-200 bg-slate-50 px-4 py-3">
      <div class="mx-auto max-w-3xl">
        <slot name="actions" :busy="busy" :send="send" :conversation="conversation" />
        <ChatComposer
          ref="composer"
          :busy="busy"
          :disabled="!conversation || !models.configured"
          :placeholder="placeholder ?? '向 AI 提问，例如：导数的几何意义是什么？'"
          @send="send"
          @stop="stop"
        />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import ChatMessage from './ChatMessage.vue'
import ChatComposer from './ChatComposer.vue'
import { ChatService } from '@/services/ai'
import { DocumentService } from '@/services/document'
import { useModelStore } from '@/stores/models'
import { useToastStore } from '@/stores/toast'
import type { ChatEvent, ContextMode, Conversation, Message, SourceRef } from '@/types/ai'
import type { Document } from '@/types/document'

const props = withDefaults(defineProps<{
  conversationId: number
  courseId: number
  placeholder?: string
  showScope?: boolean
  /** Optional sender for teaching actions (defaults to plain questions). */
  sender?: (conversationId: number, content: string, onEvent: (e: ChatEvent) => void, regenerate: boolean) => Promise<Message>
}>(), { showScope: true })
const emit = defineEmits<{ updated: [conversation: Conversation]; answered: [message: Message] }>()

const router = useRouter()
const models = useModelStore()
const toast = useToastStore()
const conversation = ref<Conversation | null>(null)
const messages = ref<Message[]>([])
const documents = ref<Document[]>([])
const loading = ref(true)
const busy = ref(false)
const title = ref('')
const contextMode = ref<ContextMode>('auto')
const profileId = ref(0)
const scopeOpen = ref(false)
const scopeBox = ref<HTMLElement>()
function closeScope(event: MouseEvent) {
  if (scopeOpen.value && scopeBox.value && !scopeBox.value.contains(event.target as Node)) scopeOpen.value = false
}
const scopeDocs = ref<number[]>([])
const scroller = ref<HTMLElement>()
const composer = ref<InstanceType<typeof ChatComposer>>()
const streamingId = ref<number | null>(null)
const requestId = ref<string | null>(null)
const reasoning = ref('')
let stickToBottom = true

const scopeLabel = computed(() => {
  if (!scopeDocs.value.length) return '全部'
  if (scopeDocs.value.length === 1) return documents.value.find(d => d.id === scopeDocs.value[0])?.filename ?? '1 份'
  return `${scopeDocs.value.length} 份`
})

function applyConversation(value: Conversation) {
  conversation.value = value
  title.value = value.title
  contextMode.value = value.context_mode
  profileId.value = value.profile_id ?? 0
  scopeDocs.value = [...value.scope.document_ids]
  emit('updated', value)
}

async function load() {
  loading.value = true
  try {
    const detail = await ChatService.get(props.conversationId)
    applyConversation(detail.conversation)
    messages.value = detail.messages
    await scrollToBottom(true)
  } catch (e) {
    toast.error(e)
  } finally {
    loading.value = false
  }
}

onMounted(async () => {
  document.addEventListener('mousedown', closeScope)
  models.load().catch(() => undefined)
  DocumentService.list(props.courseId).then(list => (documents.value = list.filter(d => d.status === 'completed'))).catch(() => undefined)
  await load()
})
watch(() => props.conversationId, load)
onBeforeUnmount(() => {
  document.removeEventListener('mousedown', closeScope)
  // Leaving the page stops the answer; the partial text is kept.
  if (requestId.value) ChatService.cancel(requestId.value).catch(() => undefined)
})

function onScroll() {
  const el = scroller.value
  if (el) stickToBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 80
}

async function scrollToBottom(force = false) {
  await nextTick()
  if (scroller.value && (force || stickToBottom)) scroller.value.scrollTop = scroller.value.scrollHeight
}

async function update(patch: Parameters<typeof ChatService.update>[0]) {
  try {
    applyConversation(await ChatService.update(patch))
  } catch (e) {
    toast.error(e)
  }
}

const saveTitle = () => conversation.value && update({ id: conversation.value.id, title: title.value })
const saveContextMode = () => conversation.value && update({ id: conversation.value.id, context_mode: contextMode.value })
const saveProfile = () => conversation.value && update({ id: conversation.value.id, profile_id: profileId.value })
function setScope(ids: number[]) {
  scopeDocs.value = ids
  if (conversation.value) update({ id: conversation.value.id, scope: { document_ids: ids, section_ids: conversation.value.scope.section_ids } })
}
function toggleDoc(id: number) {
  setScope(scopeDocs.value.includes(id) ? scopeDocs.value.filter(d => d !== id) : [...scopeDocs.value, id])
}

function onEvent(event: ChatEvent) {
  switch (event.type) {
    case 'started':
      requestId.value = event.request_id
      if (event.user_message) messages.value.push(event.user_message)
      messages.value.push(event.assistant_message)
      streamingId.value = event.assistant_message.id
      stickToBottom = true
      scrollToBottom(true)
      break
    case 'delta': {
      const message = messages.value.find(m => m.id === streamingId.value)
      if (message) message.content += event.text
      scrollToBottom()
      break
    }
    case 'reasoning':
      reasoning.value += event.text
      break
    case 'done': {
      const index = messages.value.findIndex(m => m.id === event.message.id)
      if (index >= 0) messages.value[index] = event.message
      break
    }
  }
}

async function run(content: string, regenerate: boolean) {
  if (!conversation.value || busy.value) return
  busy.value = true
  reasoning.value = ''
  if (regenerate) {
    // The last answer is replaced by the new one.
    const lastUser = [...messages.value].reverse().find(m => m.role === 'user')
    if (lastUser) messages.value = messages.value.filter(m => m.id <= lastUser.id)
  }
  try {
    const sender = props.sender ?? ChatService.send
    const message = await sender(conversation.value.id, content, onEvent, regenerate)
    emit('answered', message)
    const detail = await ChatService.get(conversation.value.id)
    applyConversation(detail.conversation)
  } catch (e) {
    toast.error(e)
    // Re-sync in case part of the turn was stored.
    const detail = await ChatService.get(conversation.value.id).catch(() => null)
    if (detail) messages.value = detail.messages
  } finally {
    busy.value = false
    streamingId.value = null
    requestId.value = null
    reasoning.value = ''
    composer.value?.focus()
  }
}

function send(content: string) {
  return run(content, false)
}

function regenerate() {
  return run('', true)
}

async function stop() {
  if (requestId.value) await ChatService.cancel(requestId.value).catch((e) => toast.error(e))
}

function openSource(source: SourceRef) {
  router.push({ name: 'documents', params: { id: props.courseId }, query: { doc: source.document_id, page: source.page_start } })
}

defineExpose({ send, reload: load, setDraft: (text: string) => composer.value?.setText(text) })
</script>
