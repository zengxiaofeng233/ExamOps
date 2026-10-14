<template>
  <div class="h-full overflow-y-auto">
    <div class="mx-auto max-w-5xl space-y-6 p-6">
      <section>
        <h1 class="text-2xl font-semibold text-slate-900">欢迎使用 ExamOps</h1>
        <p class="mt-1 text-sm text-slate-500">导入自己的课程资料，按自己的节奏学习、检验和复习。</p>
      </section>

      <section v-if="last && lastCourse" class="card flex flex-wrap items-center justify-between gap-4 p-5">
        <div class="min-w-0">
          <p class="text-xs font-medium text-indigo-600">继续上次学习</p>
          <p class="mt-1 truncate text-base font-semibold text-slate-900">{{ lastCourse.name }}</p>
          <p class="text-xs text-slate-500">{{ last.saved_at ? `上次学习：${formatRelative(last.saved_at)}` : '' }}</p>
        </div>
        <RouterLink :to="last.path" class="btn btn-primary">继续</RouterLink>
      </section>

      <section>
        <div class="mb-3 flex items-center justify-between">
          <h2 class="section-title">我的课程</h2>
          <RouterLink to="/courses" class="text-sm text-indigo-600 hover:text-indigo-700">全部课程</RouterLink>
        </div>
        <EmptyState v-if="!courses.sortedCourses.length" class="card" title="还没有课程" description="创建课程后即可导入教材、PPT 等资料。">
          <RouterLink to="/courses" class="btn btn-primary">创建课程</RouterLink>
        </EmptyState>
        <div v-else class="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
          <RouterLink
            v-for="course in courses.sortedCourses.slice(0, 6)"
            :key="course.id"
            :to="`/courses/${course.id}`"
            class="card p-4 transition-shadow hover:shadow-md"
          >
            <p class="truncate font-medium text-slate-900">{{ course.name }}</p>
            <p class="mt-1 line-clamp-2 text-xs text-slate-500">{{ course.description || '暂无简介' }}</p>
          </RouterLink>
        </div>
      </section>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import EmptyState from '@/components/ui/EmptyState.vue'
import { useCourseStore } from '@/stores/course'
import { AppService } from '@/services/app'
import type { LastLocation } from '@/types/app'
import { formatRelative } from '@/utils/format'

const courses = useCourseStore()
const last = ref<LastLocation | null>(null)
const lastCourse = computed(() => courses.courses.find(c => c.id === last.value?.course_id))

onMounted(async () => {
  await courses.ensureLoaded()
  last.value = await AppService.lastLocation().catch(() => null)
})
</script>
