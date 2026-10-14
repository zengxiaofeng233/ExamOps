<template>
  <div class="flex items-center gap-2" :title="title">
    <div class="h-1.5 overflow-hidden rounded-full bg-slate-100" :class="compact ? 'w-14' : 'w-24'" role="meter" :aria-valuenow="score === null ? undefined : Math.round(score * 100)" aria-valuemin="0" aria-valuemax="100" :aria-label="label">
      <div class="h-full rounded-full transition-all" :class="TONE_BAR[tone]" :style="{ width: `${score === null ? 0 : Math.max(4, score * 100)}%` }" />
    </div>
    <span class="text-xs whitespace-nowrap tabular-nums" :class="TONE_TEXT[tone]">{{ score === null ? '未测' : percent(score) }}</span>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { percent, scoreTone, TONE_BAR, TONE_TEXT } from '@/utils/assessment'

const props = withDefaults(defineProps<{ score: number | null; compact?: boolean; label?: string }>(), { compact: false, label: '掌握度' })
const tone = computed(() => scoreTone(props.score))
const title = computed(() => (props.score === null ? '还没有测验记录' : `${props.label} ${percent(props.score)}（由测验结果计算）`))
</script>
