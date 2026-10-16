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
            <span v-if="item.section.source === 'topic'" class="block text-[11px] text-sky-700">主题学习单元 · 自动分段</span>
            <span v-if="item.section.locked" class="text-[11px] text-amber-700">已锁定</span>
            <span v-else-if="item.section.user_edited" class="text-[11px] text-slate-500">已手动调整</span>
          </button>
        </div>
      </li>
    </ul>
    <form v-if="editable && selected" class="mt-3 space-y-2 border-t border-slate-200 p-2" @submit.prevent="emit('edit', selected, { action: 'rename', title: name })">
      <label for="section-name" class="block text-xs text-slate-600">章节名称 / 拆分后的新名称</label>
      <input id="section-name" v-model="name" class="input w-full text-xs" maxlength="200" :disabled="busy || selected.locked" />
      <div class="flex flex-wrap gap-1">
        <button type="submit" class="btn btn-secondary btn-sm" :disabled="busy || selected.locked || !name.trim()">重命名</button>
        <button type="button" class="btn btn-secondary btn-sm" :disabled="busy" @click="emit('edit', selected, { action: 'lock', locked: !selected.locked })">{{ selected.locked ? '解锁' : '锁定' }}</button>
        <button type="button" class="btn btn-secondary btn-sm" :disabled="busy || selected.locked || !following || following.locked" @click="following && emit('edit', selected, { action: 'merge', next_id: following.id })">合并下一节</button>
        <button type="button" class="btn btn-secondary btn-sm" :disabled="busy || selected.locked || !name.trim()" @click="emit('split', selected, name === selected.title ? `${name}（续）` : name)">选择拆分位置</button>
      </div>
      <p class="text-[11px] text-slate-500">合并保留当前名称。拆分后原引用保留在前半节；子章节引用保持不变。锁定后重新解析将保留当前目录；原文位置变化时提示解锁。</p>
    </form>
  </nav>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { ChevronRightIcon } from '@heroicons/vue/20/solid'
import type { Section, SectionEdit } from '@/types/document'

const props = defineProps<{ sections: Section[]; activeId?: number | null; editable?: boolean; busy?: boolean }>()
const emit = defineEmits<{ select: [section: Section]; edit: [section: Section, edit: SectionEdit]; split: [section: Section, title: string] }>()
const selected = computed(() => props.sections.find(s => s.id === props.activeId))
const following = computed(() => selected.value && props.sections.find(s => s.ordinal > selected.value!.ordinal && s.parent_id === selected.value!.parent_id && s.level === selected.value!.level))
const name = ref('')
watch(selected, section => { name.value = section?.title ?? '' }, { immediate: true })

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
