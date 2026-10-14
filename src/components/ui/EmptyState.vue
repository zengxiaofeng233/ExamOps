<template>
  <div class="flex flex-col items-center justify-center px-6 py-12 text-center">
    <div class="mb-3 flex size-12 items-center justify-center rounded-full bg-slate-100 text-slate-400">
      <slot name="icon">
        <InboxIcon class="size-6" />
      </slot>
    </div>
    <h3 class="text-sm font-semibold text-slate-800">{{ title }}</h3>
    <p v-if="description" class="mt-1 max-w-md text-sm text-slate-500">{{ description }}</p>
    <div v-if="$slots.default" class="mt-4 flex flex-wrap justify-center gap-2">
      <slot />
    </div>
  </div>
</template>

<script setup lang="ts">
import { InboxIcon } from '@heroicons/vue/24/outline'

defineProps<{ title: string; description?: string }>()
</script>
