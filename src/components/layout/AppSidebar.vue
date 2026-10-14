<template>
  <aside class="flex w-56 shrink-0 flex-col border-r border-slate-200 bg-white">
    <div class="flex items-center gap-2.5 px-4 pt-5 pb-4">
      <div class="flex size-8 items-center justify-center rounded-lg bg-indigo-600 text-sm font-bold text-white">E</div>
      <div>
        <p class="text-sm leading-tight font-semibold text-slate-900">ExamOps</p>
        <p class="text-xs leading-tight text-slate-500">AI 智能复习</p>
      </div>
    </div>

    <nav class="flex min-h-0 flex-1 flex-col gap-0.5 overflow-y-auto px-2" aria-label="主导航">
      <RouterLink v-for="item in mainItems" :key="item.to" :to="item.to" class="nav-item" :class="{ 'nav-active': isActive(item) }">
        <component :is="item.icon" class="size-5 shrink-0" />
        <span>{{ item.label }}</span>
      </RouterLink>

      <template v-if="courseStore.sortedCourses.length">
        <p class="mt-4 mb-1 px-3 text-xs font-medium text-slate-400">最近课程</p>
        <RouterLink
          v-for="course in courseStore.sortedCourses.slice(0, 6)"
          :key="course.id"
          :to="`/courses/${course.id}`"
          class="nav-item"
          :class="{ 'nav-active': activeCourseId === course.id }"
          :title="course.name"
        >
          <span class="flex size-5 shrink-0 items-center justify-center rounded bg-slate-100 text-[11px] font-semibold text-slate-600">
            {{ course.name.slice(0, 1) }}
          </span>
          <span class="truncate">{{ course.name }}</span>
        </RouterLink>
      </template>
    </nav>

    <div class="border-t border-slate-100 p-2">
      <RouterLink to="/settings" class="nav-item" :class="{ 'nav-active': route.path.startsWith('/settings') }">
        <Cog6ToothIcon class="size-5 shrink-0" />
        <span>设置</span>
      </RouterLink>
    </div>
  </aside>
</template>

<script setup lang="ts">
import { computed, type Component } from 'vue'
import { useRoute } from 'vue-router'
import { HomeIcon, RectangleStackIcon, Cog6ToothIcon } from '@heroicons/vue/24/outline'
import { useCourseStore } from '@/stores/course'

interface NavItem { to: string; label: string; icon: Component; exact?: boolean }

const route = useRoute()
const courseStore = useCourseStore()

const mainItems: NavItem[] = [
  { to: '/', label: '首页', icon: HomeIcon, exact: true },
  { to: '/courses', label: '我的课程', icon: RectangleStackIcon, exact: true }
]

const activeCourseId = computed(() => {
  const match = route.path.match(/^\/courses\/(\d+)/)
  return match ? Number(match[1]) : null
})

function isActive(item: NavItem) {
  return item.exact ? route.path === item.to : route.path.startsWith(item.to)
}
</script>

<style scoped>
@reference "../../style.css";

.nav-item {
  @apply flex items-center gap-2.5 rounded-lg px-3 py-2 text-sm text-slate-600 transition-colors hover:bg-slate-100 hover:text-slate-900;
}
.nav-active {
  @apply bg-indigo-50 font-medium text-indigo-700 hover:bg-indigo-50 hover:text-indigo-700;
}
</style>
