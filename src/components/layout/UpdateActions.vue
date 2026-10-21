<template>
  <div class="flex min-w-0 max-w-full flex-wrap items-center gap-2">
    <span v-if="updates.phase === 'downloading'" class="text-xs" role="status">正在下载并校验 · {{ updates.progressText }}</span>
    <span v-else-if="updates.phase === 'ready'" class="text-xs">更新包已校验，可安装</span>
    <button v-if="updates.phase === 'idle'" type="button" class="btn btn-primary btn-sm" :disabled="updates.checking" @click="updates.download">
      {{ updates.downloadError ? '重试下载' : '下载更新' }}
    </button>
    <button v-else-if="updates.phase !== 'downloading'" type="button" class="btn btn-primary btn-sm" :disabled="updates.busy" @click="confirmInstall = true">
      {{ updates.phase === 'installing' ? '正在启动安装…' : '安装并重启' }}
    </button>
    <button type="button" class="btn btn-ghost btn-sm" :disabled="updates.phase === 'installing'" @click="updates.openRelease">手动下载</button>
    <p v-if="updates.downloadError" class="w-full break-words text-xs text-rose-600" role="alert">{{ updates.downloadError }}</p>
  <ConfirmDialog :open="confirmInstall" title="安装更新" confirm-text="安装并重启"
    message="安装将关闭 ExamOps，完成后重新打开。请先保存正在编辑的内容，并等待 AI 回答完成。课程、资料、笔记和设置会保留。"
    @cancel="confirmInstall = false" @confirm="install" />
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import ConfirmDialog from '@/components/ui/ConfirmDialog.vue'
import { useUpdateStore } from '@/stores/updates'
const updates = useUpdateStore()
const confirmInstall = ref(false)
function install() {
  confirmInstall.value = false
  void updates.install()
}
</script>
