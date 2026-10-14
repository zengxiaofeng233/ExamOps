<template>
  <div class="h-full overflow-y-auto">
    <div class="mx-auto max-w-5xl space-y-6 p-6">
      <section>
        <h1 class="text-2xl font-semibold text-slate-900">欢迎使用 ExamOps</h1>
        <p class="mt-1 text-sm text-slate-500">导入自己的课程资料，按自己的节奏学习、检验和复习。所有数据只保存在本机。</p>
      </section>

      <section v-if="last && lastCourse" class="card flex flex-wrap items-center justify-between gap-4 p-5">
        <div class="min-w-0">
          <p class="text-xs font-medium text-indigo-600">继续上次学习</p>
          <p class="mt-1 truncate text-base font-semibold text-slate-900">{{ lastCourse.name }}</p>
          <p class="text-xs text-slate-500">{{ last.saved_at ? `上次学习：${formatRelative(last.saved_at)}` : '' }}</p>
        </div>
        <RouterLink :to="last.path" class="btn btn-primary">继续</RouterLink>
      </section>

      <section v-if="reviews.length" class="card p-5" aria-labelledby="today-title">
        <h2 id="today-title" class="section-title">今天要复习</h2>
        <ul class="mt-3 divide-y divide-slate-100">
          <li v-for="item in reviews" :key="item.course_id" class="flex flex-wrap items-center gap-3 py-2.5">
            <span class="min-w-0 flex-1 truncate text-sm font-medium text-slate-800">{{ item.name }}</span>
            <span v-if="item.due" class="badge bg-rose-50 text-rose-700">到期 {{ item.due }}</span>
            <span v-if="item.open_mistakes" class="badge bg-slate-100 text-slate-600">错题 {{ item.open_mistakes }}</span>
            <span v-if="item.days_to_exam !== null && item.days_to_exam >= 0" class="text-xs text-slate-500">{{ examCountdown(item.exam_date) }}</span>
            <RouterLink :to="{ name: 'review', params: { id: item.course_id } }" class="btn btn-soft btn-sm">去复习</RouterLink>
          </li>
        </ul>
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
            class="card flex flex-col p-4 transition-shadow hover:shadow-md"
          >
            <p class="truncate font-medium text-slate-900">{{ course.name }}</p>
            <p class="mt-1 line-clamp-2 flex-1 text-xs text-slate-500">{{ course.description || '暂无简介' }}</p>
            <template v-if="digest(course.id)?.points">
              <div class="mt-3 flex h-1.5 overflow-hidden rounded-full bg-slate-100" aria-hidden="true">
                <div class="bg-emerald-500" :style="{ width: `${share(course.id, 'mastered')}%` }" />
                <div class="bg-sky-400" :style="{ width: `${share(course.id, 'learned') - share(course.id, 'mastered')}%` }" />
              </div>
              <p class="mt-1 text-[11px] text-slate-500">
                {{ digest(course.id)!.points }} 个知识点 · 学过 {{ digest(course.id)!.learned }} · 已掌握 {{ digest(course.id)!.mastered }}
              </p>
            </template>
            <p v-if="digest(course.id)?.next_step" class="mt-1 truncate text-[11px] text-indigo-600">下一步：{{ digest(course.id)!.next_step }}</p>
            <p v-if="course.exam_date" class="mt-1 text-[11px] text-slate-400">{{ examCountdown(course.exam_date) }}</p>
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
import { StudyService } from '@/services/study'
import type { LastLocation } from '@/types/app'
import type { CourseDigest } from '@/types/study'
import { formatRelative } from '@/utils/format'
import { examCountdown } from '@/utils/study'

const courses = useCourseStore()
const last = ref<LastLocation | null>(null)
const digests = ref<CourseDigest[]>([])
const lastCourse = computed(() => courses.courses.find(c => c.id === last.value?.course_id))
const reviews = computed(() => digests.value.filter(d => d.due > 0 || d.open_mistakes > 0))

function digest(id: number) {
  return digests.value.find(d => d.course_id === id)
}

function share(id: number, field: 'learned' | 'mastered') {
  const item = digest(id)
  return item && item.points ? Math.round((item[field] / item.points) * 100) : 0
}

onMounted(async () => {
  await courses.ensureLoaded()
  const [location, list] = await Promise.all([
    AppService.lastLocation().catch(() => null),
    StudyService.dashboard().catch(() => [])
  ])
  last.value = location
  digests.value = list
})
</script>
