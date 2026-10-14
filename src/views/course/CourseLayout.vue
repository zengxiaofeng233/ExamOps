<template>
  <div class="flex h-full min-h-0 flex-col">
    <header class="border-b border-slate-200 bg-white px-6 pt-4">
      <div class="flex flex-wrap items-start justify-between gap-3">
        <div class="min-w-0">
          <RouterLink to="/courses" class="text-xs text-slate-500 hover:text-indigo-600">我的课程</RouterLink>
          <h1 class="page-title mt-0.5 truncate" :title="course?.name">{{ course?.name ?? (failed ? '课程不存在' : '加载中…') }}</h1>
        </div>
      </div>
      <nav class="-mb-px mt-3 flex gap-1 overflow-x-auto" aria-label="课程功能">
        <RouterLink
          v-for="tab in tabs"
          :key="tab.name"
          :to="{ name: tab.name, params: { id: courseId } }"
          class="border-b-2 px-3 py-2 text-sm whitespace-nowrap transition-colors"
          :class="route.name === tab.name
            ? 'border-indigo-600 font-medium text-indigo-700'
            : 'border-transparent text-slate-500 hover:border-slate-300 hover:text-slate-800'"
        >
          {{ tab.label }}
        </RouterLink>
      </nav>
    </header>
    <div class="min-h-0 flex-1 overflow-hidden">
      <EmptyState v-if="failed" title="找不到这门课程" description="它可能已被删除。">
        <RouterLink to="/courses" class="btn btn-primary">返回课程列表</RouterLink>
      </EmptyState>
      <RouterView v-else-if="course" v-slot="{ Component }">
        <component :is="Component" :key="courseId" :course="course" />
      </RouterView>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useRoute } from 'vue-router'
import EmptyState from '@/components/ui/EmptyState.vue'
import { useCourseStore } from '@/stores/course'

const props = defineProps<{ courseId: number }>()
const route = useRoute()
const store = useCourseStore()
const failed = ref(false)

const course = computed(() => store.courses.find(c => c.id === props.courseId) ?? null)

const tabs = [
  { name: 'course-overview', label: '概览' },
  { name: 'documents', label: '资料' },
  { name: 'tutor', label: 'AI 问答' }
]

watch(() => props.courseId, async (id) => {
  failed.value = false
  try {
    await store.loadCourse(id)
  } catch {
    failed.value = true
  }
}, { immediate: true })
</script>
