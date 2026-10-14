<template>
  <!-- eslint-disable-next-line vue/no-v-html -- markdown-it runs with html disabled; see utils/markdown.ts -->
  <div ref="root" class="prose-chat" @click="onClick" v-html="html" />
</template>

<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import { renderMarkdown } from '@/utils/markdown'
import { AppService } from '@/services/app'
import { useToastStore } from '@/stores/toast'
import type { SourceRef } from '@/types/ai'

const props = defineProps<{ text: string; sources?: SourceRef[] }>()
const emit = defineEmits<{ cite: [source: SourceRef] }>()
const toast = useToastStore()
const root = ref<HTMLElement>()

const html = computed(() => renderMarkdown(props.text))

// Describe each citation; mark labels the model invented (not in the material).
watch([html, () => props.sources], async () => {
  await nextTick()
  root.value?.querySelectorAll<HTMLElement>('button.cite').forEach((button) => {
    const source = props.sources?.find(s => s.label === button.dataset.cite)
    if (source) {
      button.title = `${source.filename}${source.section_title ? ` · ${source.section_title}` : ''} · ${source.loc_label}\n${source.excerpt}`
      button.classList.remove('cite-missing')
    } else {
      button.title = '资料中没有这个来源编号（AI 引用有误）'
      button.classList.add('cite-missing')
    }
  })
}, { immediate: true })

function onClick(event: MouseEvent) {
  const target = event.target as HTMLElement
  const cite = target.closest<HTMLElement>('button.cite')
  if (cite) {
    const source = props.sources?.find(s => s.label === cite.dataset.cite)
    if (source) emit('cite', source)
    return
  }
  const link = target.closest<HTMLAnchorElement>('a[data-external]')
  if (link) {
    event.preventDefault()
    AppService.openExternal(link.href).catch((e) => toast.error(e))
  }
}
</script>
