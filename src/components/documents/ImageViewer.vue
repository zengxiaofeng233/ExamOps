<template>
  <AppModal :open="!!image" :title="title" size="xl" @close="emit('close')">
    <template v-if="image">
      <LazyImage
        :key="image.id"
        class="mx-auto max-h-[65vh] max-w-full rounded border border-slate-200 bg-white [&_img]:max-h-[65vh] [&_img]:w-auto [&_img]:mx-auto"
        :load="() => DocumentService.image(image!.id)"
        :alt="title"
      />
      <div v-if="image.ocr_text" class="mt-4">
        <p class="text-xs font-medium text-slate-500">图片中识别到的文字（本地 OCR）</p>
        <p class="mt-1 rounded-lg bg-slate-50 p-3 text-sm whitespace-pre-wrap text-slate-700">{{ image.ocr_text }}</p>
      </div>
    </template>
  </AppModal>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import AppModal from '@/components/ui/AppModal.vue'
import LazyImage from '@/components/ui/LazyImage.vue'
import { DocumentService } from '@/services/document'
import type { ImageInfo } from '@/types/document'

const props = defineProps<{ image: ImageInfo | null; label?: string }>()
const emit = defineEmits<{ close: [] }>()

const title = computed(() => (props.image ? `${props.label ?? ''} 图 ${props.image.ordinal}`.trim() : ''))
</script>
