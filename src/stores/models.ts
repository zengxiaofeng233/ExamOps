import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import type { ModelProfile } from '@/types/ai'
import { ModelService } from '@/services/ai'

export const useModelStore = defineStore('models', () => {
  const profiles = ref<ModelProfile[]>([])
  const loaded = ref(false)

  const defaultProfile = computed(() => profiles.value.find(p => p.is_default) ?? profiles.value[0] ?? null)
  const configured = computed(() => profiles.value.length > 0)

  async function load(force = false) {
    if (loaded.value && !force) return profiles.value
    profiles.value = await ModelService.list()
    loaded.value = true
    return profiles.value
  }

  return { profiles, loaded, defaultProfile, configured, load }
})
