<template>
  <div class="w-full h-full flex items-center justify-center bg-gray-50">
    <div class="text-center">
      <h1 class="text-4xl font-bold text-gray-900 mb-4">ExamOps</h1>
      <p class="text-gray-600 mb-8">AI 智能学习桌面应用</p>
      <button
        @click="navigateToCourses"
        class="px-6 py-3 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
      >
        开始使用
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { useRouter } from 'vue-router'

const router = useRouter()

const navigateToCourses = () => {
  router.push('/courses')
}
</script>
