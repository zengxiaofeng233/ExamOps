<template>
  <Teleport to="body">
    <div
      v-if="open"
      class="fixed inset-0 z-50 flex items-center justify-center bg-slate-900/40 p-4"
      @mousedown.self="!persistent && emit('close')"
      @keydown.esc="!persistent && emit('close')"
    >
      <div
        ref="panel"
        role="dialog"
        aria-modal="true"
        :aria-label="title"
        tabindex="-1"
        class="card flex max-h-[90vh] w-full flex-col outline-none"
        :class="widthClass"
      >
        <header class="flex items-center justify-between gap-4 border-b border-slate-100 px-5 py-3.5">
          <h2 class="section-title">{{ title }}</h2>
          <button v-if="!persistent" class="btn btn-ghost btn-sm" aria-label="关闭" @click="emit('close')">
            <XMarkIcon class="size-5" />
          </button>
        </header>
        <div class="min-h-0 flex-1 overflow-y-auto px-5 py-4">
          <slot />
        </div>
        <footer v-if="$slots.footer" class="flex justify-end gap-2 border-t border-slate-100 px-5 py-3">
          <slot name="footer" />
        </footer>
      </div>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import { XMarkIcon } from '@heroicons/vue/24/outline'

const props = withDefaults(defineProps<{
  open: boolean
  title: string
  size?: 'sm' | 'md' | 'lg' | 'xl'
  persistent?: boolean
}>(), { size: 'md', persistent: false })

const emit = defineEmits<{ close: [] }>()
const panel = ref<HTMLElement>()

const widthClass = computed(() => ({
  sm: 'max-w-sm',
  md: 'max-w-lg',
  lg: 'max-w-2xl',
  xl: 'max-w-4xl'
}[props.size]))

watch(() => props.open, async (open) => {
  if (!open) return
  await nextTick()
  // Focus the first field so keyboard users can type immediately.
  const field = panel.value?.querySelector<HTMLElement>('input, textarea, select')
  ;(field ?? panel.value)?.focus()
})
</script>
