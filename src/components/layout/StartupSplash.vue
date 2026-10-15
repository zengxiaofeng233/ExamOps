<template>
  <div
    class="startup-splash fixed inset-0 z-[100] grid place-items-center bg-slate-50"
    aria-hidden="true"
    @animationend.self="emit('finished')"
    @animationcancel.self="emit('finished')"
  >
    <div class="startup-brand flex flex-col items-center text-center">
      <div class="mb-6 flex size-20 items-center justify-center rounded-3xl bg-primary text-4xl font-bold text-white shadow-lg ring-8 ring-indigo-500/10">E</div>
      <p class="text-3xl font-semibold tracking-tight text-slate-900">ExamOps</p>
      <p class="mt-2 text-sm tracking-widest text-slate-500">AI 智能复习</p>
    </div>
  </div>
</template>

<script setup lang="ts">
import { onBeforeUnmount, onMounted } from 'vue'

const emit = defineEmits<{ finished: [] }>()
let fallback: ReturnType<typeof setTimeout> | undefined

onMounted(() => {
  // Never leave the UI covered if animation events are suppressed.
  fallback = setTimeout(() => emit('finished'), 1500)
})
onBeforeUnmount(() => clearTimeout(fallback))
</script>

<style scoped>
.startup-splash {
  animation: startup-exit 240ms ease-in 880ms forwards;
}

.startup-brand {
  animation: startup-enter 440ms cubic-bezier(0.16, 1, 0.3, 1) both;
}

@keyframes startup-enter {
  from { opacity: 0; transform: translateY(12px) scale(0.96); }
  to { opacity: 1; transform: translateY(0) scale(1); }
}

@keyframes startup-exit {
  to { opacity: 0; }
}

@media (prefers-reduced-motion: reduce) {
  .startup-splash { display: none; }
  .startup-splash, .startup-brand { animation: none; }
}
</style>
