<template>
  <div class="flex h-full flex-col">
    <header class="flex items-center justify-between gap-4 border-b border-slate-200 bg-white px-6 py-4">
      <div>
        <h1 class="page-title">我的课程</h1>
        <p class="mt-0.5 text-sm text-slate-500">每门课程独立管理资料、学习路径和复习记录。</p>
      </div>
      <button class="btn btn-primary" @click="openCreate">
        <PlusIcon class="size-4" />创建课程
      </button>
    </header>

    <main class="flex-1 overflow-y-auto p-6">
      <p v-if="courseStore.loading && !courseStore.courses.length" class="py-12 text-center text-slate-500">加载中…</p>

      <div v-else-if="courseStore.error && !courseStore.courses.length" class="py-12 text-center">
        <p class="text-rose-600">{{ courseStore.error }}</p>
        <button class="btn btn-secondary mt-4" @click="courseStore.fetchCourses().catch(() => undefined)">重试</button>
      </div>

      <EmptyState
        v-else-if="!courseStore.sortedCourses.length"
        title="还没有课程"
        description="创建一门课程，然后导入教材、PPT 等学习资料。"
      >
        <button class="btn btn-primary" @click="openCreate">创建第一门课程</button>
      </EmptyState>

      <div v-else class="grid grid-cols-1 gap-4 md:grid-cols-2 xl:grid-cols-3">
        <article
          v-for="course in courseStore.sortedCourses"
          :key="course.id"
          class="card group flex flex-col p-5 transition-shadow hover:shadow-md"
        >
          <RouterLink :to="`/courses/${course.id}`" class="min-w-0 flex-1">
            <h3 class="truncate text-base font-semibold text-slate-900 group-hover:text-indigo-700" :title="course.name">{{ course.name }}</h3>
            <p class="mt-1.5 line-clamp-3 min-h-[1.25rem] text-sm text-slate-500">{{ course.description || '暂无简介' }}</p>
            <p v-if="course.exam_date" class="mt-1 text-xs text-indigo-600">{{ examCountdown(course.exam_date) }}（{{ course.exam_date }}）</p>
          </RouterLink>
          <div class="mt-4 flex items-center justify-between gap-2">
            <span class="text-xs text-slate-400">更新于 {{ formatRelative(course.updated_at) }}</span>
            <div class="flex gap-1">
              <RouterLink :to="`/courses/${course.id}/documents`" class="btn btn-soft btn-sm">资料</RouterLink>
              <button class="btn btn-ghost btn-sm" @click="openEdit(course)">编辑</button>
              <button class="btn btn-ghost btn-sm text-rose-600 hover:text-rose-700" @click="courseToDelete = course">删除</button>
            </div>
          </div>
        </article>
      </div>
    </main>

    <AppModal :open="dialogOpen" :title="editing ? '编辑课程' : '创建课程'" @close="closeDialog">
      <form id="course-form" class="space-y-4" @submit.prevent="handleSubmit">
        <div>
          <label class="label" for="course-name">课程名称</label>
          <input id="course-name" v-model="form.name" type="text" required maxlength="100" class="input" placeholder="例如：高等数学（上）" />
        </div>
        <div>
          <label class="label" for="course-desc">课程描述</label>
          <textarea id="course-desc" v-model="form.description" rows="4" class="input" placeholder="考试范围、任课老师、教材版本等（可选）" />
        </div>
        <div>
          <label class="label" for="course-exam">考试日期（可选）</label>
          <input id="course-exam" v-model="form.exam_date" type="date" class="input" />
          <p class="hint">设置后，复习计划会在考前自动加密，考前冲刺也会参考剩余天数。</p>
        </div>
      </form>
      <template #footer>
        <button type="button" class="btn btn-secondary" @click="closeDialog">取消</button>
        <button type="submit" form="course-form" class="btn btn-primary" :disabled="saving || !form.name.trim()">
          {{ saving ? '保存中…' : editing ? '保存' : '创建' }}
        </button>
      </template>
    </AppModal>

    <ConfirmDialog
      :open="!!courseToDelete"
      title="删除课程"
      :message="`确定要删除课程“${courseToDelete?.name}”吗？\n课程的资料副本、提取正文、学习记录将一并删除，原文件不受影响。此操作无法撤销。`"
      confirm-text="删除"
      danger
      :busy="deleting"
      @cancel="courseToDelete = null"
      @confirm="handleDelete"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { PlusIcon } from '@heroicons/vue/20/solid'
import AppModal from '@/components/ui/AppModal.vue'
import ConfirmDialog from '@/components/ui/ConfirmDialog.vue'
import EmptyState from '@/components/ui/EmptyState.vue'
import { useCourseStore } from '@/stores/course'
import { useToastStore } from '@/stores/toast'
import type { Course } from '@/types/models'
import { formatRelative } from '@/utils/format'
import { examCountdown } from '@/utils/study'

const router = useRouter()
const courseStore = useCourseStore()
const toast = useToastStore()

const dialogOpen = ref(false)
const editing = ref<Course | null>(null)
const courseToDelete = ref<Course | null>(null)
const saving = ref(false)
const deleting = ref(false)
const form = reactive({ name: '', description: '', exam_date: '' })

onMounted(() => courseStore.fetchCourses().catch(() => undefined))

function openCreate() {
  editing.value = null
  form.name = ''
  form.description = ''
  form.exam_date = ''
  dialogOpen.value = true
}

function openEdit(course: Course) {
  editing.value = course
  form.name = course.name
  form.description = course.description
  form.exam_date = course.exam_date ?? ''
  dialogOpen.value = true
}

function closeDialog() {
  dialogOpen.value = false
  editing.value = null
}

async function handleSubmit() {
  saving.value = true
  try {
    if (editing.value) {
      await courseStore.updateCourse({ id: editing.value.id, name: form.name, description: form.description, exam_date: form.exam_date })
      toast.success('课程已保存')
      closeDialog()
    } else {
      const course = await courseStore.createCourse(form.name, form.description)
      if (form.exam_date) await courseStore.updateCourse({ id: course.id, exam_date: form.exam_date })
      closeDialog()
      toast.success('课程已创建，现在可以导入资料了')
      router.push(`/courses/${course.id}/documents`)
    }
  } catch (error) {
    toast.error(error)
  } finally {
    saving.value = false
  }
}

async function handleDelete() {
  if (!courseToDelete.value) return
  deleting.value = true
  try {
    await courseStore.deleteCourse(courseToDelete.value.id)
    toast.success('课程已删除')
    courseToDelete.value = null
  } catch (error) {
    toast.error(error)
  } finally {
    deleting.value = false
  }
}
</script>
