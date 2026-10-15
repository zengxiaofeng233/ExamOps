import { defineStore } from 'pinia'
import { ref } from 'vue'
import { AppService } from '@/services/app'
import type { AvailableUpdate } from '@/types/app'
import { errorMessage } from '@/utils/errors'
import { useToastStore } from './toast'

export const useUpdateStore = defineStore('updates', () => {
  const available = ref<AvailableUpdate | null>(null)
  const checking = ref(false)
  const checked = ref(false)
  const error = ref('')
  const dismissed = ref(false)
  let pending: Promise<void> | undefined

  function check(): Promise<void> {
    if (pending) return pending
    checking.value = true
    error.value = ''
    pending = AppService.checkUpdates()
      .then(result => {
        available.value = result
        dismissed.value = false
        checked.value = true
      })
      .catch(reason => {
        // Automatic failures are quiet; the About page can explain them on demand.
        error.value = errorMessage(reason)
      })
      .finally(() => {
        checking.value = false
        pending = undefined
      })
    return pending
  }

  async function openRelease() {
    if (!available.value) return
    try {
      await AppService.openExternal(available.value.release_url)
    } catch (reason) {
      useToastStore().error(reason)
    }
  }

  function dismiss() { dismissed.value = true }

  return { available, checking, checked, error, dismissed, check, openRelease, dismiss }
})
