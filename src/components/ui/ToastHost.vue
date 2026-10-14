<template>
  <div class="pointer-events-none fixed right-4 bottom-4 z-[60] flex w-80 flex-col gap-2" aria-live="polite">
    <div
      v-for="toast in store.toasts"
      :key="toast.id"
      role="status"
      class="pointer-events-auto flex items-start gap-2 rounded-lg border bg-white px-3 py-2.5 text-sm shadow-lg"
      :class="{
        'border-emerald-200': toast.kind === 'success',
        'border-rose-200': toast.kind === 'error',
        'border-slate-200': toast.kind === 'info'
      }"
    >
      <CheckCircleIcon v-if="toast.kind === 'success'" class="mt-0.5 size-4 shrink-0 text-emerald-600" />
      <ExclamationCircleIcon v-else-if="toast.kind === 'error'" class="mt-0.5 size-4 shrink-0 text-rose-600" />
      <InformationCircleIcon v-else class="mt-0.5 size-4 shrink-0 text-indigo-600" />
      <p class="min-w-0 flex-1 break-words whitespace-pre-wrap text-slate-700">{{ toast.text }}</p>
      <button class="text-slate-400 hover:text-slate-600" aria-label="关闭" @click="store.dismiss(toast.id)">
        <XMarkIcon class="size-4" />
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { CheckCircleIcon, ExclamationCircleIcon, InformationCircleIcon, XMarkIcon } from '@heroicons/vue/20/solid'
import { useToastStore } from '@/stores/toast'

const store = useToastStore()
</script>
