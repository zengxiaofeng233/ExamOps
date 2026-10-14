<template>
  <span><template v-for="(segment, index) in segments" :key="index"><mark
    v-if="segment.hit"
    class="rounded-sm bg-amber-200/80 px-0.5 text-inherit"
  >{{ segment.text }}</mark><template v-else>{{ segment.text }}</template></template></span>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { highlightSegments, type TextSegment } from '@/utils/highlight'

const props = defineProps<{ text?: string; terms?: string[]; segments?: TextSegment[] }>()

const segments = computed(() => props.segments ?? highlightSegments(props.text ?? '', props.terms ?? []))
</script>
