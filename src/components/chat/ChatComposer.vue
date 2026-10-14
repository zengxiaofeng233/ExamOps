<template>
  <form class="rounded-xl border border-slate-300 bg-white shadow-sm focus-within:border-indigo-500 focus-within:ring-2 focus-within:ring-indigo-500/20" @submit.prevent="submit">
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
        <slot name="tools" />
      </div>
      <div class="flex items-center gap-2">
        <span class="hidden text-[11px] text-slate-400 sm:inline">Enter 发送 · Shift+Enter 换行</span>
        <button v-if="busy" type="button" class="btn btn-secondary btn-sm" @click="emit('stop')">
          <StopIcon class="size-3.5" />停止
        </button>
        <button v-else type="submit" class="btn btn-primary btn-sm" :disabled="disabled || !text.trim()">
          <PaperAirplaneIcon class="size-3.5" />发送
        </button>
      </div>
    </div>
  </form>
</template>

<script setup lang="ts">
import { nextTick, ref } from 'vue'
import { PaperAirplaneIcon, StopIcon } from '@heroicons/vue/20/solid'

const props = defineProps<{ busy?: boolean; disabled?: boolean; placeholder?: string }>()
const emit = defineEmits<{ send: [text: string]; stop: [] }>()
const text = ref('')
const area = ref<HTMLTextAreaElement>()

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
  if (!value || props.busy || props.disabled) return
  emit('send', value)
  text.value = ''
  nextTick(resize)
}

defineExpose({ focus: () => area.value?.focus(), setText: (value: string) => { text.value = value; nextTick(resize) } })
</script>
