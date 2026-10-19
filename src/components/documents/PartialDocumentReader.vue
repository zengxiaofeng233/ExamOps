<template>
  <section class="flex min-h-0 flex-1 flex-col bg-slate-50">
    <p class="border-b border-slate-200 bg-surface px-5 py-3 text-sm text-slate-600" role="status">
      {{ document.status === 'failed' ? '解析中断，已完成内容已保留。点击“重新解析”可继续。' : document.status === 'pending' ? '已加入处理队列，重新打开软件也会自动继续。' : '正在后台识别，可先阅读已完成页面。' }}
      <span v-if="document.completed_units">已完成 {{ document.completed_units }}/{{ document.total_units || '—' }} 个单元。</span>
      搜索、AI 和章节编辑将在整份解析完成后开放。
    </p>
    <p v-if="document.error && document.status === 'failed'" class="px-5 py-2 text-sm text-rose-700" role="alert">{{ document.error }}</p>
    <div class="min-h-0 flex-1 overflow-y-auto p-4">
      <p v-if="error" class="text-sm text-rose-700" role="alert">{{ error }}</p>
      <p v-if="!pages.length" class="p-4 text-sm text-slate-500">等待页面识别完成…</p>
      <div class="mx-auto max-w-3xl space-y-3">
        <PageCard v-for="page in pages" :key="page.page_number" :page="page" :document-id="document.id"
          :file-type="document.file_type" :terms="[]" :targeted="false" :preview-all="false" :compact="false"
          @open-image="image => viewing = { image, label: page.label }" />
        <button v-if="end < (document.total_units || 0)" class="btn btn-secondary w-full" :disabled="loading" @click="more">继续加载页面</button>
      </div>
    </div>
    <ImageViewer :image="viewing?.image ?? null" :label="viewing?.label" @close="viewing = null" />
  </section>
</template>

<script setup lang="ts">
import { onBeforeUnmount, ref, watch } from 'vue'
import { DocumentService } from '@/services/document'
import type { Document, ImageInfo, PageView } from '@/types/document'
import { errorMessage } from '@/utils/errors'
import PageCard from './PageCard.vue'
import ImageViewer from './ImageViewer.vue'

const props = defineProps<{ document: Document }>()
const end = ref(80)
const pages = ref<PageView[]>([])
const error = ref('')
const loading = ref(false)
const viewing = ref<{ image: ImageInfo; label: string } | null>(null)
let revision = 0
onBeforeUnmount(() => { revision++ })
async function refresh() {
  const request = ++revision
  loading.value = true
  try {
    // Bounded requests even for a very long document.
    const loaded: PageView[] = []
    for (let start = 1; start <= end.value; start += 80) {
      loaded.push(...await DocumentService.pages(props.document.id, start, Math.min(start + 79, end.value)))
    }
    if (request === revision) { pages.value = loaded; error.value = '' }
  } catch (e) {
    if (request === revision) error.value = errorMessage(e)
  } finally {
    if (request === revision) loading.value = false
  }
}
function more() { end.value += 80; refresh() }
watch(() => [props.document.id, props.document.completed_units, props.document.status], refresh, { immediate: true })
</script>
