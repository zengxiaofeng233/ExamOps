<template>
  <div class="h-full overflow-y-auto p-6">
    <div class="mx-auto max-w-5xl space-y-5">
      <section class="card p-5">
        <h2 class="section-title">课程简介</h2>
        <p class="mt-2 text-sm whitespace-pre-wrap text-slate-600">{{ course.description || '暂无简介，可在课程列表中编辑。' }}</p>
        <p class="mt-3 text-xs text-slate-400">创建于 {{ formatDateTime(course.created_at) }}</p>
      </section>
      <section class="card p-5">
        <h2 class="section-title">开始学习</h2>
        <p class="mt-1 text-sm text-slate-500">先导入教材、PPT 等资料，系统会自动提取正文。</p>
        <RouterLink :to="{ name: 'documents', params: { id: course.id } }" class="btn btn-primary mt-4">管理资料</RouterLink>
      </section>
    </div>
  </div>
</template>

<script setup lang="ts">
import type { Course } from '@/types/models'
import { formatDateTime } from '@/utils/format'

defineProps<{ course: Course }>()
</script>
