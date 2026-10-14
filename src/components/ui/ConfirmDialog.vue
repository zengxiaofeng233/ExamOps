<template>
  <AppModal :open="open" :title="title" size="sm" @close="emit('cancel')">
    <p class="text-sm leading-6 whitespace-pre-wrap text-slate-600">{{ message }}</p>
    <template #footer>
      <button class="btn btn-secondary" :disabled="busy" @click="emit('cancel')">取消</button>
      <button class="btn" :class="danger ? 'btn-danger' : 'btn-primary'" :disabled="busy" @click="emit('confirm')">
        {{ busy ? '处理中…' : confirmText }}
      </button>
    </template>
  </AppModal>
</template>

<script setup lang="ts">
import AppModal from './AppModal.vue'

withDefaults(defineProps<{
  open: boolean
  title: string
  message: string
  confirmText?: string
  danger?: boolean
  busy?: boolean
}>(), { confirmText: '确定', danger: false, busy: false })

const emit = defineEmits<{ confirm: []; cancel: [] }>()
</script>
