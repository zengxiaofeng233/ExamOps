<template>
  <form class="relative rounded-xl border border-slate-300 bg-surface shadow-sm focus-within:border-indigo-500 focus-within:ring-2 focus-within:ring-indigo-500/20" @submit.prevent="submit" @dragover.prevent="dragOver" @dragleave="dragging = false" @drop.prevent="drop" @paste="paste">
    <div v-if="dragging" class="pointer-events-none absolute inset-0 z-10 flex items-center justify-center rounded-xl border-2 border-dashed border-indigo-500 bg-indigo-50/95 text-sm text-indigo-700">松开以添加图片</div>
    <div v-if="images.length" class="flex flex-wrap gap-2 px-3 pt-3">
      <div v-for="(image, index) in images" :key="index" class="relative">
        <img :src="image.data_url" :alt="image.name" class="h-20 w-24 rounded-lg border border-slate-200 object-contain" />
        <button type="button" class="absolute -right-1 -top-1 flex size-5 items-center justify-center rounded-full bg-slate-700 text-white" :aria-label="`移除图片 ${image.name}`" :disabled="busy || reading" @click="images.splice(index, 1)">×</button>
      </div>
    </div>
    <textarea
      ref="area"
      v-model="text"
      :rows="1"
      :disabled="disabled"
      :placeholder="placeholder"
      maxlength="8000"
      class="block max-h-48 w-full resize-none rounded-t-xl border-0 bg-transparent px-4 pt-3 pb-1 text-[15px] leading-6 outline-none placeholder:text-slate-400 disabled:cursor-not-allowed"
      aria-label="输入问题"
      @keydown.enter="onEnter"
      @input="resize"
    />
    <div class="flex items-center justify-between gap-2 px-3 pb-2">
      <div class="flex min-w-0 flex-wrap items-center gap-1.5">
        <input ref="fileInput" type="file" accept="image/jpeg,image/png,image/webp" multiple class="hidden" aria-label="选择图片" @change="choose" />
        <button type="button" class="btn btn-ghost btn-sm" :disabled="disabled || busy || reading || images.length >= 4" @click="fileInput?.click()"><PhotoIcon class="size-4" />{{ reading ? '读取中…' : '添加图片' }}</button>
        <slot name="tools" />
      </div>
      <div class="flex shrink-0 items-center gap-2">
        <span class="hidden whitespace-nowrap text-[11px] text-slate-400 xl:inline">Enter 发送 · Shift+Enter 换行</span>
        <button v-if="busy" type="button" class="btn btn-secondary btn-sm" @click="emit('stop')">
          <StopIcon class="size-3.5" />停止
        </button>
        <button v-else type="submit" class="btn btn-primary btn-sm" :disabled="disabled || reading || (!text.trim() && !images.length)">
          <PaperAirplaneIcon class="size-3.5" />发送
        </button>
      </div>
    </div>
    <p class="px-4 pb-2 text-[11px] text-slate-400">可拖入或粘贴图片 · JPG / PNG / WebP · 最多 4 张，每张 5 MB · 需使用支持图片的模型</p>
    <p v-if="error" role="alert" class="px-4 pb-3 text-xs text-rose-600">{{ error }}</p>
  </form>
</template>

<script setup lang="ts">
import { nextTick, ref, watch } from 'vue'
import { PaperAirplaneIcon, PhotoIcon, StopIcon } from '@heroicons/vue/20/solid'
import type { ImageAttachment } from '@/types/ai'
import type { SendOptions } from '@/services/ai'

const props = defineProps<{ busy?: boolean; disabled?: boolean; placeholder?: string }>()
const emit = defineEmits<{ send: [text: string, options: SendOptions]; stop: [] }>()
const text = ref('')
const area = ref<HTMLTextAreaElement>()
const fileInput = ref<HTMLInputElement>()
const images = ref<ImageAttachment[]>([])
const reading = ref(false)
const dragging = ref(false)
const error = ref('')
let submitted = false
let submittedText = ''
watch(() => props.busy, busy => { if (!busy) submitted = false })

async function addImages(files: File[]) {
  if (props.disabled || props.busy || reading.value) return
  error.value = ''
  if (images.value.length + files.length > 4) {
    error.value = '每条消息最多上传 4 张图片，请减少图片数量。'
    return
  }
  reading.value = true
  try {
    const added = await Promise.all(files.map(async file => {
      if (!['image/jpeg', 'image/png', 'image/webp'].includes(file.type)) throw new Error('仅支持 JPG、PNG、WebP 图片。')
      if (!file.size || file.size > 5 * 1024 * 1024) throw new Error('图片不能为空，且每张不能超过 5 MB。')
      const data_url = await new Promise<string>((resolve, reject) => {
        const reader = new FileReader()
        reader.onload = () => resolve(String(reader.result))
        reader.onerror = () => reject(new Error('读取图片失败，请重新选择。'))
        reader.readAsDataURL(file)
      })
      const preview = new Image()
      preview.src = data_url
      await preview.decode().catch(() => { throw new Error('图片无法打开，请选择有效的图片文件。') })
      return { name: file.name.slice(0, 255), data_url }
    }))
    images.value.push(...added)
  } catch (e) {
    error.value = e instanceof Error ? e.message : '读取图片失败'
  } finally {
    reading.value = false
  }
}

function choose(event: Event) {
  const input = event.target as HTMLInputElement
  addImages(Array.from(input.files ?? []))
  input.value = ''
}

function dragOver(event: DragEvent) {
  if (!event.dataTransfer?.types.includes('Files')) return
  dragging.value = !props.disabled && !props.busy && !reading.value
  event.dataTransfer.dropEffect = dragging.value ? 'copy' : 'none'
}

function drop(event: DragEvent) {
  dragging.value = false
  addImages(Array.from(event.dataTransfer?.files ?? []))
}

function paste(event: ClipboardEvent) {
  const files = Array.from(event.clipboardData?.files ?? []).filter(file => file.type.startsWith('image/'))
  if (!files.length) return
  event.preventDefault()
  addImages(files)
}

function resize() {
  const el = area.value
  if (!el) return
  el.style.height = 'auto'
  el.style.height = `${Math.min(el.scrollHeight, 192)}px`
}

function onEnter(event: KeyboardEvent) {
  // Chinese / Japanese IMEs use Enter to confirm a candidate: never send then.
  if (event.isComposing || event.keyCode === 229 || event.shiftKey) return
  event.preventDefault()
  submit()
}

function submit() {
  const value = text.value.trim()
  if ((!value && !images.value.length) || props.busy || props.disabled || reading.value) return
  submitted = true
  submittedText = value
  emit('send', value, { images: [...images.value] })
}

function clearSubmitted() {
  if (!submitted) return
  submitted = false
  if (text.value.trim() === submittedText) text.value = ''
  images.value = []
  error.value = ''
  nextTick(resize)
}

defineExpose({ clearSubmitted, focus: () => area.value?.focus(), setText: (value: string) => { text.value = value; nextTick(resize) } })
</script>
