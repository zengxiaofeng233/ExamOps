<template>
  <div
    v-if="updates.available && !updates.dismissed"
    class="flex shrink-0 flex-wrap items-center gap-x-4 gap-y-2 border-b border-indigo-200 bg-indigo-50 px-5 py-3"
    role="status"
  >
    <div class="min-w-0 flex-1 text-sm text-indigo-900">
      <p class="font-medium">发现新版本 v{{ updates.available.version }}</p>
      <p class="mt-0.5 text-xs text-indigo-700">当前为 v{{ updates.available.current_version }}，Windows x64 安装包已发布。</p>
    </div>
    <button type="button" class="btn btn-primary btn-sm" @click="updates.openRelease">查看更新</button>
    <button type="button" class="btn btn-ghost btn-sm" @click="updates.dismiss">稍后提醒</button>
  </div>
</template>

<script setup lang="ts">
import { useUpdateStore } from '@/stores/updates'
const updates = useUpdateStore()
</script>
