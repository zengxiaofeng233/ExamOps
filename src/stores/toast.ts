import { defineStore } from 'pinia'
import { ref } from 'vue'
import { errorMessage } from '@/utils/errors'

export type ToastKind = 'success' | 'error' | 'info'

export interface Toast {
  id: number
  kind: ToastKind
  text: string
}

let nextId = 1

export const useToastStore = defineStore('toast', () => {
  const toasts = ref<Toast[]>([])

  function push(kind: ToastKind, text: string, timeout = kind === 'error' ? 7000 : 3500) {
    const id = nextId++
    toasts.value.push({ id, kind, text })
    if (timeout > 0) setTimeout(() => dismiss(id), timeout)
    return id
  }

  function dismiss(id: number) {
    toasts.value = toasts.value.filter(t => t.id !== id)
  }

  return {
    toasts,
    dismiss,
    success: (text: string) => push('success', text),
    info: (text: string) => push('info', text),
    error: (error: unknown) => push('error', errorMessage(error))
  }
})
