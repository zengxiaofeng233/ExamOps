<template>
  <!-- Paragraph / line units read as one continuous document with margin labels. -->
  <div
    v-if="compact"
    :id="`unit-${page.page_number}`"
    class="group flex scroll-mt-4 gap-3 rounded-md px-2 py-1.5 transition-colors"
    :class="targeted ? 'bg-amber-50 ring-1 ring-amber-200' : ''"
  >
    <span class="w-16 shrink-0 pt-1 text-right text-[11px] text-slate-400 select-none" :title="page.label">{{ page.label.replace(/^第 /, '').replace(/ (段|行)$/, '') }}</span>
    <div class="min-w-0 flex-1 space-y-2 text-[15px] leading-7 text-slate-800">
      <div v-for="(block, index) in visibleBlocks" :key="index">
        <button v-if="canSplit" class="mb-1 text-xs text-indigo-600 underline" @click="emit('split', (blockOffset ?? 0) + page.blocks.indexOf(block))">从此块拆分</button>
        <component :is="block.heading ? `h${Math.min(block.heading + 2, 6)}` : 'p'" :class="blockClass(block)">
          <HighlightText :text="block.text" :terms="terms" />
        </component>
        <p v-if="unreliable(block)" class="text-xs text-amber-700">识别结果待核验，请对照原文。</p>
        <details v-if="block.formula || block.issues?.length" class="text-xs text-slate-600">
          <summary class="cursor-pointer">查看提取原文</summary>
          <pre class="overflow-x-auto whitespace-pre-wrap break-words">{{ block.raw_text ?? block.text }}</pre>
        </details>
      </div>
      <div v-if="page.images.length" class="grid grid-cols-2 gap-3 sm:grid-cols-3">
        <button
          v-for="image in page.images"
          :key="image.id"
          class="overflow-hidden rounded-md border border-slate-200 bg-slate-50 text-left hover:border-indigo-300"
          :title="image.ocr_text || '查看大图'"
          @click="emit('open-image', image)"
        >
          <LazyImage :load="() => DocumentService.image(image.id)" :ratio="image.width / image.height" :alt="`${page.label}图片 ${image.ordinal}`" />
        </button>
      </div>
    </div>
  </div>
  <article
    v-else
    :id="`unit-${page.page_number}`"
    class="scroll-mt-4 rounded-lg border bg-surface transition-shadow"
    :class="targeted ? 'border-amber-300 ring-2 ring-amber-200' : 'border-slate-200'"
  >
    <header class="flex items-center justify-between gap-2 border-b border-slate-100 px-4 py-2">
      <div class="flex items-center gap-2 text-xs text-slate-500">
        <span class="font-medium text-slate-700">{{ page.label }}</span>
        <span v-if="page.has_ocr" class="badge bg-sky-50 text-sky-700" title="此页包含图片或扫描内容，部分文字由本地 OCR 识别">含 OCR 文字</span>
      </div>
      <button v-if="previewable" class="btn btn-ghost btn-sm" @click="showPreview = !showPreview">
        {{ showPreview ? '隐藏原页面' : '查看原页面' }}
      </button>
    </header>
    <div v-if="showPreview && previewable" class="border-b border-slate-100 bg-slate-50 p-3">
      <LazyImage
        class="mx-auto max-w-3xl rounded border border-slate-200 bg-surface shadow-sm"
        :load="() => DocumentService.preview(documentId, page.page_number)"
        :ratio="page.width && page.height ? page.width / page.height : 0.707"
        :alt="`${page.label}原页面`"
      />
    </div>
    <div class="space-y-2 px-4 py-3 text-[15px] leading-7 text-slate-800">
      <p v-if="page.has_ocr" class="rounded bg-amber-50 px-3 py-2 text-xs text-amber-800">OCR 文字未经人工核验，公式、上下标和分式请对照原页面。</p>
      <div v-for="(block, index) in visibleBlocks" :key="index">
        <button v-if="canSplit" class="mb-1 text-xs text-indigo-600 underline" @click="emit('split', (blockOffset ?? 0) + page.blocks.indexOf(block))">从此块拆分</button>
        <component :is="block.heading ? `h${Math.min(block.heading + 2, 6)}` : 'p'" :class="blockClass(block)">
          <HighlightText :text="block.text" :terms="terms" />
        </component>
        <div v-if="unreliable(block)" class="mt-1 text-xs text-amber-700">
          公式或字符识别待核验
          <button v-if="previewable" class="ml-2 underline" @click="showPreview = true">对照原页面</button>
        </div>
        <details v-if="block.formula || block.issues?.length" class="text-xs text-slate-600">
          <summary class="cursor-pointer py-1">查看提取原文</summary>
          <pre class="rounded bg-slate-50 p-2 whitespace-pre-wrap break-words">{{ block.raw_text ?? block.text }}</pre>
          <p v-if="block.issues?.length">可疑字符：{{ block.issues.join('、') }}</p>
        </details>
      </div>
    </div>
    <div v-if="page.images.length" class="grid grid-cols-2 gap-3 border-t border-slate-100 px-4 py-3 sm:grid-cols-3">
      <button
        v-for="image in page.images"
        :key="image.id"
        class="group overflow-hidden rounded-md border border-slate-200 bg-slate-50 text-left hover:border-indigo-300"
        :title="image.ocr_text || '查看大图'"
        @click="emit('open-image', image)"
      >
        <LazyImage :load="() => DocumentService.image(image.id)" :ratio="image.width / image.height" :alt="`${page.label}图片 ${image.ordinal}`" />
        <span class="block px-2 py-1 text-[11px] text-slate-500">图 {{ image.ordinal }} · {{ image.width }}×{{ image.height }}</span>
      </button>
    </div>
  </article>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import HighlightText from '@/components/ui/HighlightText.vue'
import LazyImage from '@/components/ui/LazyImage.vue'
import { DocumentService } from '@/services/document'
import type { Block, ImageInfo, PageView } from '@/types/document'

const props = defineProps<{
  page: PageView
  documentId: number
  fileType: string
  terms: string[]
  targeted?: boolean
  previewAll?: boolean
  /** Paragraph / line units: render inline instead of as page cards. */
  canSplit?: boolean
  blockOffset?: number
  compact?: boolean
}>()
const emit = defineEmits<{ 'open-image': [image: ImageInfo]; split: [block: number] }>()

const previewable = computed(() => props.fileType === 'pdf')
// Bookmark titles that are not printed on the page only appear in the chapter tree.
const visibleBlocks = computed(() => props.page.blocks.filter(b => b.kind !== 'synthetic'))
const showPreview = ref(!!props.previewAll)

function unreliable(block: Block) {
  return !!block.issues?.length || (block.formula && block.recognition !== 'native')
}

function blockClass(block: Block) {
  if (block.heading === 1) return 'pt-1 text-lg font-semibold text-slate-900'
  if (block.heading === 2) return 'pt-1 text-base font-semibold text-slate-900'
  if (block.heading) return 'font-semibold text-slate-800'
  if (block.kind === 'ocr') return 'border-l-2 border-sky-200 pl-3 whitespace-pre-wrap text-slate-600'
  if (block.kind === 'table') return 'overflow-x-auto rounded bg-slate-50 px-3 py-2 font-mono text-[13px] whitespace-pre'
  return 'whitespace-pre-wrap break-words'
}
</script>
