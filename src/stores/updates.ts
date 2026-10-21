import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
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
  const phase = ref<'idle' | 'downloading' | 'ready' | 'installing'>('idle')
  const downloadError = ref('')
  const downloaded = ref(0)
  const total = ref<number | null>(null)
  const busy = computed(() => phase.value === 'downloading' || phase.value === 'installing')
  const progressText = computed(() => {
    const mb = (downloaded.value / 1048576).toFixed(1)
    return total.value ? `${Math.min(100, Math.floor(downloaded.value / total.value * 100))}% · ${mb} / ${(total.value / 1048576).toFixed(1)} MB` : `${mb} MB`
  })
  let pending: Promise<void> | undefined

  function check(): Promise<void> {
    if (pending) return pending
    // Do not replace the version associated with an in-flight or verified package.
    if (phase.value !== 'idle') return Promise.resolve()
    checking.value = true
    error.value = ''
    pending = AppService.checkUpdates()
      .then(result => {
        available.value = result
        downloadError.value = ''
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

  async function download() {
    if (!available.value || phase.value !== 'idle' || checking.value) return
    phase.value = 'downloading'
    downloadError.value = ''
    downloaded.value = 0
    total.value = null
    try {
      await AppService.downloadUpdate(available.value.version, value => {
        downloaded.value = value.downloaded
        total.value = value.total
      })
      phase.value = 'ready'
    } catch (reason) {
      downloadError.value = errorMessage(reason)
      phase.value = 'idle'
    }
  }

  async function install() {
    if (phase.value !== 'ready') return
    phase.value = 'installing'
    downloadError.value = ''
    try {
      await AppService.installUpdate()
    } catch (reason) {
      downloadError.value = errorMessage(reason)
      phase.value = 'ready'
    }
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

  return { available, checking, checked, error, dismissed, phase, downloadError, downloaded, total, busy, progressText, check, download, install, openRelease, dismiss }
})
