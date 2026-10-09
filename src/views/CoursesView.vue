<template>
  <div class="w-full h-full flex flex-col bg-gray-50">
    <header class="bg-white border-b border-gray-200 px-6 py-4">
      <h1 class="text-2xl font-bold text-gray-900">我的课程</h1>
    </header>

    <main class="flex-1 p-6 overflow-y-auto">
      <div class="mb-6">
        <button
          @click="showCreateDialog = true"
          class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
        >
          创建课程
        </button>
      </div>

      <div v-if="courseStore.loading" class="text-center py-12">
        <p class="text-gray-500">加载中...</p>
      </div>

      <div v-else-if="courseStore.error" class="text-center py-12">
        <p class="text-red-600">{{ courseStore.error }}</p>
        <button
          @click="courseStore.fetchCourses()"
          class="mt-4 px-4 py-2 bg-gray-600 text-white rounded-lg hover:bg-gray-700"
        >
          重试
        </button>
      </div>

      <div v-else-if="courseStore.sortedCourses.length === 0" class="text-center py-12">
        <p class="text-gray-500">暂无课程，点击上方按钮创建第一个课程</p>
      </div>

      <div v-else class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
        <div
          v-for="course in courseStore.sortedCourses"
          :key="course.id"
          class="bg-white rounded-lg border border-gray-200 p-4 hover:shadow-md transition-shadow"
        >
          <h3 class="text-lg font-semibold text-gray-900 mb-2">{{ course.name }}</h3>
          <p class="text-gray-600 text-sm mb-4">{{ course.description }}</p>
          <div class="flex gap-2">
            <button
              @click="editCourse(course)"
              class="px-3 py-1 text-sm bg-gray-100 text-gray-700 rounded hover:bg-gray-200"
            >
              编辑
            </button>
            <button
              @click="confirmDelete(course)"
              class="px-3 py-1 text-sm bg-red-100 text-red-700 rounded hover:bg-red-200"
            >
              删除
            </button>
          </div>
        </div>
      </div>
    </main>

    <!-- Create/Edit Dialog -->
    <div
      v-if="showCreateDialog || showEditDialog"
      class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center p-4"
      @click.self="closeDialogs"
    >
      <div class="bg-white rounded-lg p-6 w-full max-w-md">
        <h2 class="text-xl font-bold mb-4">
          {{ showEditDialog ? '编辑课程' : '创建课程' }}
        </h2>
        <form @submit.prevent="handleSubmit">
          <div class="mb-4">
            <label class="block text-sm font-medium text-gray-700 mb-2">
              课程名称
            </label>
            <input
              v-model="formData.name"
              type="text"
              required
              class="w-full px-3 py-2 border border-gray-300 rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500"
              placeholder="输入课程名称"
            />
          </div>
          <div class="mb-4">
            <label class="block text-sm font-medium text-gray-700 mb-2">
              课程描述
            </label>
            <textarea
              v-model="formData.description"
              rows="4"
              class="w-full px-3 py-2 border border-gray-300 rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500"
              placeholder="输入课程描述"
            />
          </div>
          <div class="flex gap-2 justify-end">
            <button
              type="button"
              @click="closeDialogs"
              class="px-4 py-2 bg-gray-200 text-gray-700 rounded-lg hover:bg-gray-300"
            >
              取消
            </button>
            <button
              type="submit"
              :disabled="courseStore.loading"
              class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:opacity-50"
            >
              {{ showEditDialog ? '保存' : '创建' }}
            </button>
          </div>
        </form>
      </div>
    </div>

    <!-- Delete Confirmation Dialog -->
    <div
      v-if="showDeleteDialog"
      class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center p-4"
      @click.self="showDeleteDialog = false"
    >
      <div class="bg-white rounded-lg p-6 w-full max-w-md">
        <h2 class="text-xl font-bold mb-4">确认删除</h2>
        <p class="text-gray-600 mb-6">
          确定要删除课程 "{{ courseToDelete?.name }}" 吗？此操作无法撤销。
        </p>
        <div class="flex gap-2 justify-end">
          <button
            @click="showDeleteDialog = false"
            class="px-4 py-2 bg-gray-200 text-gray-700 rounded-lg hover:bg-gray-300"
          >
            取消
          </button>
          <button
            @click="handleDelete"
            :disabled="courseStore.loading"
            class="px-4 py-2 bg-red-600 text-white rounded-lg hover:bg-red-700 disabled:opacity-50"
          >
            删除
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, reactive } from 'vue'
import { useCourseStore } from '@/stores/course'
import type { Course } from '@/types/models'

const courseStore = useCourseStore()

const showCreateDialog = ref(false)
const showEditDialog = ref(false)
const showDeleteDialog = ref(false)
const courseToDelete = ref<Course | null>(null)
const editingCourse = ref<Course | null>(null)

const formData = reactive({
  name: '',
  description: ''
})

onMounted(async () => {
  await courseStore.fetchCourses()
})

const closeDialogs = () => {
  showCreateDialog.value = false
  showEditDialog.value = false
  formData.name = ''
  formData.description = ''
  editingCourse.value = null
}

const editCourse = (course: Course) => {
  editingCourse.value = course
  formData.name = course.name
  formData.description = course.description
  showEditDialog.value = true
}

const confirmDelete = (course: Course) => {
  courseToDelete.value = course
  showDeleteDialog.value = true
}

const handleSubmit = async () => {
  try {
    if (showEditDialog.value && editingCourse.value) {
      await courseStore.updateCourse(
        editingCourse.value.id,
        formData.name,
        formData.description
      )
    } else {
      await courseStore.createCourse(formData.name, formData.description)
    }
    closeDialogs()
  } catch (error) {
    console.error('Failed to save course:', error)
  }
}

const handleDelete = async () => {
  if (courseToDelete.value) {
    try {
      await courseStore.deleteCourse(courseToDelete.value.id)
      showDeleteDialog.value = false
      courseToDelete.value = null
    } catch (error) {
      console.error('Failed to delete course:', error)
    }
  }
}
</script>
