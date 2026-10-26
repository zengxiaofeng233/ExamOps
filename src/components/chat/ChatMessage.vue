<template>
  <div v-if="message.role === 'user' && isAction" class="flex justify-end">
    <span class="inline-flex items-center gap-1.5 rounded-full border border-indigo-200 bg-indigo-50 px-3 py-1 text-sm text-indigo-700">
      <BoltIcon class="size-3.5" aria-hidden="true" />{{ message.content }}
    </span>
  </div>
  <div v-else-if="message.role === 'user'" class="flex justify-end">
    <div class="max-w-[85%] rounded-2xl rounded-br-md bg-primary px-4 py-2.5 text-[15px] leading-7 break-words text-white">
      <div v-if="message.meta.images?.length" class="mb-2 flex flex-wrap gap-2">
        <button v-for="(image, index) in message.meta.images" :key="index" type="button" :aria-label="`查看图片 ${image.name}`" @click="previewImage = image; preview?.showModal()">
          <img :src="image.data_url" :alt="image.name" class="max-h-48 max-w-full rounded-lg bg-white object-contain" />
        </button>
      </div>
      <p class="whitespace-pre-wrap">{{ message.content }}</p>
    </div>
  </div>
  <div v-else class="flex gap-3">
    <div class="mt-0.5 flex size-7 shrink-0 items-center justify-center rounded-full bg-gradient-to-br from-indigo-500 to-violet-500 text-[11px] font-bold text-white">AI</div>
    <div class="min-w-0 flex-1">
      <p class="mb-1 flex flex-wrap items-center gap-x-2 gap-y-1 text-xs text-slate-500">
        <span v-if="partLabel" class="badge bg-indigo-50 text-indigo-700">{{ partLabel }}</span>
        <span v-if="message.meta.model">{{ message.meta.profile ?? '' }} · {{ message.meta.model }}</span>
        <span v-if="contextLabel" class="badge bg-slate-100 text-slate-600" :title="contextTitle">{{ contextLabel }}</span>
        <span v-if="message.meta.notice" class="text-amber-600">{{ message.meta.notice }}</span>
      </p>

      <div v-if="!message.content && message.status === 'streaming'" class="flex items-center gap-1.5 py-2 text-sm text-slate-400" role="status">
        <span class="size-1.5 animate-bounce rounded-full bg-slate-400 [animation-delay:-0.3s]" />
        <span class="size-1.5 animate-bounce rounded-full bg-slate-400 [animation-delay:-0.15s]" />
        <span class="size-1.5 animate-bounce rounded-full bg-slate-400" />
        <span class="ml-1">{{ reasoning ? '模型正在思考…' : '正在生成…' }}</span>
      </div>
      <MarkdownContent v-if="message.content" :text="message.content" :sources="message.sources" @cite="(s) => emit('open-source', s)" />

      <details v-if="reasoning" class="mt-2 text-xs text-slate-500">
        <summary class="cursor-pointer select-none">模型思考过程</summary>
        <p class="mt-1 rounded-md bg-slate-50 p-2 whitespace-pre-wrap">{{ reasoning }}</p>
      </details>

      <p v-if="message.status === 'cancelled'" class="mt-2 text-xs text-slate-500">已停止生成，以上为已生成的部分。</p>
      <p v-if="message.meta.output_truncated" class="mt-2 text-xs text-amber-600">回答达到最大输出长度被截断，可在模型设置中调大“最大输出”。</p>
      <div v-if="message.status === 'error'" class="mt-2 rounded-lg border border-rose-200 bg-rose-50 px-3 py-2 text-sm text-rose-800" role="alert">
        <p class="break-words">{{ message.error }}</p>
        <div class="mt-2 flex gap-2">
          <button v-if="isLast" class="btn btn-secondary btn-sm" :disabled="busy" @click="emit('regenerate')">重试</button>
          <RouterLink v-if="needsSettings" to="/settings?section=models" class="btn btn-secondary btn-sm">检查模型设置</RouterLink>
        </div>
      </div>

      <SourceList :sources="message.sources" @open="(s) => emit('open-source', s)" />

      <div v-if="message.status !== 'streaming' && message.content" class="mt-1.5 flex gap-1 text-xs">
        <button class="btn btn-ghost btn-sm text-slate-500" @click="copy">{{ copied ? '已复制' : '复制' }}</button>
        <button v-if="isLast && message.status !== 'error'" class="btn btn-ghost btn-sm text-slate-500" :disabled="busy" @click="emit('regenerate')">重新生成</button>
      </div>
    </div>
  </div>
  <dialog ref="preview" class="m-auto max-h-[90vh] max-w-[90vw] rounded-xl bg-surface p-3 backdrop:bg-black/60" aria-label="图片预览" @click="event => { if (event.target === preview) preview?.close() }">
    <div class="mb-2 flex justify-end"><button type="button" class="btn btn-secondary btn-sm" @click="preview?.close()">关闭图片</button></div>
    <img v-if="previewImage" :src="previewImage.data_url" :alt="previewImage.name" class="max-h-[75vh] max-w-full object-contain" />
  </dialog>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { BoltIcon } from '@heroicons/vue/20/solid'
import MarkdownContent from './MarkdownContent.vue'
import SourceList from './SourceList.vue'
import type { ImageAttachment, Message, SourceRef } from '@/types/ai'

const props = defineProps<{ message: Message; isLast?: boolean; busy?: boolean; liveReasoning?: string }>()
const emit = defineEmits<{ 'open-source': [source: SourceRef]; regenerate: [] }>()
const copied = ref(false)
const preview = ref<HTMLDialogElement>()
const previewImage = ref<ImageAttachment>()

const reasoning = computed(() => props.liveReasoning || (props.message.meta.reasoning as string | undefined) || '')
const contextLabel = computed(() => ({ full: '全文上下文', rag: '检索上下文', lesson: '本节资料', none: '无资料' } as Record<string, string>)[props.message.meta.context_mode ?? ''] ?? '')
/** Teaching buttons are stored as short action messages. */
const isAction = computed(() => !!props.message.meta.plan?.action && props.message.meta.plan.action !== 'ask')
const partLabel = computed(() => {
  const plan = props.message.meta.plan
  return plan && plan.part != null && plan.parts ? `第 ${plan.part + 1}/${plan.parts} 部分` : ''
})
const contextTitle = computed(() => {
  const meta = props.message.meta
  const parts = []
  if (meta.context_tokens != null) parts.push(`本次发送资料约 ${meta.context_tokens} tokens`)
  if (meta.material_tokens) parts.push(`范围内资料共约 ${meta.material_tokens} tokens`)
  if (meta.usage) parts.push(`用量：输入 ${meta.usage.prompt} / 输出 ${meta.usage.completion}${meta.usage.estimated ? '（估算）' : ''}`)
  return parts.join('\n')
})
const needsSettings = computed(() => ['auth', 'not_found', 'invalid_url', 'no_profile'].includes(props.message.meta.error_kind ?? ''))

async function copy() {
  try {
    await navigator.clipboard.writeText(props.message.content)
    copied.value = true
    setTimeout(() => (copied.value = false), 1500)
  } catch {
    copied.value = false
  }
}
</script>
