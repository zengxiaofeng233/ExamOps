<template>
  <div class="h-full overflow-y-auto p-5">
    <div class="mx-auto max-w-3xl">
      <p class="mb-3 text-sm text-slate-500">
        <template v-if="loading">正在检索…</template>
        <template v-else-if="result">“{{ result.query }}”共找到 {{ result.hits.length }}{{ result.hits.length >= 50 ? '+' : '' }} 处</template>
      </p>
      <EmptyState
        v-if="result && !loading && !result.hits.length"
        title="没有找到匹配内容"
        description="检索按原文精确匹配；可尝试更短的关键词，或用空格分隔多个词。"
      />
      <ul v-else class="space-y-2">
        <li v-for="hit in result?.hits ?? []" :key="hit.chunk_id">
          <button class="card block w-full p-4 text-left transition-colors hover:border-indigo-300" @click="emit('open', hit)">
            <p class="flex flex-wrap items-center gap-x-2 text-xs text-slate-500">
              <span class="font-medium text-slate-700">{{ hit.filename }}</span>
              <span v-if="hit.section_title">· {{ hit.section_title }}</span>
              <span class="badge bg-slate-100 text-slate-600">{{ hit.loc_label }}</span>
            </p>
            <p class="mt-2 text-sm leading-6 text-slate-700">
              <HighlightText :segments="hit.snippet" />
            </p>
          </button>
        </li>
      </ul>
    </div>
  </div>
</template>

<script setup lang="ts">
import EmptyState from '@/components/ui/EmptyState.vue'
import HighlightText from '@/components/ui/HighlightText.vue'
import type { SearchHit, SearchResult } from '@/types/document'

defineProps<{ result: SearchResult | null; loading: boolean }>()
const emit = defineEmits<{ open: [hit: SearchHit] }>()
</script>
