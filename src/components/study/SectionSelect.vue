<template>
  <select
    :value="modelValue ?? ''"
    class="input"
    :aria-label="label"
    @change="emit('update:modelValue', ($event.target as HTMLSelectElement).value ? Number(($event.target as HTMLSelectElement).value) : null)"
  >
    <option value="">{{ placeholder }}</option>
    <optgroup v-for="group in groups" :key="group.documentId" :label="group.filename">
      <option v-for="section in group.sections" :key="section.id" :value="section.id">
        {{ '　'.repeat(section.level - 1) }}{{ section.title }}（{{ section.loc_label }}）
      </option>
    </optgroup>
  </select>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { CourseSection } from '@/types/document'

const props = withDefaults(defineProps<{
  modelValue: number | null
  sections: CourseSection[]
  placeholder?: string
  label?: string
  /** Deepest heading level offered (chapters = 1). */
  maxLevel?: number
}>(), { placeholder: '不关联章节', label: '章节', maxLevel: 3 })
const emit = defineEmits<{ 'update:modelValue': [value: number | null] }>()

const groups = computed(() => {
  const map = new Map<number, CourseSection[]>()
  for (const section of props.sections) {
    if (section.level > props.maxLevel) continue
    const list = map.get(section.document_id) ?? []
    list.push(section)
    map.set(section.document_id, list)
  }
  return [...map.entries()].map(([documentId, sections]) => ({ documentId, filename: sections[0].filename, sections }))
})
</script>
