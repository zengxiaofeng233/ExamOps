<template>
  <div class="w-full h-full flex flex-col">
    <router-view />
  </div>
</template>

<script setup lang="ts">
import { onMounted } from 'vue'

onMounted(() => {
  console.log('ExamOps initialized')
})
</script>
