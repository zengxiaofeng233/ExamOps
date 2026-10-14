import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { AppSettings } from '@/types/app'
import { AppService } from '@/services/app'

const DEFAULTS: AppSettings = {
  restore_last_location: true,
  context: { mode: 'auto', rag_top_k: 8, full_ratio: 0.8, history_turns: 6 }
}

export const useAppStore = defineStore('app', () => {
  const settings = ref<AppSettings>({ ...DEFAULTS })
  const ready = ref(false)

  async function load() {
    try {
      const loaded = await AppService.settings()
      settings.value = { ...DEFAULTS, ...loaded, context: { ...DEFAULTS.context, ...loaded.context } }
    } finally {
      ready.value = true
    }
  }

  async function save(next: AppSettings) {
    settings.value = await AppService.updateSettings(next)
    return settings.value
  }

  let pending: ReturnType<typeof setTimeout> | undefined
  /** Debounced: remembers the current course page so the next launch can resume there. */
  function rememberLocation(path: string) {
    const match = path.match(/^\/courses\/(\d+)/)
    if (!match) return
    clearTimeout(pending)
    pending = setTimeout(() => {
      AppService.saveLocation({ path, course_id: Number(match[1]) }).catch(() => undefined)
    }, 600)
  }

  return { settings, ready, load, save, rememberLocation }
})
