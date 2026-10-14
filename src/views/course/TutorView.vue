<template>
  <div class="flex h-full min-h-0">
    <aside class="flex w-64 shrink-0 flex-col border-r border-slate-200 bg-white">
      <div class="border-b border-slate-100 p-3">
        <button class="btn btn-primary w-full" :disabled="creating" @click="createConversation()">
          <PlusIcon class="size-4" />新建对话
        </button>
      </div>
      <div class="min-h-0 flex-1 overflow-y-auto p-2">
        <p v-if="loading" class="p-3 text-sm text-slate-400">加载中…</p>
        <p v-else-if="!conversations.length" class="p-3 text-sm text-slate-500">还没有对话。新建一个对话，向 AI 提问课程内容。</p>
        <ul v-else class="space-y-1">
          <li v-for="item in conversations" :key="item.id" class="group relative">
            <button
              class="block w-full rounded-lg px-3 py-2 pr-8 text-left"
              :class="item.id === activeId ? 'bg-indigo-50' : 'hover:bg-slate-50'"
              @click="select(item.id)"
            >
              <p class="truncate text-sm font-medium" :class="item.id === activeId ? 'text-indigo-800' : 'text-slate-800'">{{ item.title || '新对话' }}</p>
              <p class="mt-0.5 truncate text-xs text-slate-500">{{ item.last_message || '暂无消息' }}</p>
              <p class="text-[11px] text-slate-400">{{ formatRelative(item.updated_at) }}</p>
            </button>
            <button
              class="absolute top-2 right-1.5 hidden rounded p-1 text-slate-400 group-hover:block hover:bg-white hover:text-rose-600"
              title="删除对话"
              @click="removing = item"
            >
              <TrashIcon class="size-4" />
            </button>
          </li>
        </ul>
      </div>
    </aside>
    <section class="min-w-0 flex-1">
      <ChatPanel v-if="activeId" :key="activeId" :conversation-id="activeId" :course-id="course.id" @updated="onUpdated">
        <template #intro="{ messages }">
          <div v-if="!messages.length" class="card p-5 text-sm text-slate-600">
            <p class="font-medium text-slate-800">基于“{{ course.name }}”的资料提问</p>
            <p class="mt-1">AI 会先查找你导入的教材和课件，回答中用 <span class="source-tag source-tag-textbook">教材</span><span class="source-tag source-tag-supplement">补充</span><span class="source-tag source-tag-derived">推导</span>标明内容来源，点击 <span class="rounded bg-indigo-50 px-1 text-[11px] font-semibold text-indigo-700">S1</span> 这样的编号可以跳到原文。</p>
            <div class="mt-3 flex flex-wrap gap-2">
              <button v-for="sample in samples" :key="sample" class="btn btn-soft btn-sm" @click="panel?.setDraft(sample)">{{ sample }}</button>
            </div>
          </div>
        </template>
      </ChatPanel>
      <EmptyState v-else-if="!loading" title="选择或新建一个对话" description="对话记录会保存在本机，下次打开可以继续。">
        <button class="btn btn-primary" @click="createConversation()">新建对话</button>
      </EmptyState>
    </section>

    <ConfirmDialog
      :open="!!removing"
      title="删除对话"
      :message="`确定删除“${removing?.title || '新对话'}”及其全部消息吗？`"
      confirm-text="删除"
      danger
      @cancel="removing = null"
      @confirm="remove"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { PlusIcon, TrashIcon } from '@heroicons/vue/20/solid'
import ChatPanel from '@/components/chat/ChatPanel.vue'
import ConfirmDialog from '@/components/ui/ConfirmDialog.vue'
import EmptyState from '@/components/ui/EmptyState.vue'
import { ChatService } from '@/services/ai'
import { useToastStore } from '@/stores/toast'
import type { Conversation } from '@/types/ai'
import type { Course } from '@/types/models'
import { formatRelative } from '@/utils/format'

const props = defineProps<{ course: Course }>()
const route = useRoute()
const router = useRouter()
const toast = useToastStore()
const conversations = ref<Conversation[]>([])
const loading = ref(true)
const creating = ref(false)
const removing = ref<Conversation | null>(null)
const panel = ref<InstanceType<typeof ChatPanel>>()
const activeId = computed(() => Number(route.query.c) || null)
const samples = ['这门课有哪些核心概念？', '用通俗的话解释一下第一章的重点', '帮我梳理容易混淆的知识点']

async function refresh() {
  conversations.value = await ChatService.list(props.course.id)
}

onMounted(async () => {
  try {
    await refresh()
    if (!activeId.value && conversations.value.length) select(conversations.value[0].id)
  } catch (e) {
    toast.error(e)
  } finally {
    loading.value = false
  }
})

function select(id: number) {
  router.replace({ query: { ...route.query, c: id } })
}

async function createConversation() {
  // Reuse an untouched conversation instead of piling up empty ones.
  const empty = conversations.value.find(c => c.message_count === 0 && c.mode === 'qa')
  if (empty) return select(empty.id)
  creating.value = true
  try {
    const conversation = await ChatService.create({ course_id: props.course.id, mode: 'qa' })
    conversations.value.unshift(conversation)
    select(conversation.id)
  } catch (e) {
    toast.error(e)
  } finally {
    creating.value = false
  }
}

function onUpdated(conversation: Conversation) {
  const index = conversations.value.findIndex(c => c.id === conversation.id)
  if (index >= 0) conversations.value[index] = conversation
  conversations.value.sort((a, b) => b.updated_at.localeCompare(a.updated_at) || b.id - a.id)
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
</script>
