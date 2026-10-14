<template>
  <details v-if="sorted.length" class="group mt-3 rounded-lg border border-slate-200 bg-slate-50/60" :open="open">
    <summary class="flex cursor-pointer list-none items-center gap-2 px-3 py-2 text-xs text-slate-600 select-none">
      <ChevronRightIcon class="size-3.5 transition-transform group-open:rotate-90" />
      <span class="font-medium">资料来源</span>
      <span>{{ citedCount }} 处被引用{{ sorted.length > citedCount ? ` · 共参考 ${sorted.length} 段` : '' }}</span>
    </summary>
    <ul class="space-y-1 px-2 pb-2">
      <li v-for="source in sorted" :key="source.label">
        <button
          class="flex w-full items-start gap-2 rounded-md px-2 py-1.5 text-left text-xs hover:bg-white"
          :title="source.excerpt"
          @click="emit('open', source)"
        >
          <span class="mt-0.5 inline-flex min-w-[1.6rem] justify-center rounded px-1 text-[11px] font-semibold"
            :class="source.cited ? 'bg-indigo-100 text-indigo-700' : 'bg-slate-200 text-slate-500'">{{ source.label }}</span>
          <span class="min-w-0 flex-1">
            <span class="block truncate font-medium text-slate-700">{{ source.filename }}<template v-if="source.section_title"> · {{ source.section_title }}</template></span>
            <span class="block text-slate-500">{{ source.loc_label }} · {{ source.excerpt }}</span>
          </span>
        </button>
      </li>
    </ul>
  </details>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { ChevronRightIcon } from '@heroicons/vue/20/solid'
import type { SourceRef } from '@/types/ai'

const props = defineProps<{ sources: SourceRef[]; open?: boolean }>()
const emit = defineEmits<{ open: [source: SourceRef] }>()

const sorted = computed(() => [...props.sources].sort((a, b) => Number(b.cited) - Number(a.cited) || Number(a.label.slice(1)) - Number(b.label.slice(1))))
const citedCount = computed(() => props.sources.filter(s => s.cited).length)
</script>
