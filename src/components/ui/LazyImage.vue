<template>
  <div ref="root" class="relative overflow-hidden" :style="placeholderStyle">
    <img v-if="src" :src="src" :alt="alt" class="block h-auto w-full" draggable="false" />
    <div v-else-if="error" class="flex h-full min-h-16 items-center justify-center p-3 text-center text-xs text-rose-600">{{ error }}</div>
    <div v-else class="flex h-full min-h-16 items-center justify-center bg-slate-100 text-xs text-slate-400">加载中…</div>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { errorMessage } from '@/utils/errors'

const props = defineProps<{
  load: () => Promise<string>
  alt?: string
  /** width / height, used to reserve space before the image arrives */
  ratio?: number
}>()

const root = ref<HTMLElement>()
const src = ref('')
const error = ref('')
let observer: IntersectionObserver | undefined

const placeholderStyle = computed(() => (!src.value && props.ratio ? { aspectRatio: String(props.ratio) } : {}))

async function fetchImage() {
  try {
    src.value = await props.load()
  } catch (e) {
    error.value = errorMessage(e)
  }
}

onMounted(() => {
  if (!('IntersectionObserver' in window)) return void fetchImage()
  observer = new IntersectionObserver((entries) => {
    if (entries.some(e => e.isIntersecting)) {
      observer?.disconnect()
      fetchImage()
    }
  }, { rootMargin: '400px' })
  if (root.value) observer.observe(root.value)
})

onBeforeUnmount(() => observer?.disconnect())
</script>
