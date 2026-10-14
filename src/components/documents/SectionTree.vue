<template>
  <nav class="text-sm" aria-label="章节目录">
    <ul class="space-y-0.5">
      <li v-for="item in visible" :key="item.section.id">
        <div
          class="group flex items-center gap-1 rounded-md pr-2"
          :class="item.section.id === activeId ? 'bg-indigo-50 text-indigo-800' : 'text-slate-700 hover:bg-slate-100'"
          :style="{ paddingLeft: `${(item.section.level - 1) * 14 + 4}px` }"
        >
          <button
            v-if="item.hasChildren"
            class="flex size-5 shrink-0 items-center justify-center rounded text-slate-400 hover:text-slate-700"
            :aria-label="collapsed.has(item.section.id) ? '展开' : '收起'"
            :aria-expanded="!collapsed.has(item.section.id)"
            @click="toggle(item.section.id)"
          >
            <ChevronRightIcon class="size-3.5 transition-transform" :class="{ 'rotate-90': !collapsed.has(item.section.id) }" />
          </button>
          <span v-else class="size-5 shrink-0" />
          <button
            class="min-w-0 flex-1 py-1.5 text-left"
            :class="item.section.level === 1 ? 'font-medium' : ''"
            :title="`${item.section.title}（${item.section.loc_label}）`"
            @click="emit('select', item.section)"
          >
            <span class="line-clamp-2 break-words">{{ item.section.title }}</span>
            <span class="block text-[11px] text-slate-400">{{ item.section.loc_label }}</span>
          </button>
        </div>
      </li>
    </ul>
  </nav>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { ChevronRightIcon } from '@heroicons/vue/20/solid'
import type { Section } from '@/types/document'

const props = defineProps<{ sections: Section[]; activeId?: number | null }>()
const emit = defineEmits<{ select: [section: Section] }>()

const collapsed = ref(new Set<number>())
const children = computed(() => {
  const map = new Map<number, number>()
  for (const s of props.sections) if (s.parent_id) map.set(s.parent_id, (map.get(s.parent_id) ?? 0) + 1)
  return map
})
const byId = computed(() => new Map(props.sections.map(s => [s.id, s])))

watch(() => props.sections, (sections) => {
  // Big trees start with only chapters expanded.
  const next = new Set<number>()
  if (sections.length > 60) for (const s of sections) if (s.level >= 1 && children.value.get(s.id)) next.add(s.id)
  collapsed.value = next
}, { immediate: true })

watch(() => props.activeId, (id) => {
  // Keep the active section visible.
  let current = id ? byId.value.get(id) : undefined
  while (current?.parent_id) {
    collapsed.value.delete(current.parent_id)
    current = byId.value.get(current.parent_id)
  }
}, { immediate: true })

const visible = computed(() => props.sections
  .filter((section) => {
    let parent = section.parent_id ? byId.value.get(section.parent_id) : undefined
    while (parent) {
      if (collapsed.value.has(parent.id)) return false
      parent = parent.parent_id ? byId.value.get(parent.parent_id) : undefined
    }
    return true
  })
  .map(section => ({ section, hasChildren: (children.value.get(section.id) ?? 0) > 0 })))

function toggle(id: number) {
  const next = new Set(collapsed.value)
  if (next.has(id)) next.delete(id)
  else next.add(id)
  collapsed.value = next
}
</script>
